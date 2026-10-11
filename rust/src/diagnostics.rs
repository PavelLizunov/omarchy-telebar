//! Bounded numeric diagnostics. No request arguments, message text, paths,
//! environment, exception strings or process command lines enter records.
use crate::storage::Storage;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const FIELDS: &[&str] = &[
    "timer_max_ms",
    "timer_late_count",
    "frame_count",
    "frame_max_ms",
    "frame_slow_count",
    "pending",
    "pending_max",
    "oldest_ms",
    "rx_lines",
    "rx_bytes",
    "tx_requests",
    "response_max_ms",
    "response_slow_count",
    "handler_max_ms",
    "handler_slow_count",
    "parse_errors",
    "connected",
    "focused",
    "visible",
    "chats",
    "messages",
    "files",
    "stories",
    "width",
    "height",
    "chat_list_width",
    "generation",
    "implementation_revision",
];
pub fn numeric_fields(value: &Value) -> Value {
    let mut fields = serde_json::Map::new();
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            if FIELDS.contains(&key.as_str())
                && (value.is_boolean()
                    || value
                        .as_f64()
                        .is_some_and(|n| n.is_finite() && (0.0..=1e12).contains(&n)))
            {
                fields.insert(key.clone(), value.clone());
            }
        }
    }
    Value::Object(fields)
}
fn bounded_read(path: &Path, limit: u64) -> io::Result<String> {
    let mut text = String::new();
    File::open(path)?
        .take(limit + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > limit {
        return Err(io::Error::other("diagnostic input exceeds limit"));
    }
    Ok(text)
}
pub fn process_metrics(pid: u32) -> Value {
    let root = std::path::PathBuf::from(format!("/proc/{pid}"));
    let read = || -> io::Result<Value> {
        let status = bounded_read(&root.join("status"), 65536)?;
        let fields: BTreeMap<_, _> = status
            .lines()
            .filter_map(|line| line.split_once(':'))
            .collect();
        let counter = |name: &str| -> u64 {
            fields
                .get(name)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|n| n.parse().ok())
                .unwrap_or(0)
        };
        let raw = bounded_read(&root.join("stat"), 8192)?;
        let end = raw
            .rfind(')')
            .ok_or_else(|| io::Error::other("invalid process counters"))?;
        let parts: Vec<_> = raw[end + 1..].split_whitespace().collect();
        let number = |i: usize| -> io::Result<u64> {
            parts
                .get(i)
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| io::Error::other("invalid process counters"))
        };
        // SAFETY: sysconf has no pointers and returns the kernel clock frequency.
        let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if ticks <= 0 {
            return Err(io::Error::other("invalid clock frequency"));
        }
        let mut result = json!({"pid":pid,"rss_kb":counter("VmRSS"),"swap_kb":counter("VmSwap"),"threads":counter("Threads"),"minor_faults":number(7)?,"major_faults":number(9)?,"user_ms":(number(11)? as f64*1000.0/ticks as f64).round_ties_even() as u64,"system_ms":(number(12)? as f64*1000.0/ticks as f64).round_ties_even() as u64});
        if let Ok(text) = bounded_read(&root.join("io"), 8192) {
            for (key, value) in text.lines().filter_map(|l| l.split_once(':')) {
                if matches!(key, "read_bytes" | "write_bytes")
                    && let Ok(n) = value.trim().parse::<u64>()
                {
                    result[key] = json!(n);
                }
            }
        }
        Ok(result)
    };
    read().unwrap_or_else(|_| json!({"pid":pid,"unavailable":true}))
}
#[derive(Clone, Copy)]
pub enum Event {
    Start,
    Stop,
    UiSample,
    UiMissing,
    UiRestored,
    Sample,
    Stall,
}
impl Event {
    fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::UiSample => "ui_sample",
            Self::UiMissing => "ui_heartbeat_missing",
            Self::UiRestored => "ui_heartbeat_restored",
            Self::Sample => "sample",
            Self::Stall => "main_loop_stall",
        }
    }
}
#[derive(Clone, Copy)]
pub enum Phase {
    Startup,
    Idle,
    Native,
    Ipc,
    Shutdown,
}
impl Phase {
    fn code(self) -> u64 {
        self as u64
    }
    fn name(code: u64) -> &'static str {
        match code {
            0 => "startup",
            1 => "idle",
            2 => "td_receive",
            3 => "ipc",
            4 => "shutdown",
            _ => "unknown",
        }
    }
}
struct Shared {
    stop: AtomicBool,
    heartbeat: AtomicU64,
    phase: AtomicU64,
    dropped: AtomicU64,
    errors: AtomicU64,
    written: AtomicU64,
    queued: AtomicU64,
}
struct Record {
    event: Event,
    fields: Value,
}
pub struct Diagnostics {
    started: Instant,
    shared: Arc<Shared>,
    sender: mpsc::SyncSender<Record>,
    thread: Option<JoinHandle<()>>,
    watcher: Option<JoinHandle<()>>,
}
impl Diagnostics {
    pub fn start(home: &Storage) -> io::Result<Self> {
        Self::start_with(home, 4 * 1024 * 1024, 4)
    }
    pub fn start_with(home: &Storage, bytes: usize, backups: usize) -> io::Result<Self> {
        if !(128..=4 * 1024 * 1024).contains(&bytes) || backups > 4 {
            return Err(io::Error::other("invalid diagnostic bounds"));
        }
        let directory = home.create_directory(".local/state/omagram/diagnostics", 0o700)?;
        // SAFETY: owned pinned directory descriptor, permission-only operation.
        if unsafe { libc::fchmod(directory.as_raw_fd(), 0o700) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let started = Instant::now();
        let (sender, receiver) = mpsc::sync_channel::<Record>(2048);
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            heartbeat: AtomicU64::new(0),
            phase: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            written: AtomicU64::new(0),
            queued: AtomicU64::new(0),
        });
        let control = shared.clone();
        let worker = thread::Builder::new()
            .name("telebar-log-writer".into())
            .spawn(move || {
                let mut writer = Log::open(directory, bytes, backups);
                if writer.is_err() {
                    control.errors.fetch_add(1, Ordering::Relaxed);
                }
                loop {
                    match receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(record) => {
                            control.queued.fetch_sub(1, Ordering::Relaxed);
                            if let Ok(log) = &mut writer {
                                if log
                                    .write(record.event, &record.fields, started.elapsed())
                                    .is_ok()
                                {
                                    control.written.fetch_add(1, Ordering::Relaxed);
                                } else {
                                    control.errors.fetch_add(1, Ordering::Relaxed);
                                    writer = Err(io::Error::other("diagnostic write failed"));
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if control.stop.load(Ordering::Relaxed) {
                                break;
                            }
                        }
                    }
                }
            })?;
        let control = shared.clone();
        let events = sender.clone();
        let watcher=match thread::Builder::new().name("telebar-watchdog".into()).spawn(move||{
            let mut last_stall=None;let mut last_sample=0;
            while !control.stop.load(Ordering::Relaxed){
                thread::sleep(Duration::from_millis(100));
                if control.stop.load(Ordering::Relaxed){break;}
                let now=started.elapsed().as_millis() as u64;
                let lag=now.saturating_sub(control.heartbeat.load(Ordering::Relaxed));
                let mut records=Vec::new();
                if lag>=2500 && last_stall.is_none_or(|last|now.saturating_sub(last)>=15000){
                    last_stall=Some(now);
                    records.push(Record{event:Event::Stall,fields:json!({"lag_ms":lag,"phase":Phase::name(control.phase.load(Ordering::Relaxed)),"stack_capture_supported":false})});
                }
                if now.saturating_sub(last_sample)>=5000{last_sample=now;records.push(Record{event:Event::Sample,fields:json!({"process":process_metrics(std::process::id())})});}
                for record in records{
                    control.queued.fetch_add(1,Ordering::Relaxed);
                    if events.try_send(record).is_err(){control.queued.fetch_sub(1,Ordering::Relaxed);control.dropped.fetch_add(1,Ordering::Relaxed);}
                }
            }
        }){Ok(watcher)=>watcher,Err(error)=>{shared.stop.store(true,Ordering::Relaxed);drop(sender);let _=worker.join();return Err(error);}};
        let log = Self {
            started,
            shared,
            sender,
            thread: Some(worker),
            watcher: Some(watcher),
        };
        log.emit(Event::Start, json!({}));
        Ok(log)
    }
    pub fn progress(&self, phase: Phase) {
        self.shared.phase.store(phase.code(), Ordering::Relaxed);
        self.shared
            .heartbeat
            .store(self.started.elapsed().as_millis() as u64, Ordering::Relaxed);
    }
    pub fn phase(&self) -> &'static str {
        Phase::name(self.shared.phase.load(Ordering::Relaxed))
    }
    pub fn emit(&self, event: Event, fields: Value) {
        // Caller supplies only locally constructed bounded numeric structures.
        // Strip strings recursively except fixed surface/phase tags.
        fn clean(v: &Value, depth: usize) -> Value {
            if depth > 6 {
                return Value::Null;
            }
            match v {
                Value::String(s)
                    if matches!(
                        s.as_str(),
                        "window" | "shell" | "startup" | "idle" | "td_receive" | "ipc" | "shutdown"
                    ) =>
                {
                    v.clone()
                }
                Value::String(_) => Value::Null,
                Value::Number(_) | Value::Bool(_) | Value::Null => v.clone(),
                Value::Array(a) => {
                    Value::Array(a.iter().take(32).map(|v| clean(v, depth + 1)).collect())
                }
                Value::Object(o) => Value::Object(
                    o.iter()
                        .take(80)
                        .filter(|(k, _)| {
                            FIELDS.contains(&k.as_str())
                                || matches!(
                                    k.as_str(),
                                    "peer_pid"
                                        | "surface"
                                        | "metrics"
                                        | "process"
                                        | "gap_ms"
                                        | "age_ms"
                                        | "lag_ms"
                                        | "phase"
                                        | "stack_capture_supported"
                                        | "pid"
                                        | "rss_kb"
                                        | "swap_kb"
                                        | "threads"
                                        | "minor_faults"
                                        | "major_faults"
                                        | "user_ms"
                                        | "system_ms"
                                        | "read_bytes"
                                        | "write_bytes"
                                        | "unavailable"
                                )
                        })
                        .map(|(k, v)| (k.clone(), clean(v, depth + 1)))
                        .collect(),
                ),
            }
        }
        let fields = clean(&fields, 0);
        if fields.to_string().len() > 60 * 1024 {
            self.shared.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        self.shared.queued.fetch_add(1, Ordering::Relaxed);
        if self.sender.try_send(Record { event, fields }).is_err() {
            self.shared.queued.fetch_sub(1, Ordering::Relaxed);
            self.shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn counters(&self) -> Value {
        json!({"logger_queue":self.shared.queued.load(Ordering::Relaxed),"logger_dropped":self.shared.dropped.load(Ordering::Relaxed),"logger_write_errors":self.shared.errors.load(Ordering::Relaxed),"logger_written":self.shared.written.load(Ordering::Relaxed)})
    }
}
impl Drop for Diagnostics {
    fn drop(&mut self) {
        self.emit(Event::Stop, json!({}));
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
        if let Some(worker) = self.thread.take() {
            let _ = worker.join();
        }
    }
}
struct Log {
    directory: File,
    file: File,
    size: u64,
    limit: usize,
    backups: usize,
}
impl Log {
    fn open(directory: File, limit: usize, backups: usize) -> io::Result<Self> {
        let file = Self::file(&directory)?;
        let size = file.metadata()?.len();
        Ok(Self {
            directory,
            file,
            size,
            limit,
            backups,
        })
    }
    fn file(directory: &File) -> io::Result<File> {
        // SAFETY: pinned parent and fixed basename; nonblocking/no-follow open.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                c"trace.jsonl".as_ptr(),
                libc::O_WRONLY
                    | libc::O_APPEND
                    | libc::O_CREAT
                    | libc::O_NOFOLLOW
                    | libc::O_NONBLOCK
                    | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fresh unique descriptor returned above.
        let file = unsafe { File::from_raw_fd(fd) };
        let meta = file.metadata()?;
        if !meta.is_file() || meta.uid() != unsafe { libc::getuid() } || meta.nlink() != 1 {
            return Err(io::Error::other("unsafe diagnostic log"));
        }
        if unsafe { libc::fchmod(fd, 0o600) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(file)
    }
    fn write(&mut self, event: Event, fields: &Value, mono: Duration) -> io::Result<()> {
        let mut record = fields.as_object().cloned().unwrap_or_default();
        record.insert("event".into(), json!(event.name()));
        record.insert("pid".into(), json!(std::process::id()));
        record.insert(
            "time".into(),
            json!(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64()
            ),
        );
        record.insert("mono".into(), json!(mono.as_secs_f64()));
        let mut data = Value::Object(record).to_string().into_bytes();
        data.push(b'\n');
        if data.len() > self.limit {
            return Err(io::Error::other("diagnostic record exceeds limit"));
        }
        if self.size + data.len() as u64 > self.limit as u64 {
            for i in (1..=self.backups).rev() {
                let source = std::ffi::CString::new(if i == 1 {
                    "trace.jsonl".into()
                } else {
                    format!("trace.jsonl.{}", i - 1)
                })
                .unwrap();
                let target = std::ffi::CString::new(format!("trace.jsonl.{i}")).unwrap();
                // SAFETY: fixed entries in pinned directory, rename never follows links.
                if unsafe {
                    libc::renameat(
                        self.directory.as_raw_fd(),
                        source.as_ptr(),
                        self.directory.as_raw_fd(),
                        target.as_ptr(),
                    )
                } != 0
                    && io::Error::last_os_error().kind() != io::ErrorKind::NotFound
                {
                    return Err(io::Error::last_os_error());
                }
            }
            if self.backups == 0
                && unsafe { libc::unlinkat(self.directory.as_raw_fd(), c"trace.jsonl".as_ptr(), 0) }
                    != 0
            {
                return Err(io::Error::last_os_error());
            }
            self.file = Self::file(&self.directory)?;
            self.size = 0;
        }
        self.file.write_all(&data)?;
        self.size += data.len() as u64;
        Ok(())
    }
}
#[derive(Default)]
pub struct Peers {
    rows: BTreeMap<u64, Peer>,
}
struct Peer {
    pid: u32,
    seen: Option<Instant>,
    metrics: Value,
    stalled: bool,
}
impl Peers {
    pub fn connect(&mut self, peer: u64, pid: u32) {
        if self.rows.len() < 16 {
            self.rows.entry(peer).or_insert(Peer {
                pid,
                seen: None,
                metrics: json!({}),
                stalled: false,
            });
        }
    }
    pub fn retire(&mut self, peer: u64) {
        self.rows.remove(&peer);
    }
    pub fn report(
        &mut self,
        peer: u64,
        args: &Value,
        now: Instant,
        log: Option<&Diagnostics>,
        window: bool,
    ) -> Result<(), String> {
        let row = self
            .rows
            .get_mut(&peer)
            .ok_or("the connection was closed")?;
        if row
            .seen
            .is_some_and(|seen| now.saturating_duration_since(seen) < Duration::from_secs(1))
        {
            return Err("diagnostic reports are rate limited".into());
        }
        if row.stalled
            && let (Some(log), Some(seen)) = (log, row.seen)
        {
            log.emit(Event::UiRestored,json!({"peer_pid":row.pid,"gap_ms":now.saturating_duration_since(seen).as_millis()}));
        }
        row.seen = Some(now);
        row.stalled = false;
        row.metrics = numeric_fields(args);
        if let Some(log) = log {
            log.emit(Event::UiSample,json!({"peer_pid":row.pid,"surface":if window{"window"}else{"shell"},"metrics":row.metrics}));
        }
        Ok(())
    }
    pub fn view(
        &mut self,
        now: Instant,
        windows: &std::collections::HashSet<u64>,
        log: Option<&Diagnostics>,
    ) -> Value {
        json!(self.rows.iter_mut().map(|(peer,row)|{
            let age=row.seen.map(|seen|now.saturating_duration_since(seen).as_millis());let surface=if windows.contains(peer){"window"}else{"shell"};
            if age.is_some_and(|age|age>10000)&&!row.stalled{row.stalled=true;if let Some(log)=log{log.emit(Event::UiMissing,json!({"peer_pid":row.pid,"surface":surface,"age_ms":age}));}}
            json!({"surface":surface,"age_ms":age,"process":process_metrics(row.pid),"metrics":row.metrics})
        }).collect::<Vec<_>>())
    }
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
