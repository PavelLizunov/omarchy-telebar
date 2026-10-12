//! One explicitly owned recorder. Production inputs are fixed, not IPC argv.
use crate::{
    commands::{Action, Result},
    process::Managed,
    storage::Storage,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Debug)]
pub struct Context {
    pub home: PathBuf,
    pub runtime: PathBuf,
    pub environment: BTreeMap<String, String>,
}
pub struct Capture {
    pub account: String,
    pub generation: u64,
    pub kind: String,
    pub chat: i128,
    pub started: u128,
    pub path: PathBuf,
    pub preview: Option<PathBuf>,
    helper: Managed,
    file: File,
    preview_file: Option<File>,
    root: PathBuf,
    storage: Storage,
    relative: String,
    preview_relative: Option<String>,
    keep_input: bool,
    ended: bool,
    reservation: Option<File>,
}
fn remove(storage: &Storage, path: &str, file: &File) {
    use std::os::unix::fs::MetadataExt;
    if let Ok(m) = file.metadata() {
        let _ = storage.remove_owned(path, m.dev(), m.ino());
    }
}
impl std::fmt::Debug for Capture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Capture")
            .field("kind", &self.kind)
            .field("pid", &self.helper.pid())
            .finish()
    }
}
impl Capture {
    pub fn start(
        context: &Context,
        account: String,
        generation: u64,
        kind: &str,
        chat: i128,
    ) -> Result<Self> {
        Self::start_with(context, account, generation, kind, chat, None)
    }
    /// Explicit synthetic fixture input is never accepted through IPC or normal CLI.
    pub fn start_with(
        context: &Context,
        account: String,
        generation: u64,
        kind: &str,
        chat: i128,
        inputs: Option<Vec<String>>,
    ) -> Result<Self> {
        if !crate::paths::valid_account_id(&account) || !matches!(kind, "voice" | "video") {
            return Err("cannot start the recorder".into());
        }
        let mut bytes = [0u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes))
            .map_err(|_| "cannot start the recorder")?;
        let serial = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let (root, relative) = if kind == "voice" {
            (
                context.home.clone(),
                format!(
                    "{}/voice-{serial}.ogg",
                    if account == "default" {
                        ".local/share/omagram/sent".into()
                    } else {
                        format!(".local/share/omagram/accounts/{account}/sent")
                    }
                ),
            )
        } else {
            (
                context.runtime.clone(),
                format!("omagram/rec/note-{serial}.mp4"),
            )
        };
        let storage = Storage::open(&root).map_err(|_| "cannot start the recorder")?;
        let directory = relative
            .rsplit_once('/')
            .ok_or("cannot start the recorder")?
            .0;
        let reservation = crate::upload_lease::reserve(
            &storage,
            directory,
            520 * 1024 * 1024,
            &AtomicBool::new(false),
            std::time::Instant::now() + Duration::from_secs(2),
        )
        .map_err(|_| "saved recording input budget exceeded; existing inputs were kept")?;
        let file = storage
            .create_file(&relative)
            .map_err(|_| "cannot start the recorder")?;
        let path = root.join(&relative);
        use std::os::fd::AsRawFd;
        let output = format!("/proc/{}/fd/{}", std::process::id(), file.as_raw_fd());
        let preview_relative =
            (kind == "video").then(|| format!("omagram/rec/note-{serial}-preview.jpg"));
        let preview_file = match &preview_relative {
            Some(name) => match storage.create_file(name) {
                Ok(f) => Some(f),
                Err(_) => {
                    remove(&storage, &relative, &file);
                    return Err("cannot start the recorder".into());
                }
            },
            None => None,
        };
        let preview = preview_relative.as_ref().map(|p| root.join(p));
        let mut args = if kind == "video" {
            crate::media::video_capture_argv(
                &output,
                &format!(
                    "/proc/{}/fd/{}",
                    std::process::id(),
                    preview_file.as_ref().unwrap().as_raw_fd()
                ),
                &inputs.unwrap_or_default(),
            )
        } else if let Some(inputs) = inputs {
            let mut a = vec![
                "-hide_banner".into(),
                "-loglevel".into(),
                "error".into(),
                "-nostdin".into(),
            ];
            a.extend(inputs);
            a.extend(
                [
                    "-ac",
                    "1",
                    "-ar",
                    "48000",
                    "-c:a",
                    "libopus",
                    "-b:a",
                    "32k",
                    "-application",
                    "voip",
                    "-f",
                    "ogg",
                    &output,
                ]
                .map(str::to_owned),
            );
            a
        } else {
            crate::media::voice_argv(&output)
        };
        args.insert(4, "-y".into());
        let output_index = args.iter().position(|s| s == &output).unwrap();
        args.splice(
            output_index..output_index,
            ["-fs".into(), (512u64 * 1024 * 1024).to_string()],
        );
        // Descriptor names have no suffix: specify the video container explicitly.
        if kind == "video" {
            let index = args.iter().position(|s| s == &output).unwrap();
            args.splice(index..index, ["-f".into(), "mp4".into()]);
        }
        let helper = match Managed::start(
            "/usr/bin/ffmpeg",
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
            &context.environment,
            &[],
            Duration::from_secs(if kind == "video" { 65 } else { 605 }),
            64 * 1024,
        ) {
            Ok(helper) => helper,
            Err(_) => {
                remove(&storage, &relative, &file);
                if let (Some(name), Some(file)) = (&preview_relative, &preview_file) {
                    remove(&storage, name, file);
                }
                return Err("cannot start the recorder".into());
            }
        };
        Ok(Self {
            account,
            generation,
            kind: kind.into(),
            chat,
            started: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            path,
            preview,
            helper,
            file,
            preview_file,
            root,
            storage,
            relative,
            preview_relative,
            keep_input: false,
            ended: false,
            reservation: Some(reservation),
        })
    }
    pub fn finish(&mut self, cancel: &AtomicBool) -> Result<()> {
        while !self.poll() {
            if cancel.load(Ordering::Relaxed) {
                let _ = self.helper.cancel();
                return Err("the connection was closed".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        if let (Some(name), Some(file)) = (&self.preview_relative, &self.preview_file) {
            remove(&self.storage, name, file);
        }
        self.preview_file = None;
        let size = self
            .file
            .metadata()
            .map_err(|_| "the retained recording is not available")?
            .len();
        if size >= 512 * 1024 * 1024 {
            return Err(
                "The recording could not be prepared; it was kept for explicit retry or discard"
                    .into(),
            );
        }
        Ok(())
    }
    pub fn prepare_send(mut self, args: Value, home: PathBuf, options: Value) -> Result<Action> {
        self.keep_input = true; // Stop-with-send never deletes raw input on setup failure.
        let mut bytes = [0u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes))
            .map_err(|_| "the request failed")?;
        let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let source = crate::uploads::Upload {
            path: self.path.clone(),
            size: 0,
            file: self
                .file
                .try_clone()
                .map_err(|_| "the retained recording is not available")?,
        };
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
        let stopped = self
            .helper
            .signal_stop(libc::SIGINT, Duration::from_secs(5))
            .is_ok();
        // A crash between capture stop and main-thread reservation must still
        // expose the original recording and destination. No native send occurs.
        self.keep_input = true;
        let recording = crate::recorded_send::Recording {
            token: token.clone(),
            kind: self.kind.clone(),
            path: source.path.to_string_lossy().into_owned(),
            source: None,
            chat: self.chat,
            args: Value::Object(picked.clone()),
            content: None,
            status: crate::recorded_send::Status::Preparing,
            input: None,
        };
        let checkpoint =
            crate::recording_checkpoint::Checkpoint::open(&home, &self.root, &self.account)
                .and_then(|checkpoint| checkpoint.save(Some(&recording), false));
        if checkpoint.is_err() || !stopped {
            return Ok(Action::RecoverRecorded(recording));
        }
        Ok(Action::PrepareRecorded {
            recording: Some(crate::recorded_send::Recording {
                token: token.clone(),
                kind: self.kind.clone(),
                path: source.path.to_string_lossy().into_owned(),
                source: None,
                chat: self.chat,
                args: Value::Object(picked),
                content: None,
                status: crate::recorded_send::Status::Preparing,
                input: None,
            }),
            preparation: Box::new(crate::recording_jobs::Preparation {
                source,
                home,
                account: self.account.clone(),
                kind: self.kind.clone(),
                token,
                options,
                capture: Some(Box::new(self)),
            }),
        })
    }
    pub fn verify_input(&self, file: &File) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        let expected = file
            .metadata()
            .map_err(|_| "the retained recording is not available")?;
        let visible =
            Storage::open(&self.root).map_err(|_| "the retained recording is not available")?;
        if !visible
            .same_root(&self.storage)
            .map_err(|_| "the retained recording is not available")?
        {
            return Err("the retained recording is not available".into());
        }
        let current = visible
            .open_read(&self.relative)
            .and_then(|f| f.metadata())
            .map_err(|_| "the retained recording is not available")?;
        if expected.dev() != current.dev() || expected.ino() != current.ino() {
            return Err("the retained recording is not available".into());
        }
        Ok(())
    }
    pub fn pid(&self) -> u32 {
        self.helper.pid()
    }
    pub fn poll(&mut self) -> bool {
        if !self.ended {
            self.ended = self.helper.poll().map_or(true, |out| out.is_some());
            if self.ended {
                self.reservation = None;
            }
        }
        self.ended
    }
    pub fn discard(mut self) {
        let _ = self.helper.cancel();
        self.remove();
    }
    fn remove(&self) {
        remove(&self.storage, &self.relative, &self.file);
        if let (Some(name), Some(file)) = (&self.preview_relative, &self.preview_file) {
            remove(&self.storage, name, file);
        }
    }
    pub fn stop(mut self, args: Value, home: PathBuf, options: Value) -> Result<Action> {
        if args["send"] == true {
            return self.prepare_send(args, home, options);
        }
        self.helper
            .signal_stop(libc::SIGINT, Duration::from_secs(5))
            .map_err(|_| "cannot stop the recorder")?;
        Ok(Action::Work(crate::jobs::Work::StopCapture(Box::new(
            Stop { capture: self },
        ))))
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.helper.cancel();
        if !self.keep_input {
            remove(&self.storage, &self.relative, &self.file);
        }
        if let (Some(name), Some(file)) = (&self.preview_relative, &self.preview_file) {
            remove(&self.storage, name, file);
        }
    }
}
#[derive(Debug)]
pub struct Stop {
    pub capture: Capture,
}
impl Stop {
    pub fn run(mut self, cancel: &AtomicBool) -> Result<Action> {
        while !self.capture.poll() {
            if cancel.load(Ordering::Relaxed) {
                let _ = self.capture.helper.cancel();
                self.capture.remove();
                return Err("the connection was closed".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        if let (Some(name), Some(file)) =
            (&self.capture.preview_relative, &self.capture.preview_file)
        {
            remove(&self.capture.storage, name, file);
        }
        self.capture.preview_file = None;
        self.capture.remove();
        Ok(Action::Reply(serde_json::json!({})))
    }
}
