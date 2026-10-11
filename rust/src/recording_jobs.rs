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
        let _ = owned_input(path, &rec).map_err(|_| "that is not a recording of Telebar's")?;
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("that is not a recording of Telebar's")?;
        Storage::open(&rec)
            .and_then(|s| s.remove_file(name))
            .map_err(|_| "that is not a recording of Telebar's")?;
        return Ok(Action::Reply(json!({})));
    }
    if command == "recording.discard" {
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
        if record.status != Status::Unknown {
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
                match storage.open_read(relative) {
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => return Err("the retained recording is not available".into()),
                }
                files.push((storage, relative.to_owned()));
            }
            for (storage, relative) in files {
                storage
                    .remove_file(&relative)
                    .map_err(|_| "the retained recording is not available")?;
            }
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
        }),
        preparation: Box::new(Preparation {
            source,
            home,
            account: account.into(),
            kind: "video".into(),
            token,
            options,
        }),
    })
}
impl Preparation {
    pub fn run(mut self, cancel: &AtomicBool) -> Result<Action> {
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
        let source = self.home.join(&copied);
        let source = source
            .to_str()
            .ok_or("the retained recording is not available")?;
        let content = if self.kind == "voice" {
            let(seconds,waveform)=crate::media::prepare_voice_cancel(source,cancel).map_err(|_|"The recording could not be prepared; it was kept for explicit retry or discard")?;
            if seconds < 1 {
                return Err("The recording could not be prepared; it was kept for explicit retry or discard".into());
            }
            json!({"@type":"inputMessageVoiceNote","voice_note":{"@type":"inputVoiceNote","voice_note":{"@type":"inputFileLocal","path":source},"duration":seconds,"waveform":waveform},"caption":null,"self_destruct_type":null})
        } else {
            // Exclusive input directory holds the converted output and retained
            // source. Neither path is deleted after uncertain native delivery.
            let source_path = self.home.join(copied);
            let name = format!("converted-{}.mp4", self.token);
            let name = if source_path.file_name().is_some_and(|n| n == name.as_str()) {
                format!("converted-output-{}.mp4", self.token)
            } else {
                name
            };
            let target = source_path.with_file_name(name);
            let target = target
                .to_str()
                .ok_or("the retained recording is not available")?;
            let seconds = crate::media::prepare_video_note_until(source, target, cancel, deadline)
                .map_err(|_| "The recording could not be prepared; it was kept for explicit retry or discard")?;
            if seconds < 1.0 {
                return Err("The recording could not be prepared; it was kept for explicit retry or discard".into());
            }
            json!({"@type":"inputMessageVideoNote","video_note":{"@type":"inputVideoNote","video_note":{"@type":"inputFileLocal","path":target},"thumbnail":null,"duration":(seconds as u64).clamp(1,60),"length":480},"self_destruct_type":null})
        };
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return Err("the connection was closed".into());
        }
        Ok(Action::Recorded {
            token: self.token,
            content,
            options: self.options,
        })
    }
}
