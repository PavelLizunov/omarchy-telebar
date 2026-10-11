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
    let size = source
        .metadata()
        .map_err(|_| "the file could not be saved to Downloads")?
        .len();
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
            output.sync_all()?;
            directory.sync_all()
        })();
        if result.is_err() {
            // SAFETY: only exact entry exclusively created above.
            unsafe {
                libc::unlinkat(directory.as_raw_fd(), c.as_ptr(), 0);
            }
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
