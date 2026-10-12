//! Durable retained-recording metadata. No automatic sends or credential/profile
//! access. Recovery validates bounded metadata and exact owned file identities;
//! dispatch uncertainty disables retry, and path substitution never authorizes
//! deletion of a replacement. One checkpoint per persisted account identity.
use crate::{
    recorded_send::{Recording, Status},
    storage::Storage,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
pub struct Checkpoint {
    home: PathBuf,
    runtime: PathBuf,
    home_store: Storage,
    runtime_store: Storage,
    path: String,
    sent: String,
    identities: RefCell<HashMap<String, Value>>,
}
impl std::fmt::Debug for Checkpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecordingCheckpoint")
    }
}
impl Checkpoint {
    pub fn open(home: &Path, runtime: &Path, account: &str) -> io::Result<Self> {
        if !crate::paths::valid_account_id(account) {
            return Err(io::Error::other("invalid recording account"));
        }
        let base = if account == "default" {
            ".local/share/omagram".into()
        } else {
            format!(".local/share/omagram/accounts/{account}")
        };
        Ok(Self {
            home: home.into(),
            runtime: runtime.into(),
            home_store: Storage::open(home)?,
            runtime_store: Storage::open(runtime)?,
            path: format!("{base}/retained-recording-v1.json"),
            sent: format!("{base}/sent"),
            identities: RefCell::new(HashMap::new()),
        })
    }
    fn scope<'a>(&'a self, path: &str) -> io::Result<(&'a Storage, &'a Path, String)> {
        let path = Path::new(path);
        let (store, root) = if path.starts_with(self.home.join(&self.sent)) {
            (&self.home_store, self.home.as_path())
        } else if path.starts_with(self.runtime.join("omagram/rec")) {
            (&self.runtime_store, self.runtime.as_path())
        } else {
            return Err(io::Error::other("recording path outside owned media"));
        };
        let relative = path
            .strip_prefix(root)
            .ok()
            .and_then(|p| p.to_str())
            .ok_or_else(|| io::Error::other("invalid recording path"))?
            .to_owned();
        // Storage parses every component and refuses links/special files.
        Ok((store, root, relative))
    }
    fn identity(&self, path: &str) -> io::Result<Value> {
        let (store, root, relative) = self.scope(path)?;
        let visible = Storage::open(root)?;
        if !visible.same_root(store)? {
            return Err(io::Error::other("recording root changed"));
        }
        match store.open_read(&relative) {
            Ok(file) => {
                let m = file.metadata()?;
                let v = visible.open_read(&relative)?.metadata()?;
                if m.nlink() != 1
                    || m.mode() & 0o022 != 0
                    || m.dev() != v.dev()
                    || m.ino() != v.ino()
                {
                    return Err(io::Error::other("recording input changed"));
                }
                Ok(json!({"path":path,"dev":m.dev(),"ino":m.ino()}))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                Ok(json!({"path":path,"missing":true}))
            }
            Err(e) => Err(e),
        }
    }
    fn remembered(&self, path: &str) -> io::Result<Value> {
        if let Some(identity) = self.identities.borrow().get(path) {
            return Ok(identity.clone());
        }
        let identity = self.identity(path)?;
        self.identities
            .borrow_mut()
            .insert(path.into(), identity.clone());
        Ok(identity)
    }
    pub fn admit(&self, record: &Recording) -> io::Result<()> {
        self.remembered(&record.path)?;
        if let Some(source) = &record.source {
            self.remembered(source)?;
        }
        Ok(())
    }
    pub fn save(&self, record: Option<&Recording>, dispatched: bool) -> io::Result<()> {
        if dispatched && let Some(r) = record {
            for path in std::iter::once(&r.path).chain(r.source.as_ref()) {
                let expected = self.remembered(path)?;
                if expected["missing"] == true || self.identity(path)? != expected {
                    return Err(io::Error::other("recording path changed before dispatch"));
                }
            }
        }
        let value = if let Some(r) = record {
            json!({"version":1,"dispatched":dispatched,"token":r.token,"kind":r.kind,"chatId":crate::projection::number(r.chat),"args":r.args,"content":Value::Null,"state":r.status.name(),"input":self.remembered(&r.path)?,"source":r.source.as_ref().map(|p|self.remembered(p)).transpose()?})
        } else {
            json!({"version":1,"record":null})
        };
        let bytes = serde_json::to_vec(&value)?;
        if bytes.len() > 32768 {
            return Err(io::Error::other("recording checkpoint exceeds limit"));
        }
        self.home_store.write(&self.path, &bytes, 0o600)?;
        if let Some(record) = record {
            self.identities
                .borrow_mut()
                .retain(|path, _| path == &record.path || record.source.as_ref() == Some(path));
        } else {
            self.identities.borrow_mut().clear();
        }
        Ok(())
    }
    pub fn load(&self) -> io::Result<Option<Recording>> {
        let Some(bytes) = self.home_store.read(&self.path, 32768)? else {
            return Ok(None);
        };
        let value = crate::wire::bounded_json(&bytes, 12, 512, 8192)
            .map_err(|_| io::Error::other("invalid recording checkpoint"))?;
        if value["version"] != 1 {
            return Err(io::Error::other("unknown recording checkpoint"));
        }
        if value.as_object().is_some_and(|m| m.len() == 2)
            && value.get("record") == Some(&Value::Null)
        {
            return Ok(None);
        }
        let invalid = || io::Error::other("invalid recording checkpoint");
        if value.as_object().is_none_or(|m| m.len() != 10) {
            return Err(invalid());
        }
        let token = value["token"]
            .as_str()
            .filter(|v| v.len() == 32 && v.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(invalid)?
            .to_owned();
        let kind = value["kind"]
            .as_str()
            .filter(|v| matches!(*v, "voice" | "video"))
            .ok_or_else(invalid)?
            .to_owned();
        let chat = value["chatId"]
            .as_i64()
            .filter(|v| *v != 0)
            .ok_or_else(invalid)? as i128;
        let dispatched = value["dispatched"].as_bool().ok_or_else(invalid)?;
        if !matches!(
            value["state"].as_str(),
            Some("preparing" | "prepareFailed" | "sending" | "rejected" | "unknown")
        ) {
            return Err(invalid());
        }
        if value["args"].as_object().is_none_or(|args| {
            args.keys().any(|k| {
                !matches!(
                    k.as_str(),
                    "replyToMessageId" | "topicId" | "threadId" | "silent" | "sendAt"
                )
            })
        }) {
            return Err(invalid());
        }
        if value["args"].get("silent").is_some_and(|v| !v.is_boolean()) {
            return Err(invalid());
        }
        crate::commands::topic(&value["args"]).map_err(|_| invalid())?;
        crate::commands::reply(&value["args"]).map_err(|_| invalid())?;
        let check = |input: &Value| -> io::Result<(String, bool)> {
            let path = input["path"]
                .as_str()
                .filter(|p| p.len() <= 4096)
                .ok_or_else(invalid)?
                .to_owned();
            self.scope(&path)?;
            self.identities
                .borrow_mut()
                .insert(path.clone(), input.clone());
            let actual = self.identity(&path);
            Ok((
                path,
                actual.is_ok_and(|a| {
                    input["dev"].as_u64().is_some()
                        && a["dev"] == input["dev"]
                        && a["ino"] == input["ino"]
                }),
            ))
        };
        let (path, valid) = check(&value["input"])?;
        let source = if value["source"].is_null() {
            None
        } else {
            Some(check(&value["source"])?)
        };
        let valid = valid && source.as_ref().is_none_or(|(_, valid)| *valid);
        // Confirmed rejection is safely re-prepared from its retained source;
        // never deserialize native content into an automatic continuation.
        let status = if !valid || dispatched || value["state"] == "unknown" {
            Status::Unknown
        } else {
            Status::PrepareFailed
        };
        let mut input = None;
        if valid
            && Path::new(&path).starts_with(self.home.join(&self.sent))
            && Path::new(&path)
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|p| p.to_str())
                .is_some_and(|n| n.starts_with("upload-"))
        {
            let relative = Path::new(&path)
                .strip_prefix(&self.home)
                .ok()
                .and_then(|p| p.to_str())
                .ok_or_else(invalid)?;
            // Recovery retains the existing generated file; it is not a new
            // pre-send worker copy and must survive until explicit resolution.
            input = crate::upload_lease::Lease::restore(self.home.clone(), relative).ok();
            if input.is_none() {
                return Ok(Some(Recording {
                    token,
                    kind,
                    path,
                    source: source.map(|(p, _)| p),
                    chat,
                    args: value["args"].clone(),
                    content: None,
                    status: Status::Unknown,
                    input: None,
                }));
            }
        }
        Ok(Some(Recording {
            token,
            kind,
            path,
            source: source.map(|(p, _)| p),
            chat,
            args: value["args"].clone(),
            content: None,
            status,
            input,
        }))
    }
    pub fn dismiss_unknown(&self) -> io::Result<()> {
        crate::upload_lease::preserve_unsent(&self.home_store, &self.sent)?;
        self.save(None, false)
    }
    pub fn discard(&self, record: &Recording) -> io::Result<()> {
        if record.status == Status::Unknown {
            return self.dismiss_unknown();
        }
        {
            for path in std::iter::once(&record.path).chain(record.source.as_ref()) {
                let expected = self.remembered(path)?;
                let (store, _, relative) = self.scope(path)?;
                if let (Some(dev), Some(ino)) = (expected["dev"].as_u64(), expected["ino"].as_u64())
                {
                    store.remove_owned(&relative, dev, ino)?;
                }
            }
            if let Some(input) = &record.input {
                input.rejected(400);
            }
        }
        self.save(None, false)
    }
}
