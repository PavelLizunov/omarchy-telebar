//! Explicit private sticker cache. Finite gzip helper, no network or UI access.
use crate::{
    commands::{Action, Answer, Result},
    process,
    projection::{integer, number},
    state::State,
    storage::Storage,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
const DIRECTORY: &str = ".cache/omagram/lottie";
const INPUT: usize = 1024 * 1024;
const OUTPUT: usize = 16 * 1024 * 1024;
const COUNT: usize = 4096;
const TOTAL: u64 = 256 * 1024 * 1024;
#[derive(Debug)]
pub enum CacheAnswer {
    Lottie(PathBuf),
    Stats(PathBuf),
    Clear(PathBuf),
}
pub fn plan(command: &str, args: &Value, home: PathBuf) -> Option<Result<Action>> {
    let (query, answer) = match command {
        "sticker.lottie" => {
            let id = match crate::commands::arg_int(
                args,
                "fileId",
                Some(1),
                Some(i32::MAX as i128),
                None,
            ) {
                Ok(id) => id,
                Err(e) => return Some(Err(e)),
            };
            (
                json!({"@type":"getFile","file_id":number(id)}),
                CacheAnswer::Lottie(home),
            )
        }
        "storage.stats" => (
            json!({"@type":"getStorageStatisticsFast"}),
            CacheAnswer::Stats(home),
        ),
        "storage.clear" => (
            json!({"@type":"optimizeStorage","size":0,"ttl":0,"count":0,"immunity_delay":0,"file_types":[],"chat_ids":[],"exclude_chat_ids":[],"return_deleted_file_statistics":false,"chat_limit":0}),
            CacheAnswer::Clear(home),
        ),
        _ => return None,
    };
    Some(Ok(Action::Native {
        query,
        answer: Answer::Cache(answer),
    }))
}
#[derive(Debug)]
pub enum Job {
    Lottie {
        home: PathBuf,
        path: String,
        roots: Vec<String>,
    },
    Stats {
        home: PathBuf,
        native: Value,
    },
    Clear {
        home: PathBuf,
        remaining: i128,
    },
}
impl CacheAnswer {
    pub fn complete(&self, event: &Value, state: &State) -> Result<Action> {
        Ok(match self {
            Self::Lottie(home) => {
                let view = crate::projection::file_view(event, &state.roots);
                let path = view["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    Action::Reply(json!({"path":""}))
                } else {
                    Action::Work(crate::jobs::Work::Cache(Job::Lottie {
                        home: home.clone(),
                        path: path.into(),
                        roots: state.roots.clone(),
                    }))
                }
            }
            Self::Stats(home) => Action::Work(crate::jobs::Work::Cache(Job::Stats {
                home: home.clone(),
                native: json!({"filesSize":number(integer(&event["files_size"],0).max(0)),"fileCount":number(integer(&event["file_count"],0).max(0)),"databaseSize":number(integer(&event["database_size"],0).max(0)),"logSize":number(integer(&event["log_size"],0).max(0))}),
            })),
            Self::Clear(home) => Action::Work(crate::jobs::Work::Cache(Job::Clear {
                home: home.clone(),
                remaining: integer(&event["size"], 0).max(0),
            })),
        })
    }
}
fn entries(home: &Storage) -> Vec<(String, std::fs::Metadata)> {
    home.files(DIRECTORY, COUNT * 2)
        .unwrap_or_default()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".json"))
        .collect()
}
fn trim(home: &Storage, cancel: &AtomicBool) {
    let mut files = entries(home);
    let mut total: u64 = files.iter().map(|(_, m)| m.len()).sum();
    if total <= TOTAL && files.len() <= COUNT {
        return;
    }
    files.sort_by_key(|(_, m)| m.modified().ok());
    let mut count = files.len();
    for (name, m) in files {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if total <= TOTAL * 4 / 5 && count <= COUNT * 4 / 5 {
            break;
        }
        if home.remove_file(&format!("{DIRECTORY}/{name}")).is_ok() {
            total = total.saturating_sub(m.len());
            count -= 1;
        }
    }
}
fn inflate(
    home: &Path,
    path: &str,
    roots: &[String],
    cancel: &AtomicBool,
) -> std::io::Result<Option<PathBuf>> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let root = roots
        .iter()
        .filter(|r| path.starts_with(&format!("{}/", r.trim_end_matches('/'))))
        .max_by_key(|r| r.len())
        .ok_or_else(|| std::io::Error::other("invalid cache source"))?;
    let store = Storage::open(Path::new(root))?;
    let Some(blob) = store.read(&path[root.trim_end_matches('/').len() + 1..], INPUT)? else {
        return Ok(None);
    };
    if !blob.starts_with(b"\x1f\x8b") {
        return Ok(None);
    }
    let home_store = Storage::open(home)?;
    let hash = format!("{:x}", Sha256::digest(&blob));
    let target = format!("{DIRECTORY}/{hash}.json");
    if home_store
        .open_read(&target)
        .ok()
        .and_then(|f| f.metadata().ok())
        .is_some_and(|m| m.len() > 0 && m.len() <= OUTPUT as u64)
    {
        return Ok(Some(home.join(target)));
    }
    let output = process::run_cancel(
        "/usr/bin/gzip",
        &["-d", "-c"],
        &BTreeMap::new(),
        &blob,
        Duration::from_secs(10),
        OUTPUT,
        cancel,
    )?;
    if !output.ok()
        || !crate::wire::bounded_json(&output.stdout, 64, 2_000_000, 1024 * 1024)
            .is_ok_and(|v| v.is_object())
        || cancel.load(Ordering::Relaxed)
    {
        return Ok(None);
    }
    // Refuse new writes if a pre-existing overfull directory cannot be
    // inventoried within the finite ceiling; never turn that error into zero.
    home_store.files(DIRECTORY, COUNT * 2)?;
    home_store.write(&target, &output.stdout, 0o600)?;
    trim(&home_store, cancel);
    Ok(Some(home.join(target)))
}
impl Job {
    pub fn run(self, cancel: &AtomicBool) -> Result<Action> {
        if cancel.load(Ordering::Relaxed) {
            return Err("the connection was closed".into());
        }
        Ok(Action::Reply(match self {
            Self::Lottie { home, path, roots } => {
                json!({"path":inflate(&home,&path,&roots,cancel).ok().flatten().and_then(|p|p.to_str().map(str::to_owned)).unwrap_or_default()})
            }
            Self::Stats { home, mut native } => {
                let size = Storage::open(&home)
                    .ok()
                    .map(|s| entries(&s).iter().map(|(_, m)| m.len()).sum::<u64>())
                    .unwrap_or(0);
                native["stickerCacheSize"] = json!(size);
                native
            }
            Self::Clear { home, remaining } => {
                if let Ok(s) = Storage::open(&home) {
                    for (name, _) in entries(&s) {
                        if cancel.load(Ordering::Relaxed) {
                            return Err("the connection was closed".into());
                        }
                        let _ = s.remove_file(&format!("{DIRECTORY}/{name}"));
                    }
                }
                json!({"remaining":number(remaining)})
            }
        }))
    }
}
