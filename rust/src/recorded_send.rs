//! Retained recording outcome owner. Uncertain native sends cannot be retried
//! or have their input deleted; only explicit rejection permits resend.
use serde_json::{Value, json};
#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    Preparing,
    PrepareFailed,
    Sending,
    Rejected,
    Unknown,
}
impl Status {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::PrepareFailed => "prepareFailed",
            Self::Sending => "sending",
            Self::Rejected => "rejected",
            Self::Unknown => "unknown",
        }
    }
}
#[derive(Debug)]
pub struct Recording {
    pub token: String,
    pub kind: String,
    pub path: String,
    pub source: Option<String>,
    pub chat: i128,
    pub args: Value,
    pub content: Option<Value>,
    pub status: Status,
    pub input: Option<crate::upload_lease::Lease>,
}
#[derive(Default, Debug)]
pub struct Retained {
    pub record: Option<Recording>,
    checkpoint: Option<crate::recording_checkpoint::Checkpoint>,
    dispatched: std::sync::atomic::AtomicBool,
    recovery_error: bool,
}
impl Retained {
    pub fn attach(
        &mut self,
        checkpoint: crate::recording_checkpoint::Checkpoint,
    ) -> std::io::Result<()> {
        if self.record.is_some() || self.checkpoint.is_some() {
            return Err(std::io::Error::other(
                "retained recording already configured",
            ));
        }
        match checkpoint.load() {
            Ok(record) => self.record = record,
            Err(_) => self.recovery_error = true,
        }

        self.dispatched.store(
            self.record
                .as_ref()
                .is_some_and(|r| r.status == Status::Unknown),
            std::sync::atomic::Ordering::Relaxed,
        );
        self.checkpoint = Some(checkpoint);
        Ok(())
    }
    pub fn recover_live(&mut self, mut record: Recording) -> Result<(), &'static str> {
        if self.record.is_some() || self.recovery_error {
            return Err("a recorded message is still pending; resolve or discard it first");
        }
        record.status = Status::PrepareFailed;
        if let Some(checkpoint) = &self.checkpoint {
            checkpoint
                .admit(&record)
                .map_err(|_| "the stopped recording could not be retained")?;
        }
        self.record = Some(record);
        let _ = self.persist(); // Keep live explicit resolution even on disk failure.
        Ok(())
    }
    pub fn fail_preparation(&mut self, token: &str) {
        if let Some(record) = self.record.as_mut().filter(|r| r.token == token) {
            record.status = if self.dispatched.load(std::sync::atomic::Ordering::Relaxed) {
                Status::Unknown
            } else {
                Status::PrepareFailed
            };
        }
        let _ = self.persist();
    }
    pub fn persist(&self) -> Result<(), &'static str> {
        if let Some(checkpoint) = &self.checkpoint {
            checkpoint
                .save(
                    self.record.as_ref(),
                    self.dispatched.load(std::sync::atomic::Ordering::Relaxed),
                )
                .map_err(|_| "the retained recording checkpoint could not be saved")?;
        }
        Ok(())
    }
    pub fn native_dispatched(&self, token: &str) -> std::io::Result<()> {
        let record = self
            .record
            .as_ref()
            .filter(|r| r.token == token)
            .ok_or_else(|| std::io::Error::other("recording owner missing"))?;
        if let Some(checkpoint) = &self.checkpoint {
            checkpoint.save(Some(record), true)?;
        }
        if let Some(input) = &record.input {
            input.submitted()?;
        }
        self.dispatched
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
    pub fn protected_inputs(&self, home: &std::path::Path) -> Vec<String> {
        self.record
            .iter()
            .flat_map(|r| std::iter::once(&r.path).chain(r.source.as_ref()))
            .filter_map(|p| {
                std::path::Path::new(p)
                    .strip_prefix(home)
                    .ok()
                    .and_then(|p| p.to_str())
                    .map(str::to_owned)
            })
            .collect()
    }
    pub fn recovery_blocked(&self) -> bool {
        self.recovery_error
    }
    pub fn has_checkpoint(&self) -> bool {
        self.checkpoint.is_some()
    }
    pub fn reserve(&mut self, record: Recording) -> Result<(), &'static str> {
        if self.record.is_some() || self.recovery_error {
            return Err("a recorded message is still pending; resolve or discard it first");
        }
        if record.token.len() != 32
            || !record.token.bytes().all(|b| b.is_ascii_hexdigit())
            || !matches!(record.kind.as_str(), "voice" | "video")
            || record.status != Status::Preparing
        {
            return Err("invalid retained recording");
        }
        self.dispatched
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.record = Some(record);
        if let Err(error) = self.persist() {
            self.record = None;
            return Err(error);
        }
        Ok(())
    }
    pub fn view(&self, account: &str) -> Value {
        if self.recovery_error {
            return json!({"token":"0".repeat(32),"kind":"video","chatId":0,"state":"unknown","account":account});
        }
        self.record.as_ref().map(|r|json!({"token":r.token,"kind":r.kind,"chatId":crate::projection::number(r.chat),"state":r.status.name(),"account":account})).unwrap_or(Value::Null)
    }
    pub fn checked(&mut self, token: &str) -> Result<&mut Recording, &'static str> {
        self.record
            .as_mut()
            .filter(|r| r.token == token)
            .ok_or("that recorded message is no longer available")
    }
    pub fn retry(&mut self, token: &str) -> Result<bool, &'static str> {
        let r = self.checked(token)?;
        match r.status {
            Status::PrepareFailed => Ok(true),
            Status::Rejected => Ok(false),
            _ => Err("retry requires a confirmed rejection; this message may still be sent"),
        }
    }
    pub fn sending(&mut self, token: &str) -> Result<(), &'static str> {
        let r = self.checked(token)?;
        if r.content.is_none() || !matches!(r.status, Status::Preparing | Status::Rejected) {
            return Err("recording content is not prepared");
        }
        r.status = Status::Sending;
        if let Err(error) = self.persist() {
            self.checked(token)?.status = Status::PrepareFailed;
            return Err(error);
        }
        Ok(())
    }
    pub fn prepare_failed(&mut self, token: &str) -> Result<(), &'static str> {
        let r = self.checked(token)?;
        if r.status != Status::Preparing {
            return Err("recording is not being prepared");
        }
        r.status = Status::PrepareFailed;
        self.persist()
    }
    pub fn reject(&mut self, token: &str, code: i64) -> Result<&'static str, &'static str> {
        let r = self.checked(token)?;
        if r.status != Status::Sending {
            return Err("recording is not being sent");
        }
        r.status = if (1..500).contains(&code) && code != 408 {
            Status::Rejected
        } else {
            Status::Unknown
        };
        let rejected = r.status == Status::Rejected;
        if rejected {
            self.dispatched
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }
        self.persist()?;
        Ok(if rejected {
            "Telegram rejected the recorded message; it was kept for explicit retry"
        } else {
            "The recorded message may still be sent; check Telegram before discarding; retry is disabled"
        })
    }
    pub fn cancelled(&mut self, token: &str) -> Result<(), &'static str> {
        let r = self.checked(token)?;
        r.status = match r.status {
            Status::Preparing => Status::PrepareFailed,
            Status::Sending => Status::Unknown,
            _ => return Ok(()),
        };
        self.persist()
    }
    pub fn accepted(&mut self, token: &str, event: &Value) -> Result<(), &'static str> {
        let r = self.checked(token)?;
        if r.status != Status::Sending
            || event["@type"] != "message"
            || crate::projection::integer(&event["chat_id"], 0) != r.chat
            || crate::projection::integer(&event["id"], 0) <= 0
        {
            return Err("invalid recorded-message acknowledgement");
        }
        if let Some(checkpoint) = &self.checkpoint {
            checkpoint
                .save(None, false)
                .map_err(|_| "the retained recording checkpoint could not be cleared")?;
        }
        self.record = None;
        Ok(())
    }
    /// Return exact deletable owned paths; unknown sends deliberately keep all
    /// files, since TDLib may still hold them. Caller enforces media-root policy.
    pub fn discard(&mut self, token: &str) -> Result<Vec<String>, &'static str> {
        if self.recovery_error && token == "0".repeat(32) {
            self.checkpoint
                .as_ref()
                .ok_or("recording checkpoint missing")?
                .dismiss_unknown()
                .map_err(|_| "the retained recording checkpoint could not be cleared")?;
            self.recovery_error = false;
            return Ok(vec![]);
        }
        let r = self.checked(token)?;
        if matches!(r.status, Status::Preparing | Status::Sending) {
            return Err("the recorded message is still being prepared or sent");
        }
        let paths = if r.status == Status::Unknown {
            vec![]
        } else {
            std::iter::once(r.path.clone())
                .chain(r.source.clone())
                .collect()
        };
        if let Some(checkpoint) = &self.checkpoint {
            checkpoint
                .discard(self.record.as_ref().expect("checked recording"))
                .map_err(|_| "the retained recording checkpoint could not be cleared")?;
        }
        self.record = None;
        Ok(paths)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn recording() -> Recording {
        Recording {
            token: "a".repeat(32),
            kind: "voice".into(),
            path: "/inert/sent/voice.ogg".into(),
            source: None,
            chat: 42,
            args: json!({}),
            content: Some(json!({"@type":"inputMessageVoiceNote"})),
            status: Status::Preparing,
            input: None,
        }
    }
    #[test]
    fn timeout_and_server_failures_disable_retry_and_keep_input() {
        for code in [0, 408, 500, 503] {
            let mut retained = Retained::default();
            retained.reserve(recording()).unwrap();
            retained.sending(&"a".repeat(32)).unwrap();
            retained.reject(&"a".repeat(32), code).unwrap();
            assert!(retained.retry(&"a".repeat(32)).is_err());
            assert!(retained.discard(&"a".repeat(32)).unwrap().is_empty());
        }
    }
    #[test]
    fn rejected_recording_is_kept_for_explicit_retry_only() {
        let mut retained = Retained::default();
        retained.reserve(recording()).unwrap();
        assert!(retained.reserve(recording()).is_err());
        retained.sending(&"a".repeat(32)).unwrap();
        retained.reject(&"a".repeat(32), 400).unwrap();
        assert!(!retained.retry(&"a".repeat(32)).unwrap());
        retained.sending(&"a".repeat(32)).unwrap();
        assert!(
            retained
                .accepted(
                    &"a".repeat(32),
                    &json!({"@type":"message","chat_id":43,"id":7})
                )
                .is_err()
        );
        retained
            .accepted(
                &"a".repeat(32),
                &json!({"@type":"message","chat_id":42,"id":7}),
            )
            .unwrap();
        assert!(retained.record.is_none());
    }
}
