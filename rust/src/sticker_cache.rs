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
    Note(PathBuf),
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
        "videonote.animation" => {
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
                json!({"@type":"downloadFile","file_id":number(id),"priority":32,"offset":0,"limit":0,"synchronous":true}),
                CacheAnswer::Note(home),
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
    Note {
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
            Self::Note(home) => {
                let view = crate::projection::file_view(event, &state.roots);
                let path = view["path"]
                    .as_str()
                    .filter(|p| !p.is_empty())
                    .ok_or("the round video could not be fetched")?;
                Action::Work(crate::jobs::Work::Cache(Job::Note {
                    home: home.clone(),
                    path: path.into(),
                    roots: state.roots.clone(),
                }))
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
            Self::Note { home, path, roots } => {
                let path = note_animation(&home, &path, &roots, cancel)
                    .map_err(|_| "that round video could not be shown")?;
                json!({"path":path,"fps":20})
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

fn note_animation(
    home: &Path,
    path: &str,
    roots: &[String],
    cancel: &AtomicBool,
) -> std::io::Result<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    use std::time::{Instant, SystemTime};
    let error = || std::io::Error::other("invalid note animation");
    if cancel.load(Ordering::Relaxed) {
        return Err(error());
    }
    let root = roots
        .iter()
        .filter(|r| path.starts_with(&format!("{}/", r.trim_end_matches('/'))))
        .max_by_key(|r| r.len())
        .ok_or_else(error)?;
    let source_store = Storage::open(Path::new(root))?;
    let source = source_store.open_read(&path[root.trim_end_matches('/').len() + 1..])?;
    let meta = source.metadata()?;
    if meta.len() == 0 || meta.len() > 64 * 1024 * 1024 {
        return Err(error());
    }
    let nanoseconds = i128::from(meta.mtime()) * 1_000_000_000 + i128::from(meta.mtime_nsec());
    let key = format!("{path}\0{}\0{nanoseconds}", meta.len());
    let hash = format!("{:x}", Sha256::digest(key.as_bytes()));
    let directory = ".cache/omagram/notes";
    let target = format!("{directory}/{hash}.webp");
    let store = Storage::open(home)?;
    if let Ok(file) = store.open_read(&target)
        && file
            .metadata()
            .is_ok_and(|m| m.len() > 0 && m.len() <= OUTPUT as u64)
    {
        // Refresh least-recently-wanted identity through the admitted descriptor.
        file.set_modified(SystemTime::now())?;
        return Ok(home.join(target));
    }
    store.files(directory, 2048)?;
    // Retain admitted source bytes and send only a private stable snapshot to
    // ffmpeg. No inherited descriptor or working-profile path goes to the helper.
    let mut source = source;
    let deadline = Instant::now() + Duration::from_secs(150);
    let copy = store.snapshot_upload(
        directory,
        &mut source,
        meta.len(),
        "source.mp4",
        cancel,
        deadline,
    )?;
    let relative = format!("{}/making.webp", copy.rsplit_once('/').ok_or_else(error)?.0);
    let output_path = home.join(&relative);
    let source_path = home.join(&copy);
    let mut args = crate::media::note_animation_argv(
        source_path.to_str().ok_or_else(error)?,
        output_path.to_str().ok_or_else(error)?,
    );
    // Disk output receives an explicit cap as well as a postcondition check.
    let output_index = args.len() - 1;
    args.splice(
        output_index..output_index,
        ["-fs".into(), OUTPUT.to_string()],
    );
    let converted = (|| {
        let out = process::run_cancel(
            "/usr/bin/ffmpeg",
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
            &BTreeMap::new(),
            &[],
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_secs(90)),
            64 * 1024,
            cancel,
        )?;
        if !out.ok() || cancel.load(Ordering::Relaxed) {
            return Err(error());
        }
        let bytes = store
            .read(&relative, OUTPUT)?
            .filter(|b| !b.is_empty())
            .ok_or_else(error)?;
        if !bytes.starts_with(b"RIFF")
            || bytes.get(8..12) != Some(b"WEBP")
            || bytes.len() >= OUTPUT
            || bytes
                .get(4..8)
                .map(|s| u32::from_le_bytes(s.try_into().expect("four bytes")) as usize + 8)
                != Some(bytes.len())
        {
            return Err(error());
        }
        store.write(&target, &bytes, 0o600)?;
        Ok(())
    })();
    let _ = store.remove_file(&copy);
    let _ = store.remove_file(&relative);
    if let Some((container, _)) = copy.rsplit_once('/') {
        let _ = store.remove_empty_directory(container);
    }
    converted?;
    let mut kept = store
        .files(directory, 2048)?
        .into_iter()
        .filter(|(n, _)| n.ends_with(".webp"))
        .collect::<Vec<_>>();
    kept.sort_by_key(|(_, m)| m.modified().ok());
    let mut total: u64 = kept.iter().map(|(_, m)| m.len()).sum();
    let mut count = kept.len();
    for (name, m) in kept {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let stale = m
            .modified()
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|d| d > Duration::from_secs(3600));
        if ((name.starts_with("making-") && stale) || total > 128 * 1024 * 1024 || count > 512)
            && store.remove_file(&format!("{directory}/{name}")).is_ok()
        {
            total = total.saturating_sub(m.len());
            count -= 1;
        }
    }
    Ok(home.join(target))
}
