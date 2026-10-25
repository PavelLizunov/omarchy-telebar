//! Local-upload admission and exact Telegram content/album preparation.
//! Only explicitly chosen files are admitted, never account database content.
use crate::commands::Result;
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::{
    ffi::CString,
    fs::{self, File},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Component, Path, PathBuf},
};
#[derive(Debug)]
pub struct Upload {
    pub path: PathBuf,
    pub size: u64,
    pub file: File,
}
pub fn admit(value: &Value, databases: &[PathBuf]) -> Result<Upload> {
    let given = value
        .as_str()
        .filter(|s| s.starts_with('/') && s.chars().count() <= 4096 && !s.chars().any(|c| c < ' '))
        .ok_or("the file must be given as an absolute path")?;
    let path = fs::canonicalize(given).map_err(|_| "that file does not exist")?;
    admit_path(path, databases)
}
fn open_canonical(path: &Path) -> io::Result<File> {
    let mut parent = File::options()
        .read(true)
        .custom_flags(libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC)
        .open("/")?;
    let mut parts = path.components().skip(1).peekable();
    while let Some(part) = parts.next() {
        let Component::Normal(name) = part else {
            return Err(io::Error::other("invalid upload path"));
        };
        let name = CString::new(name.as_encoded_bytes())
            .map_err(|_| io::Error::other("invalid upload path"))?;
        let flags = if parts.peek().is_some() {
            libc::O_PATH | libc::O_DIRECTORY
        } else {
            libc::O_RDONLY | libc::O_NONBLOCK
        };
        // Canonicalization is not a lock: pin every ancestor without following
        // replacement links, not only the final selected filename.
        // SAFETY: parent is live, name is NUL-terminated, and flags never create a file.
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: openat returned a new descriptor; this File is its sole owner.
        parent = unsafe { File::from_raw_fd(fd) };
    }
    Ok(parent)
}
fn admit_path(path: PathBuf, databases: &[PathBuf]) -> Result<Upload> {
    let meta = fs::metadata(&path).map_err(|_| "that file does not exist")?;
    if !meta.is_file() {
        return Err("only regular files can be sent".into());
    }
    if databases.iter().any(|db| {
        path.starts_with(db) || fs::canonicalize(db).is_ok_and(|real| path.starts_with(real))
    }) {
        return Err("Telebar's own database is not sent".into());
    }
    let file = open_canonical(&path).map_err(|_| "that file is not readable")?;
    let opened = file.metadata().map_err(|_| "that file is not readable")?;
    if !opened.is_file() || opened.dev() != meta.dev() || opened.ino() != meta.ino() {
        return Err("that file does not exist".into());
    }
    if opened.len() == 0 {
        return Err("that file is empty".into());
    }
    if opened.len() > 2 * 1024 * 1024 * 1024 {
        return Err("Telegram accepts files up to 2 GB".into());
    }
    Ok(Upload {
        path,
        size: opened.len(),
        file,
    })
}
pub fn kind(path: &str, size: u64, as_media: bool) -> &'static str {
    let path = path.to_lowercase();
    if as_media {
        if [".jpg", ".jpeg", ".png", ".webp"]
            .iter()
            .any(|ext| path.ends_with(ext))
            && size <= 10 * 1024 * 1024
        {
            return "photo";
        }
        if [".mp4", ".mov", ".m4v"]
            .iter()
            .any(|ext| path.ends_with(ext))
        {
            return "video";
        }
        if [".mp3", ".m4a", ".flac", ".ogg", ".opus"]
            .iter()
            .any(|ext| path.ends_with(ext))
        {
            return "audio";
        }
    }
    "document"
}
pub fn content(path: &str, kind: &str, caption: Value, probed: [u64; 3]) -> Value {
    let upload = json!({"@type":"inputFileLocal","path":path});
    let [duration, width, height] = probed;
    match kind {
        "photo" => {
            json!({"@type":"inputMessagePhoto","caption":caption,"show_caption_above_media":false,"self_destruct_type":null,"has_spoiler":false,"photo":{"@type":"inputPhoto","photo":upload,"thumbnail":null,"video":null,"added_sticker_file_ids":[],"width":0,"height":0}})
        }
        "video" => {
            json!({"@type":"inputMessageVideo","caption":caption,"show_caption_above_media":false,"self_destruct_type":null,"has_spoiler":false,"video":{"@type":"inputVideo","video":upload,"thumbnail":null,"cover":null,"start_timestamp":0,"added_sticker_file_ids":[],"duration":duration,"width":width,"height":height,"supports_streaming":true}})
        }
        "audio" => {
            json!({"@type":"inputMessageAudio","caption":caption,"audio":{"@type":"inputAudio","audio":upload,"album_cover_thumbnail":null,"duration":duration,"title":"","performer":""}})
        }
        _ => {
            json!({"@type":"inputMessageDocument","caption":caption,"document":{"@type":"inputDocument","document":upload,"thumbnail":null,"disable_content_type_detection":false}})
        }
    }
}
pub fn groups(kinds: &[String]) -> Vec<Vec<usize>> {
    let mut buckets: IndexMap<&str, Vec<usize>> = IndexMap::new();
    for (i, kind) in kinds.iter().enumerate() {
        let group = if matches!(kind.as_str(), "photo" | "video") {
            "media"
        } else {
            kind
        };
        buckets.entry(group).or_default().push(i);
    }
    buckets
        .into_values()
        .flat_map(|v| v.chunks(10).map(<[usize]>::to_vec).collect::<Vec<_>>())
        .collect()
}
pub fn project(value: &Value) -> Value {
    let path = value["path"].as_str().unwrap_or("");
    match value["kind"].as_str() {
        Some("groups") => json!(groups(
            &value["kinds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        )),
        Some("content") => {
            let p = &value["probed"];
            content(
                path,
                value["fileKind"].as_str().unwrap_or("document"),
                value["caption"].clone(),
                [
                    p[0].as_u64().unwrap_or(0),
                    p[1].as_u64().unwrap_or(0),
                    p[2].as_u64().unwrap_or(0),
                ],
            )
        }
        _ => json!(kind(
            path,
            value["size"].as_u64().unwrap_or(0),
            value["asMedia"] == true
        )),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn changed_parent_after_canonicalization_cannot_expose_database() {
        let root = std::env::temp_dir().join(format!("telebar-upload-swap-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(root.join("database")).unwrap();
        fs::create_dir(root.join("chosen")).unwrap();
        fs::write(root.join("database/payload"), b"synthetic-private-database").unwrap();
        fs::write(root.join("chosen/payload"), b"chosen-public-file").unwrap();
        let resolved = fs::canonicalize(root.join("chosen/payload")).unwrap();
        // The directory owner swaps this ancestor after canonicalize, before open.
        fs::rename(root.join("chosen"), root.join("held")).unwrap();
        std::os::unix::fs::symlink(root.join("database"), root.join("chosen")).unwrap();
        let admitted = admit_path(resolved, &[root.join("database")]);
        fs::remove_dir_all(&root).unwrap();
        assert!(
            admitted.is_err(),
            "a changed parent bypassed database exclusion"
        );
    }
    #[test]
    fn upload_does_not_admit_database_even_through_symlink() {
        let root = std::env::temp_dir().join(format!("telebar-upload-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(root.join("database")).unwrap();
        fs::write(root.join("database/private"), b"synthetic").unwrap();
        std::os::unix::fs::symlink(root.join("database/private"), root.join("link")).unwrap();
        let rejected = admit(&json!(root.join("link")), &[root.join("database")]);
        assert_eq!(
            rejected.err().unwrap(),
            "Telebar's own database is not sent"
        );
        fs::write(root.join("chosen.txt"), b"synthetic").unwrap();
        std::os::unix::fs::symlink(root.join("chosen.txt"), root.join("chosen-link")).unwrap();
        assert_eq!(
            admit(&json!(root.join("chosen-link")), &[root.join("database")])
                .unwrap()
                .path,
            root.join("chosen.txt")
        );
        assert_eq!(
            admit(&json!(root.join("chosen.txt")), &[root.join("database")])
                .unwrap()
                .size,
            9
        );
        fs::remove_dir_all(root).unwrap();
    }
}
