//! Finite startup cleanup for legacy abandoned raw camera/clipboard files.
//! Never traverses symlinks, sent inputs, databases or retained recording paths.
use crate::storage::Storage;
use std::{
    io,
    path::Path,
    time::{Duration, SystemTime},
};
pub fn raw(runtime: &Path, protected: &[String], now: SystemTime) -> io::Result<usize> {
    let store = Storage::open(runtime)?;
    let mut removed = 0;
    for (name, meta) in store.files("omagram/rec", 4096)? {
        if !name.starts_with("note-") && !name.starts_with("paste-") {
            continue;
        }
        let relative = format!("omagram/rec/{name}");
        let path = runtime.join(&relative);
        if protected.iter().any(|p| Path::new(p) == path) {
            continue;
        }
        if meta
            .modified()
            .ok()
            .and_then(|time| now.duration_since(time).ok())
            .is_some_and(|age| age > Duration::from_secs(3600))
        {
            store.remove_file(&relative)?;
            removed += 1;
        }
    }
    Ok(removed)
}
