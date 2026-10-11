//! Admitted playback selection and foreground helper; never opens a UI window.
use crate::{
    commands::{Action, Answer, Result, arg_int},
    process::Managed,
    projection::{file_view, number},
    state::State,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, time::Duration};
#[derive(Debug)]
pub struct Selection {
    pub serial: u64,
    pub chat: i128,
    pub message: i128,
    pub file: i128,
    pub rate: f64,
    pub environment: BTreeMap<String, String>,
}
#[derive(Debug)]
pub struct Start {
    pub selection: Selection,
    pub path: String,
    pub roots: Vec<String>,
}
impl Selection {
    pub fn prepare(
        args: &Value,
        serial: u64,
        rate: f64,
        environment: BTreeMap<String, String>,
    ) -> Result<Action> {
        if !rate.is_finite() || !(0.5..=2.0).contains(&rate) {
            return Err("the player could not start".into());
        }
        let selection = Self {
            serial,
            chat: arg_int(args, "chatId", None, None, None)?,
            message: arg_int(args, "messageId", Some(1), None, None)?,
            file: arg_int(args, "fileId", Some(1), Some(i32::MAX as i128), None)?,
            rate,
            environment,
        };
        Ok(Action::Native {
            query: json!({"@type":"downloadFile","file_id":number(selection.file),"priority":32,"offset":0,"limit":0,"synchronous":true}),
            answer: Answer::Playback(selection),
        })
    }
    pub fn complete(&self, event: &Value, state: &State) -> Result<Action> {
        let view = file_view(event, &state.roots);
        let path = view["path"]
            .as_str()
            .filter(|p| !p.is_empty())
            .ok_or("the message could not be fetched")?;
        if crate::projection::integer(&event["id"], 0) != self.file {
            return Err("the message could not be fetched".into());
        }
        Ok(Action::Playback(Start {
            selection: Self {
                serial: self.serial,
                chat: self.chat,
                message: self.message,
                file: self.file,
                rate: self.rate,
                environment: self.environment.clone(),
            },
            path: path.into(),
            roots: state.roots.clone(),
        }))
    }
}
impl Start {
    pub fn spawn(&self) -> Result<(Managed, std::fs::File)> {
        let root = self
            .roots
            .iter()
            .filter(|r| {
                self.path
                    .starts_with(&format!("{}/", r.trim_end_matches('/')))
            })
            .max_by_key(|r| r.len())
            .ok_or("the message could not be fetched")?;
        let source = crate::storage::Storage::open(Path::new(root))
            .and_then(|s| s.open_read(&self.path[root.trim_end_matches('/').len() + 1..]))
            .map_err(|_| "the message could not be fetched")?;
        let mut args = vec![
            "-nodisp".into(),
            "-vn".into(),
            "-sn".into(),
            "-autoexit".into(),
            "-loglevel".into(),
            "quiet".into(),
        ];
        if self.selection.rate != 1.0 {
            args.extend(["-af".into(), format!("atempo={}", self.selection.rate)]);
        }
        // Keep the admitted descriptor alive until the foreground player ends.
        // Child opens the parent's pinned inode, not a replaceable source path.
        use std::os::fd::AsRawFd;
        args.push(format!(
            "/proc/{}/fd/{}",
            std::process::id(),
            source.as_raw_fd()
        ));
        Managed::start(
            "/usr/bin/ffplay",
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
            &self.selection.environment,
            &[],
            Duration::from_secs(900),
            64 * 1024,
        )
        .map(|helper| (helper, source))
        .map_err(|_| "the player could not start".into())
    }
}
