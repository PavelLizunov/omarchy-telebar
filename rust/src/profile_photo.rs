//! Profile photo conversion and native handoff with outcome-aware input leases.
use crate::{
    commands::{Action, Answer, Result},
    storage::Storage,
    uploads::Upload,
};
use serde_json::json;
use std::{
    io::Read,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
#[derive(Debug)]
pub struct Job {
    pub source: Upload,
    pub runtime: PathBuf,
}
#[derive(Debug)]
struct Owned {
    runtime: PathBuf,
    path: String,
    submitted: AtomicBool,
}
/// Unsubmitted/certainly settled images clean up. Submitted images survive an
/// unknown native outcome: local timeout is not native cancellation.
#[derive(Clone, Debug)]
pub struct Lease(Arc<Owned>);
impl Lease {
    pub fn submitted(&self) {
        self.0.submitted.store(true, Ordering::Relaxed);
    }
    pub fn settled(&self) {
        self.0.submitted.store(false, Ordering::Relaxed);
    }
    pub fn path(&self) -> PathBuf {
        self.0.runtime.join(&self.0.path)
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        if !self.submitted.load(Ordering::Relaxed)
            && let Ok(storage) = Storage::open(&self.runtime)
        {
            let _ = storage.remove_file(&self.path);
        }
    }
}
impl Job {
    pub fn run(mut self, cancel: &AtomicBool) -> Result<Action> {
        if cancel.load(Ordering::Relaxed) {
            return Err("the connection was closed".into());
        }
        if self.source.size > 32 * 1024 * 1024 {
            return Err("that picture could not be read".into());
        }
        let before = self
            .source
            .file
            .metadata()
            .map_err(|_| "that picture could not be read")?;
        let mut bytes = vec![];
        self.source
            .file
            .by_ref()
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "that picture could not be read")?;
        use std::os::unix::fs::MetadataExt;
        let after = self
            .source
            .file
            .metadata()
            .map_err(|_| "that picture could not be read")?;
        if bytes.len() as u64 != self.source.size
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err("that picture could not be read".into());
        }
        // Pipe-only demuxing prevents a chosen container/playlist from fetching URLs
        // or reaching other local files while still accepting first video frames.
        let mut argv = crate::media::profile_photo_argv("pipe:0", "pipe:1");
        let index = argv.iter().position(|s| s == "-i").unwrap();
        argv.splice(index..index, ["-protocol_whitelist".into(), "pipe".into()]);
        let index = argv.iter().position(|s| s == "image2").unwrap();
        argv[index] = "image2pipe".into();
        let out = crate::process::run_cancel(
            "/usr/bin/ffmpeg",
            &argv.iter().map(String::as_str).collect::<Vec<_>>(),
            &Default::default(),
            &bytes,
            Duration::from_secs(90),
            8 * 1024 * 1024,
            cancel,
        )
        .map_err(|_| "that picture could not be read")?;
        if !out.ok()
            || out.stdout.len() < 4
            || !out.stdout.starts_with(&[0xff, 0xd8])
            || !out.stdout.ends_with(&[0xff, 0xd9])
        {
            return Err("that picture could not be read".into());
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("the connection was closed".into());
        }
        let mut random = [0u8; 16];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut random))
            .map_err(|_| "that picture could not be read")?;
        let name = format!(
            "omagram/rec/profile-{}.jpg",
            random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let storage = Storage::open(&self.runtime).map_err(|_| "that picture could not be read")?;
        storage
            .write(&name, &out.stdout, 0o600)
            .map_err(|_| "that picture could not be read")?;
        let lease = Lease(Arc::new(Owned {
            runtime: self.runtime,
            path: name,
            submitted: AtomicBool::new(false),
        }));
        if cancel.load(Ordering::Relaxed) {
            return Err("the connection was closed".into());
        }
        Ok(Action::Native {
            query: json!({"@type":"setProfilePhoto","is_public":false,"photo":{"@type":"inputChatPhotoStatic","photo":{"@type":"inputFileLocal","path":lease.path()}}}),
            answer: Answer::ProfilePhoto(lease),
        })
    }
}
