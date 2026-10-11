//! Bounded multi-query command assembly. Aggregate owners retain input order;
//! local cancellation retires siblings, never retries native operations.
use crate::{
    commands::{Action, Answer, Result, arg_int},
    projection::{integer, number},
    state::State,
};
use indexmap::IndexMap;
use serde_json::{Value, json};
#[derive(Debug)]
pub enum Aggregate {
    Settings { field: &'static str },
    Uploads { groups: usize },
    Folders { main_position: i128 },
    MediaCounts { chat: i128 },
}
#[derive(Debug)]
pub struct Batch {
    pub queries: Vec<(String, Value, Answer)>,
    pub aggregate: Aggregate,
}
impl Aggregate {
    pub fn complete(&self, values: IndexMap<String, Option<Value>>) -> Result<Value> {
        Ok(match self {
            Self::Uploads { groups } => {
                if let Some(error) = values
                    .values()
                    .flatten()
                    .find_map(|v| v["uploadError"].as_str())
                {
                    return Err(error.into());
                }
                if values.values().any(Option::is_none) {
                    return Err("Telegram did not take the files".into());
                }
                json!({"groups":groups})
            }
            Self::Settings { field } => {
                let mut result = json!({});
                let mut settings = json!({});
                for (key, value) in values {
                    settings[key] = value.unwrap_or(Value::Null);
                }
                result[field] = settings;
                result
            }
            Self::Folders { main_position } => {
                json!({"folders":values.into_values().flatten().filter(|v|!v.is_null()).collect::<Vec<_>>(),"mainPosition":number(*main_position)})
            }
            Self::MediaCounts { chat } => {
                if values.values().any(Option::is_none) {
                    return Err("Telegram did not return all media counts; try again".into());
                }
                let mut counts = json!({});
                for (key, value) in values {
                    counts[key] = value.unwrap_or(Value::Null);
                }
                json!({"chatId":number(*chat),"counts":counts})
            }
        })
    }
}
pub const SCOPES: [(&str, &str); 3] = [
    ("private", "notificationSettingsScopePrivateChats"),
    ("groups", "notificationSettingsScopeGroupChats"),
    ("channels", "notificationSettingsScopeChannelChats"),
];
pub const PRIVACY: [(&str, &str); 9] = [
    ("status", "userPrivacySettingShowStatus"),
    ("photo", "userPrivacySettingShowProfilePhoto"),
    ("phone", "userPrivacySettingShowPhoneNumber"),
    ("findByPhone", "userPrivacySettingAllowFindingByPhoneNumber"),
    ("bio", "userPrivacySettingShowBio"),
    ("birthdate", "userPrivacySettingShowBirthdate"),
    ("forwards", "userPrivacySettingShowLinkInForwardedMessages"),
    ("calls", "userPrivacySettingAllowCalls"),
    ("invites", "userPrivacySettingAllowChatInvites"),
];
pub fn plan(command: &str, args: &Value, state: &State) -> Result<Action> {
    let batch = match command {
        "notifications.get" => Batch {
            queries: SCOPES
                .into_iter()
                .map(|(key, kind)| {
                    (
                        key.into(),
                        json!({"@type":"getScopeNotificationSettings","scope":{"@type":kind}}),
                        Answer::Scope,
                    )
                })
                .collect(),
            aggregate: Aggregate::Settings { field: "scopes" },
        },
        "privacy.get" => Batch {
            queries: PRIVACY
                .into_iter()
                .map(|(key, kind)| {
                    (
                        key.into(),
                        json!({"@type":"getUserPrivacySettingRules","setting":{"@type":kind}}),
                        Answer::Privacy,
                    )
                })
                .collect(),
            aggregate: Aggregate::Settings { field: "settings" },
        },
        "folders.get" => Batch {
            queries: state
                .folders
                .iter()
                .map(|f| {
                    let id = integer(&f["id"], 0);
                    (
                        id.to_string(),
                        json!({"@type":"getChatFolder","chat_folder_id":number(id)}),
                        Answer::Folder(id),
                    )
                })
                .collect(),
            aggregate: Aggregate::Folders {
                main_position: state.main_position,
            },
        },
        "chat.mediaCounts" => {
            let chat = arg_int(args, "chatId", None, None, None)?;
            Batch{queries:crate::chat_commands::MEDIA_FILTERS.into_iter().map(|(key,kind)|(key.into(),json!({"@type":"getChatMessageCount","chat_id":number(chat),"topic_id":null,"filter":{"@type":kind},"return_local":false}),Answer::Count)).collect(),aggregate:Aggregate::MediaCounts{chat}}
        }
        _ => return Err("migration command is not implemented".into()),
    };
    if batch.queries.is_empty() {
        return Ok(Action::Reply(batch.aggregate.complete(IndexMap::new())?));
    }
    Ok(Action::Batch(batch))
}

pub fn fixture(batch: Batch, answers: &Value, state: &State) -> (Vec<Value>, Result<Value>) {
    let mut queries = Vec::new();
    let mut values = IndexMap::new();
    for (index, (key, query, answer)) in batch.queries.into_iter().enumerate() {
        queries.push(query);
        let event = answers
            .as_array()
            .and_then(|a| a.get(index))
            .unwrap_or(&Value::Null);
        let value = if event["@type"] == "error" {
            if matches!(batch.aggregate, Aggregate::Uploads { .. }) {
                Some(json!({"uploadError":crate::projection::text(&event["message"],256)}))
            } else {
                None
            }
        } else {
            answer.complete(event, state).ok().map(|c| c.result)
        };
        values.insert(key, value);
    }
    let result = batch.aggregate.complete(values);
    (queries, result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folder_results_keep_input_order_and_counts_fail_as_one() {
        let state = State::default();
        let Action::Batch(b) = plan("chat.mediaCounts", &json!({"chatId":42}), &state).unwrap()
        else {
            panic!("batch")
        };
        let answers =
            json!([{"count":1},{"@type":"error"},{"count":3},{"count":4},{"count":5},{"count":6}]);
        assert!(fixture(b, &answers, &state).1.is_err());
        let mut values = IndexMap::new();
        values.insert("10".into(), Some(json!({"id":10})));
        values.insert("2".into(), None);
        values.insert("3".into(), Some(json!({"id":3})));
        assert_eq!(
            Aggregate::Folders { main_position: 1 }
                .complete(values)
                .unwrap(),
            json!({"folders":[{"id":10},{"id":3}],"mainPosition":1})
        );
    }
}
