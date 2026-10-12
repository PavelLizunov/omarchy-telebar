//! Bundled bridge process and persisted per-account proxy identity. Startup is
//! cancellable worker work; native proxy configuration stays on the Driver.
use crate::{
    commands::{Action, Result},
    process::Managed,
    storage::Storage,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
#[derive(Clone, Debug)]
pub struct Context {
    pub binary: PathBuf,
    pub environment: BTreeMap<String, String>,
}
pub struct Running {
    pub helper: Managed,
    pub port: u16,
    secret: String,
    pending: Vec<u8>,
    pub metrics: Value,
}
impl std::fmt::Debug for Running {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Running { secret: [redacted] }")
    }
}
impl Running {
    pub fn proxy(&self) -> Value {
        json!({"@type":"proxy","server":"127.0.0.1","port":self.port,"type":{"@type":"proxyTypeMtproto","secret":format!("dd{}",self.secret)}})
    }
    pub fn poll(&mut self) -> bool {
        if !self.helper.poll().is_ok_and(|output| output.is_none()) {
            return false;
        }
        self.pending.extend(self.helper.take_stdout());
        if self.pending.len() > 8192 {
            return false;
        }
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            if end > 4096 {
                return false;
            }
            let line = self.pending.drain(..=end).collect::<Vec<_>>();
            let Ok(data) = crate::wire::bounded_json(&line, 2, 16, 80) else {
                return false;
            };
            if data["event"] == "stats" {
                let mut metrics = json!({});
                for (key, value) in data.as_object().into_iter().flatten() {
                    if value.as_u64().is_some_and(|n| n <= 1_000_000_000_000_000) {
                        metrics[key] = value.clone();
                    }
                }
                self.metrics = metrics;
            }
        }
        self.pending.len() <= 4096
    }
}
impl Context {
    pub fn start(self, cancel: &AtomicBool) -> Result<Action> {
        if cancel.load(Ordering::Relaxed) {
            return Err("the connection was closed".into());
        }
        let mut secret = [0; 16];
        openssl::rand::rand_bytes(&mut secret).map_err(|_| "could not start the bundled bridge")?;
        let secret = secret
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let input = (json!({"secret":secret}).to_string() + "\n").into_bytes();
        let mut helper = Managed::start_owned(
            self.binary
                .to_str()
                .ok_or("could not start the bundled bridge")?,
            &["--proxy"],
            &self.environment,
            &input,
            8192,
        )
        .map_err(|_| "could not start the bundled bridge")?;
        helper.keep_stdin_open();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut pending = Vec::new();
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("the connection was closed".into());
            }
            if Instant::now() >= deadline || !helper.poll().is_ok_and(|output| output.is_none()) {
                return Err("could not start the bundled bridge".into());
            }
            pending.extend(helper.take_stdout());
            if pending.len() > 4096 {
                return Err("could not start the bundled bridge".into());
            }
            if let Some(end) = pending.iter().position(|b| *b == b'\n') {
                let line = pending.drain(..=end).collect::<Vec<_>>();
                let data = crate::wire::bounded_json(&line, 2, 8, 80)
                    .map_err(|_| "could not start the bundled bridge")?;
                if data["event"] != "ready" {
                    return Err("could not start the bundled bridge".into());
                }
                let port = data["port"]
                    .as_u64()
                    .filter(|n| (1..=65535).contains(n))
                    .ok_or("could not start the bundled bridge")? as u16;
                return Ok(Action::BridgeReady(Running {
                    helper,
                    port,
                    secret,
                    pending,
                    metrics: json!({}),
                }));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
pub struct Bridge {
    pub context: Option<Context>,
    pub running: Option<Running>,
    pub metadata: Value,
    pub busy: std::collections::HashSet<String>,
    pub idle_at: Option<Instant>,
}
#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Test,
    Enable,
    Disable,
}
#[derive(Debug, Clone)]
pub enum Answer {
    EnableList { proxy: Value, entry: Value },
    Enabled { previous: Value },
    DisableList { entry: Value },
    Disabled,
}
#[derive(Debug)]
pub enum Mutation {
    Enable(Value),
    Disable,
}
impl Answer {
    pub fn complete(&self, event: &Value) -> Result<Action> {
        let native = |query, answer| Action::Native {
            query,
            answer: crate::commands::Answer::Bridge(answer),
        };
        let proxies: Vec<_> = crate::projection::array(&event["proxies"], 100).collect();
        match self {
            Self::EnableList { proxy, entry } => {
                let active = proxies
                    .iter()
                    .find(|p| p["is_enabled"] == true)
                    .map_or(0, |p| crate::projection::integer(&p["id"], 0));
                let id = crate::projection::integer(&entry["proxy_id"], 0);
                let previous = if entry.is_object() {
                    entry.get("previous_id").cloned().unwrap_or(json!(0))
                } else {
                    crate::projection::number(active)
                };
                let query = if id > 0
                    && proxies
                        .iter()
                        .any(|p| crate::projection::integer(&p["id"], 0) == id)
                {
                    json!({"@type":"editProxy","proxy_id":crate::projection::number(id),"proxy":proxy,"enable":true,"comment":"Telebar bundled bridge"})
                } else {
                    json!({"@type":"addProxy","proxy":proxy,"enable":true,"comment":"Telebar bundled bridge"})
                };
                Ok(native(query, Self::Enabled { previous }))
            }
            Self::Enabled { previous } => {
                let id = crate::projection::integer(&event["id"], 0);
                if id <= 0 {
                    return Err("could not enable the bundled bridge".into());
                }
                Ok(Action::BridgeMutation(Mutation::Enable(
                    json!({"proxy_id":crate::projection::number(id),"previous_id":previous}),
                )))
            }
            Self::DisableList { entry } => {
                let active = proxies
                    .iter()
                    .find(|p| p["is_enabled"] == true)
                    .map_or(0, |p| crate::projection::integer(&p["id"], 0));
                if active != crate::projection::integer(&entry["proxy_id"], 0) {
                    return Ok(Action::BridgeMutation(Mutation::Disable));
                }
                let previous = crate::projection::integer(&entry["previous_id"], 0);
                let query = if previous > 0
                    && proxies
                        .iter()
                        .any(|p| crate::projection::integer(&p["id"], 0) == previous)
                {
                    json!({"@type":"enableProxy","proxy_id":crate::projection::number(previous)})
                } else {
                    json!({"@type":"disableProxy"})
                };
                Ok(native(query, Self::Disabled))
            }
            Self::Disabled => Ok(Action::BridgeMutation(Mutation::Disable)),
        }
    }
}
impl Bridge {
    pub fn load(home: &Storage) -> Self {
        let raw = home
            .read(".config/omagram/bridge.json", 8192)
            .ok()
            .flatten()
            .and_then(|b| crate::wire::bounded_json(&b, 4, 80, 64).ok())
            .unwrap_or_else(|| json!({}));
        let mut metadata = json!({});
        for (id, entry) in raw.as_object().into_iter().flatten() {
            if metadata.as_object().expect("object").len() >= 10 {
                break;
            }
            if crate::paths::valid_account_id(id)
                && entry.is_object()
                && ["proxy_id", "previous_id"].iter().all(|key| {
                    entry
                        .get(key)
                        .unwrap_or(&Value::from(0))
                        .as_u64()
                        .is_some_and(|n| n < 1 << 31)
                })
            {
                metadata[id] = entry.clone();
            }
        }
        Self {
            context: None,
            running: None,
            metadata,
            busy: Default::default(),
            idle_at: None,
        }
    }
    pub fn status(&self, account: &str) -> Value {
        json!({"running":self.running.is_some(),"enabled":self.metadata.get(account).is_some(),"port":self.running.as_ref().map_or(0,|r|r.port),"metrics":self.running.as_ref().map_or_else(||json!({}),|r|r.metrics.clone()),"pid":self.running.as_ref().map_or(0,|r|r.helper.pid())})
    }
    pub fn save(&self, home: &Storage) -> Result<()> {
        home.write(
            ".config/omagram/bridge.json",
            self.metadata.to_string().as_bytes(),
            0o600,
        )
        .map_err(|_| "could not save the bundled bridge".into())
    }
    pub fn poll(&mut self) {
        if self.running.as_mut().is_some_and(|r| !r.poll()) {
            self.running = None;
        }
        if self.metadata.as_object().is_none_or(|o| o.is_empty())
            && self.busy.is_empty()
            && self.idle_at.is_some_and(|t| Instant::now() >= t)
        {
            self.running = None;
            self.idle_at = None;
        }
    }
}
