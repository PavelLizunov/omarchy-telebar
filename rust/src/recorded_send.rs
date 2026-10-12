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
    fn name(&self) -> &'static str {
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
}
impl Retained {
    pub fn reserve(&mut self, record: Recording) -> Result<(), &'static str> {
        if self.record.is_some() {
            return Err("a recorded message is still pending; resolve or discard it first");
        }
        if record.token.len() != 32
            || !record.token.bytes().all(|b| b.is_ascii_hexdigit())
            || !matches!(record.kind.as_str(), "voice" | "video")
            || record.status != Status::Preparing
        {
            return Err("invalid retained recording");
        }
        self.record = Some(record);
        Ok(())
    }
    pub fn view(&self, account: &str) -> Value {
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
        Ok(())
    }
    pub fn prepare_failed(&mut self, token: &str) -> Result<(), &'static str> {
        let r = self.checked(token)?;
        if r.status != Status::Preparing {
            return Err("recording is not being prepared");
        }
        r.status = Status::PrepareFailed;
        Ok(())
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
        Ok(if r.status == Status::Rejected {
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
        Ok(())
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
        self.record = None;
        Ok(())
    }
    /// Return exact deletable owned paths; unknown sends deliberately keep all
    /// files, since TDLib may still hold them. Caller enforces media-root policy.
    pub fn discard(&mut self, token: &str) -> Result<Vec<String>, &'static str> {
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
