//! Explicit per-person notification sound work. Synthetic rendering and cache
//! preparation are separated from Driver-owned foreground PipeWire playback.
use crate::{
    commands::{Action, Result},
    process::Managed,
    storage::Storage,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
#[derive(Debug)]
pub struct Job {
    pub home: PathBuf,
    pub seed: i128,
    pub variant: u64,
    pub style: String,
    pub environment: BTreeMap<String, String>,
}
pub struct Prepared {
    file: std::fs::File,
    environment: BTreeMap<String, String>,
}
impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreparedSound { owned_file: [redacted] }")
    }
}
impl Job {
    pub fn prepare(
        home: PathBuf,
        seed: i128,
        settings: &Value,
        environment: BTreeMap<String, String>,
    ) -> Option<Self> {
        let style = settings["sounds"]["style"].as_str().unwrap_or("pop");
        if seed == 0 || !matches!(style, "pop" | "drop" | "knock") {
            return None;
        }
        Some(Self {
            home,
            seed,
            variant: settings["sounds"]["variants"][seed.to_string()]
                .as_u64()
                .unwrap_or(0),
            style: style.into(),
            environment,
        })
    }
    pub fn run(self, cancel: &AtomicBool) -> Result<Action> {
        if cancel.load(Ordering::Relaxed) {
            return Err("the connection was closed".into());
        }
        let home = Storage::open(&self.home).map_err(|_| "the sound could not be prepared")?;
        let identity = format!(
            "{:x}",
            Sha256::digest(format!("{}:{}", self.seed, self.variant).as_bytes())
        );
        let name = format!(
            ".cache/omagram/sounds/{}-{}.wav",
            self.style,
            &identity[..24]
        );
        let cached = home
            .read(&name, 128 * 1024)
            .map_err(|_| "the sound could not be prepared")?;
        if cached.as_ref().is_some_and(|bytes| {
            bytes.len() < 44 || !bytes.starts_with(b"RIFF") || bytes.get(8..12) != Some(b"WAVE")
        }) {
            return Err("the sound could not be prepared".into());
        }
        if cached.is_none() {
            let bytes = crate::sounds::render(
                &crate::sounds::motif(&self.seed.to_string(), self.variant),
                &self.style,
            )
            .map_err(|_| "the sound could not be prepared")?;
            if cancel.load(Ordering::Relaxed) {
                return Err("the connection was closed".into());
            }
            let entries = home
                .files(".cache/omagram/sounds", 350)
                .map_err(|_| "the sound could not be prepared")?;
            if entries.len() >= 300 {
                for (entry, _) in entries {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("the connection was closed".into());
                    }
                    if entry.ends_with(".wav") {
                        home.remove_file(&format!(".cache/omagram/sounds/{entry}"))
                            .map_err(|_| "the sound could not be prepared")?;
                    }
                }
            }
            home.write(&name, &bytes, 0o600)
                .map_err(|_| "the sound could not be prepared")?;
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("the connection was closed".into());
        }
        let file = home
            .open_read(&name)
            .map_err(|_| "the sound could not be prepared")?;
        Ok(Action::SoundReady(Prepared {
            file,
            environment: self.environment,
        }))
    }
}
impl Prepared {
    pub fn spawn(&self) -> Result<Managed> {
        use std::os::fd::AsRawFd;
        Managed::start(
            "/usr/bin/pw-play",
            &[
                "--media-role",
                "Notification",
                &format!("/proc/{}/fd/{}", std::process::id(), self.file.as_raw_fd()),
            ],
            &self.environment,
            &[],
            Duration::from_secs(10),
            4096,
        )
        .map_err(|_| "the sound could not play".into())
    }
}
