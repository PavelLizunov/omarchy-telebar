//! Server-permission and original media-membership gate. No filesystem action
//! begins until all native continuations have validated the requested context.
use crate::{
    commands::{Action, Answer, Result, arg_int},
    projection::{integer, number, typed},
    state::State,
};
use serde_json::{Value, json};
#[derive(Debug)]
pub struct Context {
    chat: i128,
    message: i128,
    file: i128,
    photo: bool,
    save: Option<crate::files::Save>,
    copy: Option<crate::clipboard::Context>,
}
#[derive(Debug)]
pub enum Gate {
    File(Context),
    Message(Context, String),
    Rich(Context, String),
    Permission(
        String,
        Option<crate::files::Save>,
        Option<crate::clipboard::Context>,
    ),
}
fn native(query: Value, gate: Gate) -> Action {
    Action::Native {
        query,
        answer: Answer::Export(gate),
    }
}
fn prepare_with(args: &Value, photo: bool, save: Option<crate::files::Save>) -> Result<Action> {
    let context = Context {
        chat: arg_int(args, "chatId", None, None, None)?,
        message: arg_int(args, "messageId", Some(1), None, None)?,
        file: arg_int(args, "fileId", Some(1), Some(i32::MAX as i128), None)?,
        photo,
        save,
        copy: None,
    };
    Ok(native(
        json!({"@type":"getFile","file_id":number(context.file)}),
        Gate::File(context),
    ))
}
pub fn prepare(args: &Value, photo: bool) -> Result<Action> {
    prepare_with(args, photo, None)
}
pub fn prepare_save(args: &Value, home: std::path::PathBuf) -> Result<Action> {
    prepare_with(
        args,
        false,
        Some(crate::files::Save {
            home,
            name: crate::files::name(&args["fileName"]),
        }),
    )
}
pub fn prepare_copy(args: &Value, clipboard: crate::clipboard::Context) -> Result<Action> {
    let mut action = prepare_with(args, true, None)?;
    if let Action::Native {
        answer: Answer::Export(Gate::File(context)),
        ..
    } = &mut action
    {
        context.copy = Some(clipboard);
    }
    Ok(action)
}
impl Context {
    fn copy(&self) -> Self {
        Self {
            chat: self.chat,
            message: self.message,
            file: self.file,
            photo: self.photo,
            save: self.save.clone(),
            copy: self.copy.clone(),
        }
    }
    fn permission(&self, body: &Value, path: String) -> Result<Action> {
        let candidates: Vec<_> = if body["kind"] == "rich" {
            body["blocks"].as_array().into_iter().flatten().collect()
        } else {
            vec![body]
        };
        if !candidates.iter().any(|c| {
            (!self.photo || c["kind"] == "photo")
                && integer(&c["media"]["file"]["id"], 0) == self.file
        }) {
            return Err("that file is not this message's media".into());
        }
        Ok(native(
            json!({"@type":"getMessageProperties","chat_id":number(self.chat),"message_id":number(self.message)}),
            Gate::Permission(path, self.save.clone(), self.copy.clone()),
        ))
    }
}
impl Gate {
    pub fn complete(&self, event: &Value, state: &State) -> Result<Action> {
        Ok(match self {
            Self::File(c) => {
                if integer(&event["id"], 0) != c.file {
                    return Err("that is not the requested file".into());
                }
                let file = typed(event, "file");
                if file["local"]["is_downloading_completed"] != true {
                    return Err("the file is not downloaded yet".into());
                }
                let path = crate::projection::local_path(&file["local"]["path"], &state.roots);
                if path.is_empty() {
                    return Err("that file is not one of Telebar's".into());
                }
                native(
                    json!({"@type":"getMessage","chat_id":number(c.chat),"message_id":number(c.message)}),
                    Self::Message(c.copy(), path),
                )
            }
            Self::Message(c, path) => {
                if integer(&event["chat_id"], 0) != c.chat || integer(&event["id"], 0) != c.message
                {
                    return Err("that file is not this message's media".into());
                }
                let body = crate::content_projection::content(&event["content"], &state.roots);
                if body["kind"] == "rich" && body["full"] != true {
                    native(
                        json!({"@type":"getFullRichMessage","chat_id":number(c.chat),"message_id":number(c.message)}),
                        Self::Rich(c.copy(), path.clone()),
                    )
                } else {
                    c.permission(&body, path.clone())?
                }
            }
            Self::Rich(c, path) => c.permission(
                &crate::rich_projection::rich_message(event, &state.roots),
                path.clone(),
            )?,
            Self::Permission(path, save, clipboard) => {
                if event["can_be_saved"] != true {
                    return Err("this message's media cannot be saved or copied".into());
                }
                if let Some(context) = clipboard {
                    Action::Work(crate::jobs::Work::CopyImage {
                        path: path.clone(),
                        roots: state.roots.clone(),
                        context: context.clone(),
                    })
                } else if let Some(save) = save {
                    Action::Work(crate::jobs::Work::Save {
                        path: path.clone(),
                        roots: state.roots.clone(),
                        destination: save.clone(),
                    })
                } else {
                    Action::Reply(json!({"approvedPath":path}))
                }
            }
        })
    }
}
pub fn project(value: &Value, roots: &[String]) -> Value {
    let state = State::new(roots.to_vec());
    let mut queries = Vec::new();
    let mut action = match prepare(&value["args"], value["photoOnly"] == true) {
        Ok(a) => a,
        Err(error) => return json!({"error":error}),
    };
    for event in value["answers"].as_array().into_iter().flatten() {
        let Action::Native {
            query,
            answer: Answer::Export(gate),
        } = action
        else {
            break;
        };
        queries.push(query);
        action = match gate.complete(event, &state) {
            Ok(a) => a,
            Err(error) => return json!({"error":error,"queries":queries}),
        };
    }
    match action {
        Action::Reply(result) => json!({"result":result,"queries":queries}),
        _ => json!({"incomplete":true,"queries":queries}),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_requires_original_membership_then_server_permission() {
        let state = State::new(vec!["/fixture/files".into()]);
        let gate = Gate::File(Context {
            chat: 42,
            message: 7,
            file: 3,
            photo: false,
            save: None,
            copy: None,
        });
        assert!(
            gate.complete(&json!({"@type":"file","id":4}), &state)
                .is_err()
        );
        let Action::Native{answer:Answer::Export(gate),..}=gate.complete(&json!({"@type":"file","id":3,"local":{"is_downloading_completed":true,"path":"/fixture/files/a"}}),&state).unwrap()else{panic!("gate")};
        assert!(
            gate.complete(&json!({"@type":"message","id":7,"chat_id":43}), &state)
                .is_err()
        );
        let denied = Gate::Permission("/fixture/files/a".into(), None, None);
        assert!(
            denied
                .complete(&json!({"can_be_saved":false}), &state)
                .is_err()
        );
        assert!(matches!(
            denied
                .complete(&json!({"can_be_saved":true}), &state)
                .unwrap(),
            Action::Reply(_)
        ));
    }
}
