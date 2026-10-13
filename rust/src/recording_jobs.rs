//! Retained media preparation over explicit owned files, not capture devices.
use crate::{
    commands::{Action, Result},
    storage::Storage,
    uploads::Upload,
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
#[derive(Debug)]
pub struct Preparation {
    pub source: Upload,
    pub home: PathBuf,
    pub account: String,
    pub kind: String,
    pub token: String,
    pub options: Value,
    pub capture: Option<Box<crate::capture::Capture>>,
}
fn owned_input(path: &str, root: &std::path::Path) -> Result<Upload> {
    use std::os::unix::fs::MetadataExt;
    let path = std::path::Path::new(path);
    let relative = path
        .strip_prefix(root)
        .ok()
        .and_then(|p| p.to_str())
        .ok_or("the retained recording is not available")?;
    let file = Storage::open(root)
        .and_then(|s| s.open_read(relative))
        .map_err(|_| "the retained recording is not available")?;
    let meta = file
        .metadata()
        .map_err(|_| "the retained recording is not available")?;
    if meta.uid() != unsafe { libc::getuid() } || meta.len() == 0 || meta.len() > 512 * 1024 * 1024
    {
        return Err("the retained recording is not available".into());
    }
    Ok(Upload {
        path: path.into(),
        size: meta.len(),
        file,
    })
}
fn remove_input(source: Upload, root: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let relative = source
        .path
        .strip_prefix(root)
        .ok()
        .and_then(|p| p.to_str())
        .ok_or_else(|| std::io::Error::other("invalid recording input"))?;
    let m = source.file.metadata()?;
    let removed = Storage::open(root)?.remove_owned(relative, m.dev(), m.ino())?;
    if !removed {
        return Err(std::io::Error::other("recording input changed"));
    }
    Ok(())
}
fn remove_collected(files: &[(Storage, String, Option<std::fs::File>)]) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    // Keep every descriptor alive through the entire deletion batch. Dev/inode
    // numbers without the open file can be recycled after ordinary unlink.
    for (storage, relative, file) in files {
        if let Some(file) = file {
            let m = file.metadata()?;
            storage.remove_owned(relative, m.dev(), m.ino())?;
        }
    }
    Ok(())
}
pub fn plan(
    command: &str,
    args: &Value,
    state: &crate::state::State,
    home: PathBuf,
    runtime: &std::path::Path,
    account: &str,
    now: f64,
) -> Result<Action> {
    use crate::recorded_send::{Recording, Status};
    if !crate::paths::valid_account_id(account) {
        return Err("unknown account".into());
    }
    if command == "videonote.discard" {
        let path = args["path"]
            .as_str()
            .filter(|p| p.len() <= 4096)
            .ok_or("that is not a recording of Telebar's")?;
        let rec = runtime.join("omagram/rec");
        if std::path::Path::new(path).parent() != Some(rec.as_path()) {
            return Err("that is not a recording of Telebar's".into());
        }
        if state
            .retained
            .record
            .as_ref()
            .is_some_and(|r| r.path == path || r.source.as_deref() == Some(path))
        {
            return Err("the recording is owned by a pending send; resolve it by token".into());
        }
        let source = owned_input(path, &rec).map_err(|_| "that is not a recording of Telebar's")?;
        remove_input(source, &rec).map_err(|_| "that is not a recording of Telebar's")?;
        return Ok(Action::Reply(json!({})));
    }
    if command == "recording.discard" {
        if state.retained.recovery_blocked() && args["token"] == "0".repeat(32) {
            return Ok(Action::DiscardRecorded("0".repeat(32)));
        }
        let token = args["token"]
            .as_str()
            .ok_or("that recorded message is no longer available")?;
        let record = state
            .retained
            .record
            .as_ref()
            .filter(|r| r.token == token)
            .ok_or("that recorded message is no longer available")?;
        if matches!(record.status, Status::Preparing | Status::Sending) {
            return Err("the recorded message is still being prepared or sent".into());
        }
        if record.status != Status::Unknown && !state.retained.has_checkpoint() {
            let sent = if account == "default" {
                home.join(".local/share/omagram/sent")
            } else {
                home.join(format!(".local/share/omagram/accounts/{account}/sent"))
            };
            let rec = runtime.join("omagram/rec");
            // Validate every owned root before removing any entry. Never use
            // this path for unknown native outcomes or arbitrary chosen files.
            let mut files = Vec::new();
            for path in std::iter::once(&record.path).chain(record.source.as_ref()) {
                let path = std::path::Path::new(path);
                let root = [&sent, &rec]
                    .into_iter()
                    .find(|root| path.starts_with(root))
                    .ok_or("the retained recording is not available")?;
                let relative = path
                    .strip_prefix(root)
                    .ok()
                    .and_then(|p| p.to_str())
                    .ok_or("the retained recording is not available")?;
                let storage =
                    Storage::open(root).map_err(|_| "the retained recording is not available")?;
                let file = match storage.open_read(relative) {
                    Ok(file) => Some(file),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(_) => return Err("the retained recording is not available".into()),
                };
                files.push((storage, relative.to_owned(), file));
            }
            if let Some(input) = &record.input {
                input.rejected(400);
            }
            remove_collected(&files).map_err(|_| "the retained recording is not available")?;
        }
        return Ok(Action::DiscardRecorded(token.into()));
    }
    if command == "recording.retry" {
        let token = args["token"]
            .as_str()
            .ok_or("that recorded message is no longer available")?;
        let record = state
            .retained
            .record
            .as_ref()
            .filter(|r| r.token == token)
            .ok_or("that recorded message is no longer available")?;
        if !matches!(record.status, Status::Rejected | Status::PrepareFailed) {
            return Err(
                "retry requires a confirmed rejection; this message may still be sent".into(),
            );
        }
        let path = if record.status == Status::Rejected {
            &record.path
        } else {
            record.source.as_deref().unwrap_or(&record.path)
        };
        let source = owned_input(path, runtime).or_else(|_| {
            let sent = if account == "default" {
                home.join(".local/share/omagram/sent")
            } else {
                home.join(format!(".local/share/omagram/accounts/{account}/sent"))
            };
            owned_input(path, &sent)
        })?;
        let options = crate::sending::options(&record.args, state, now)?;
        return if record.status == Status::Rejected {
            Ok(Action::Recorded {
                token: token.into(),
                content: record
                    .content
                    .clone()
                    .ok_or("recording content is not prepared")?,
                options,
                input: None,
            })
        } else {
            Ok(Action::PrepareRecorded {
                recording: None,
                preparation: Box::new(Preparation {
                    source,
                    home,
                    account: account.into(),
                    kind: record.kind.clone(),
                    token: token.into(),
                    options,
                    capture: None,
                }),
            })
        };
    }
    if command != "videonote.send" {
        return Err("unknown command".into());
    }
    if state.retained.record.is_some() {
        return Err("a recorded message is still pending; resolve or discard it first".into());
    }
    let chat = crate::commands::arg_int(args, "chatId", None, None, None)?;
    let path = args["path"]
        .as_str()
        .filter(|p| p.len() <= 4096)
        .ok_or("that is not a recording of Telebar's")?;
    if std::path::Path::new(path).parent() != Some(runtime.join("omagram/rec").as_path()) {
        return Err("that is not a recording of Telebar's".into());
    }
    let source = owned_input(path, runtime).map_err(|_| "that is not a recording of Telebar's")?;
    crate::commands::topic(args)?;
    crate::commands::reply(args)?;
    let options = crate::sending::options(args, state, now)?;
    let mut bytes = [0u8; 16];
    use std::io::Read;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| "the request failed")?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let picked: serde_json::Map<String, Value> = [
        "replyToMessageId",
        "topicId",
        "threadId",
        "silent",
        "sendAt",
    ]
    .into_iter()
    .filter_map(|k| args.get(k).map(|v| (k.into(), v.clone())))
    .collect();
    Ok(Action::PrepareRecorded {
        recording: Some(Recording {
            token: token.clone(),
            kind: "video".into(),
            path: path.into(),
            source: Some(path.into()),
            chat,
            args: Value::Object(picked),
            content: None,
            status: Status::Preparing,
            input: None,
        }),
        preparation: Box::new(Preparation {
            source,
            home,
            account: account.into(),
            kind: "video".into(),
            token,
            options,
            capture: None,
        }),
    })
}
impl Preparation {
    pub fn run(mut self, cancel: &AtomicBool) -> Result<Action> {
        if let Some(mut capture) = self.capture.take() {
            capture.finish(cancel)?;
            capture.verify_input(&self.source.file)?;
            self.source.size = self
                .source
                .file
                .metadata()
                .map_err(|_| "the retained recording is not available")?
                .len();
            if self.source.size == 0 || self.source.size > 512 * 1024 * 1024 {
                return Err("The recording could not be prepared; it was kept for explicit retry or discard".into());
            }
        }
        if !crate::paths::valid_account_id(&self.account)
            || !matches!(self.kind.as_str(), "voice" | "video")
            || self.token.len() != 32
            || !self.token.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid retained recording".into());
        }
        let home = Storage::open(&self.home).map_err(
            |_| "The recording could not be prepared; it was kept for explicit retry or discard",
        )?;
        let relative = if self.account == "default" {
            ".local/share/omagram/sent".into()
        } else {
            format!(".local/share/omagram/accounts/{}/sent", self.account)
        };
        let deadline = Instant::now() + Duration::from_secs(150);
        let name = self
            .source
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("the retained recording is not available")?;
        let copied = home.snapshot_upload(&relative, &mut self.source.file, self.source.size, name, cancel, deadline)
            .map_err(|_| "The recording could not be prepared; it was kept for explicit retry or discard")?;
        let mut lease = crate::upload_lease::Lease::new(self.home.clone())
            .map_err(|_| "the recording input could not be retained")?;
        lease
            .add(copied.clone())
            .map_err(|_| "the recording input could not be retained")?;
        let source_path = self.home.join(&copied);
        let input = home
            .open_read(&copied)
            .map_err(|_| "the retained recording is not available")?;
        use std::os::fd::AsRawFd;
        let descriptor = format!("/proc/{}/fd/{}", std::process::id(), input.as_raw_fd());
        let source = source_path
            .to_str()
            .ok_or("the retained recording is not available")?;
        let content = if self.kind == "voice" {
            let(seconds,waveform)=crate::media::prepare_voice_cancel(&descriptor,cancel).map_err(|_|"The recording could not be prepared; it was kept for explicit retry or discard")?;
            if seconds < 1 {
                return Err("The recording could not be prepared; it was kept for explicit retry or discard".into());
            }
            json!({"@type":"inputMessageVoiceNote","voice_note":{"@type":"inputVoiceNote","voice_note":{"@type":"inputFileLocal","path":source},"duration":seconds,"waveform":waveform},"caption":null,"self_destruct_type":null})
        } else {
            // Exclusive input directory holds the converted output and retained
            // source. Neither path is deleted after uncertain native delivery.
            let source_path = self.home.join(&copied);
            let name = format!("converted-{}.mp4", self.token);
            let name = if source_path.file_name().is_some_and(|n| n == name.as_str()) {
                format!("converted-output-{}.mp4", self.token)
            } else {
                name
            };
            let target = source_path.with_file_name(name);
            let relative = target
                .strip_prefix(&self.home)
                .ok()
                .and_then(|p| p.to_str())
                .ok_or("invalid recording output")?;
            // Reserve the bounded converter output while holding the shared
            // input-directory lock. Both fds/journal exist before ffmpeg writes.
            let _reservation = crate::upload_lease::reserve(
                &home,
                relative
                    .rsplit_once('/')
                    .and_then(|(p, _)| p.rsplit_once('/'))
                    .map(|(p, _)| p)
                    .ok_or("invalid recording output")?,
                512 * 1024 * 1024,
                cancel,
                deadline,
            )
            .map_err(
                |_| "the saved recording input budget is exhausted; existing inputs were kept",
            )?;
            let output = home
                .create_file(relative)
                .map_err(|_| "the recording output could not be created")?;
            lease
                .add(relative.to_owned())
                .map_err(|_| "the recording output could not be retained")?;
            let target = target
                .to_str()
                .ok_or("the retained recording is not available")?;
            let seconds = crate::media::prepare_video_note_fd_until(&descriptor, &output, cancel, deadline)
                .map_err(|_| "The recording could not be prepared; it was kept for explicit retry or discard")?;
            if seconds < 1.0 {
                return Err("The recording could not be prepared; it was kept for explicit retry or discard".into());
            }
            // Conversion consumed this generated scratch copy. The admitted
            // raw original remains for retry; TDLib receives only the output.
            lease
                .consumed(&copied)
                .map_err(|_| "the recording scratch input could not be removed")?;
            json!({"@type":"inputMessageVideoNote","video_note":{"@type":"inputVideoNote","video_note":{"@type":"inputFileLocal","path":target},"thumbnail":null,"duration":(seconds as u64).clamp(1,60),"length":480},"self_destruct_type":null})
        };
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return Err("the connection was closed".into());
        }
        // Move cleanup ownership through the actual completion. A dropped or
        // stale worker result must not leak inputs before native admission.
        Ok(Action::Recorded {
            token: self.token,
            content,
            options: self.options,
            input: Some(lease),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_discard_never_unlinks_a_file_substituted_after_admission() {
        let root = std::env::temp_dir().join(format!("telebar-raw-inode-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let store = Storage::open(&root).unwrap();
        store.write("note.mp4", b"original media", 0o600).unwrap();
        let source = owned_input(root.join("note.mp4").to_str().unwrap(), &root).unwrap();
        std::fs::rename(root.join("note.mp4"), root.join("original.mp4")).unwrap();
        store
            .write("note.mp4", b"replacement media", 0o600)
            .unwrap();
        assert!(remove_input(source, &root).is_err());
        assert_eq!(
            std::fs::read(root.join("note.mp4")).unwrap(),
            b"replacement media"
        );
        assert_eq!(
            std::fs::read(root.join("original.mp4")).unwrap(),
            b"original media"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn fallback_discard_pins_unlinked_and_missing_inputs_until_batch_finishes() {
        use std::os::unix::fs::MetadataExt;
        let root =
            std::env::temp_dir().join(format!("telebar-fallback-inode-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let store = Storage::open(&root).unwrap();
        for name in ["first", "second", "ordinary"] {
            store.write(name, b"original media", 0o600).unwrap();
        }
        let files = ["first", "second", "ordinary", "missing"]
            .into_iter()
            .map(|name| {
                let owned = Storage::open(&root).unwrap();
                let file = match owned.open_read(name) {
                    Ok(file) => Some(file),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(e) => panic!("{e}"),
                };
                (owned, name.to_owned(), file)
            })
            .collect::<Vec<_>>();
        for index in [0, 1] {
            let before = files[index].2.as_ref().unwrap().metadata().unwrap();
            std::fs::remove_file(root.join(&files[index].1)).unwrap();
            store
                .write(&files[index].1, b"foreign replacement", 0o600)
                .unwrap();
            let old = files[index].2.as_ref().unwrap().metadata().unwrap();
            let replacement = store
                .open_read(&files[index].1)
                .unwrap()
                .metadata()
                .unwrap();
            assert_eq!(old.ino(), before.ino());
            assert_eq!(old.nlink(), 0);
            assert_ne!(
                (replacement.dev(), replacement.ino()),
                (old.dev(), old.ino()),
                "open inode must not be reused"
            );
        }
        store.write("missing", b"new foreign input", 0o600).unwrap();
        remove_collected(&files).unwrap();
        for name in ["first", "second"] {
            assert_eq!(
                std::fs::read(root.join(name)).unwrap(),
                b"foreign replacement"
            );
        }
        assert_eq!(
            std::fs::read(root.join("missing")).unwrap(),
            b"new foreign input"
        );
        assert!(!root.join("ordinary").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
