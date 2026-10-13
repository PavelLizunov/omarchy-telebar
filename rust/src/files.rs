//! Owned export work after native membership/permission checks. Descriptor
//! traversal rejects symlinks; Downloads never overwrites an existing entry.
use crate::{
    commands::{Action, Result},
    storage::Storage,
};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use unicode_general_category::{GeneralCategory as C, get_general_category};
#[derive(Clone, Debug)]
pub struct Save {
    pub home: PathBuf,
    pub name: String,
}
pub fn name(value: &Value) -> String {
    let base = value.as_str().unwrap_or("").replace('\\', "/");
    let base = base.rsplit('/').next().unwrap_or("");
    let text: String = base
        .chars()
        .filter(|c| {
            *c != '/'
                && (*c == ' '
                    || !matches!(
                        get_general_category(*c),
                        C::Control
                            | C::Format
                            | C::Surrogate
                            | C::PrivateUse
                            | C::Unassigned
                            | C::SpaceSeparator
                            | C::LineSeparator
                            | C::ParagraphSeparator
                    ))
        })
        .take(200)
        .collect();
    let text = text.trim().trim_start_matches('.');
    if text.is_empty() {
        "file".into()
    } else {
        text.into()
    }
}
pub fn save(
    path: &str,
    roots: &[String],
    destination: Save,
    cancel: &AtomicBool,
) -> Result<Action> {
    save_with(path, roots, destination, cancel, &mut || {})
}
fn save_with(
    path: &str,
    roots: &[String],
    destination: Save,
    cancel: &AtomicBool,
    after_create: &mut dyn FnMut(),
) -> Result<Action> {
    let root = roots
        .iter()
        .filter(|r| path.starts_with(&format!("{}/", r.trim_end_matches('/'))))
        .max_by_key(|r| r.len())
        .ok_or("that file is not one of Telebar's")?;
    let store =
        Storage::open(Path::new(root)).map_err(|_| "the file could not be saved to Downloads")?;
    let mut source = store
        .open_read(&path[root.trim_end_matches('/').len() + 1..])
        .map_err(|_| "the file could not be saved to Downloads")?;
    let before = source
        .metadata()
        .map_err(|_| "the file could not be saved to Downloads")?;
    let size = before.len();
    if size > 2 * 1024 * 1024 * 1024 {
        return Err("the file could not be saved to Downloads".into());
    }
    let home =
        Storage::open(&destination.home).map_err(|_| "the file could not be saved to Downloads")?;
    let directory = home
        .create_directory("Downloads", 0o755)
        .map_err(|_| "the file could not be saved to Downloads")?;
    let name = destination.name;
    if name.is_empty() || name.contains(['/', '\\', '\0']) || matches!(name.as_str(), "." | "..") {
        return Err("the file could not be saved to Downloads".into());
    }
    let dot = name.rfind('.').filter(|n| *n > 0);
    let (stem, suffix) = dot.map_or((name.as_str(), ""), |n| (&name[..n], &name[n..]));
    let deadline = Instant::now() + Duration::from_secs(150);
    for n in 1..1000 {
        if cancel.load(Ordering::Relaxed) {
            return Err("the connection was closed".into());
        }
        let candidate = if n == 1 {
            name.clone()
        } else {
            format!("{stem} ({n}){suffix}")
        };
        let c = std::ffi::CString::new(candidate.as_str())
            .map_err(|_| "the file could not be saved to Downloads")?;
        // SAFETY: descriptor is pinned, component checked; O_EXCL never replaces.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                c.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o644,
            )
        };
        if fd < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::AlreadyExists {
                continue;
            }
            return Err("the file could not be saved to Downloads".into());
        }
        // SAFETY: unique new descriptor returned by openat.
        let mut output = unsafe { File::from_raw_fd(fd) };
        let created = output
            .metadata()
            .map_err(|_| "the file could not be saved to Downloads")?;
        let pinned = Storage::from_directory(
            directory
                .try_clone()
                .map_err(|_| "the file could not be saved to Downloads")?,
        )
        .map_err(|_| "the file could not be saved to Downloads")?;
        after_create();
        let result = (|| {
            let mut left = size;
            let mut bytes = vec![0; 1024 * 1024];
            while left > 0 {
                if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    return Err(std::io::Error::other("copy cancelled"));
                }
                let count = source.read(&mut bytes[..left.min(1024 * 1024) as usize])?;
                if count == 0 {
                    return Err(std::io::Error::other("file changed"));
                }
                output.write_all(&bytes[..count])?;
                left -= count as u64;
            }
            if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
                return Err(std::io::Error::other("copy cancelled"));
            }
            let after = source.metadata()?;
            if after.len() != before.len()
                || after.mtime() != before.mtime()
                || after.mtime_nsec() != before.mtime_nsec()
                || after.ctime() != before.ctime()
                || after.ctime_nsec() != before.ctime_nsec()
            {
                return Err(std::io::Error::other("export source changed"));
            }
            output.sync_all()?;
            let visible = Storage::open(&destination.home)?;
            let downloads = Storage::from_directory(visible.open_directory("Downloads")?)?;
            let current = downloads.open_read(&candidate)?.metadata()?;
            if !downloads.same_root(&pinned)?
                || current.dev() != created.dev()
                || current.ino() != created.ino()
            {
                return Err(std::io::Error::other("export output changed"));
            }
            directory.sync_all()
        })();
        if result.is_err() {
            // Cleanup belongs to the original Downloads descriptor/inode,
            // never a replacement name or reopened home pathname.
            let _ = pinned.remove_owned(&candidate, created.dev(), created.ino());
            return Err("the file could not be saved to Downloads".into());
        }
        let result = destination.home.join("Downloads").join(candidate);
        return Ok(Action::Reply(json!({"path":result})));
    }
    Err("the file could not be saved to Downloads".into())
}
pub fn project(value: &Value) -> Value {
    json!(name(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_cleanup_and_success_do_not_adopt_replaced_output_or_source() {
        let base =
            std::env::temp_dir().join(format!("telebar-export-owned-{}", std::process::id()));
        std::fs::create_dir(&base).unwrap();
        for kind in [
            "cancel-replacement",
            "output-replacement",
            "directory-replacement",
            "source-change",
            "ordinary",
        ] {
            let root = base.join(kind);
            std::fs::create_dir(&root).unwrap();
            let store = Storage::open(&root).unwrap();
            store
                .write("files/source", b"original media", 0o600)
                .unwrap();
            let cancel = AtomicBool::new(false);
            let destination = Save {
                home: root.clone(),
                name: "saved.txt".into(),
            };
            let out = save_with(
                root.join("files/source").to_str().unwrap(),
                &[root.join("files").to_str().unwrap().into()],
                destination,
                &cancel,
                &mut || {
                    if matches!(kind, "cancel-replacement" | "output-replacement") {
                        std::fs::rename(
                            root.join("Downloads/saved.txt"),
                            root.join("Downloads/old-output"),
                        )
                        .unwrap();
                        store
                            .write("Downloads/saved.txt", b"replacement data", 0o600)
                            .unwrap();
                        if kind == "cancel-replacement" {
                            cancel.store(true, Ordering::Relaxed);
                        }
                    } else if kind == "directory-replacement" {
                        std::fs::rename(root.join("Downloads"), root.join("original-downloads"))
                            .unwrap();
                        store
                            .write("Downloads/saved.txt", b"replacement data", 0o600)
                            .unwrap();
                    } else if kind == "source-change" {
                        std::fs::write(root.join("files/source"), b"changed").unwrap();
                    }
                },
            );
            if kind == "ordinary" {
                assert!(out.is_ok());
                assert_eq!(
                    std::fs::read(root.join("Downloads/saved.txt")).unwrap(),
                    b"original media"
                );
            } else {
                assert!(out.is_err(), "{kind}");
                if kind == "source-change" {
                    assert!(!root.join("Downloads/saved.txt").exists());
                    assert_eq!(
                        std::fs::read(root.join("files/source")).unwrap(),
                        b"changed"
                    );
                } else {
                    assert_eq!(
                        std::fs::read(root.join("Downloads/saved.txt")).unwrap(),
                        b"replacement data"
                    );
                    assert_eq!(
                        std::fs::read(root.join("files/source")).unwrap(),
                        b"original media"
                    );
                }
                if kind == "directory-replacement" {
                    assert!(!root.join("original-downloads/saved.txt").exists());
                }
            }
        }
        std::fs::remove_dir_all(base).unwrap();
    }
}
