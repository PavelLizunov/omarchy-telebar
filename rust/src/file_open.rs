//! Downloaded-file opener, admitted native identity and descriptor-pinned input.
use crate::{
    commands::{Action, Answer, Result, arg_int},
    process::Managed,
    projection::{integer, local_path, number},
    state::State,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
#[derive(Clone, Debug)]
pub struct Selection {
    file: i128,
    environment: BTreeMap<String, String>,
}
#[derive(Debug)]
pub struct Start {
    path: String,
    roots: Vec<String>,
    environment: BTreeMap<String, String>,
}
impl Selection {
    pub fn prepare(args: &Value, environment: BTreeMap<String, String>) -> Result<Action> {
        let file = arg_int(args, "fileId", Some(1), Some(i32::MAX as i128), None)?;
        Ok(Action::Native {
            query: json!({"@type":"getFile","file_id":number(file)}),
            answer: Answer::OpenFile(Self { file, environment }),
        })
    }
    pub fn complete(&self, event: &Value, state: &State) -> Result<Action> {
        if integer(&event["id"], 0) != self.file {
            return Err("that is not the requested file".into());
        }
        if event["local"]["is_downloading_completed"] != true {
            return Err("the file is not downloaded yet".into());
        }
        let path = local_path(&event["local"]["path"], &state.roots);
        if path.is_empty() {
            return Err("that file is not one of Telebar's".into());
        }
        Ok(Action::OpenFile(Start {
            path,
            roots: state.roots.clone(),
            environment: self.environment.clone(),
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
            .ok_or("that file is not one of Telebar's")?;
        let file = crate::storage::Storage::open(Path::new(root))
            .and_then(|s| s.open_read(&self.path[root.trim_end_matches('/').len() + 1..]))
            .map_err(|_| "that file is not one of Telebar's")?;
        use std::os::fd::AsRawFd;
        // xdg-open can wait for the player to close. Keep ownership and the
        // pinned input until it exits, without imposing a viewing deadline.
        Managed::start_owned(
            "/usr/bin/xdg-open",
            &[&format!(
                "/proc/{}/fd/{}",
                std::process::id(),
                file.as_raw_fd()
            )],
            &self.environment,
            &[],
            4096,
        )
        .map(|helper| (helper, file))
        .map_err(|_| "the file could not be opened".into())
    }
}
