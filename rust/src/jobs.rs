//! Admitted finite helper work. Workers never access TDLib or mutable account
//! state. Credential/startup work uses explicit redacted Secret Service context;
//! the main-thread Engine/Driver owns request tokens and continuations.
use crate::{
    batch::{Aggregate, Batch},
    commands::{Action, Answer, Result, arg_int, reply, topic},
    projection::number,
    sending,
    state::State,
    storage::Storage,
    uploads,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
#[derive(Debug)]
pub enum Work {
    StartBridge(crate::bridge::Context),
    // Planned on the owning thread after the earlier global operation settles.
    DeferredSettings {
        command: String,
        args: Value,
    },
    GlobalShortcuts(crate::global_shortcuts::Job),
    StopCapture(Box<crate::capture::Stop>),
    ProfilePhoto(crate::profile_photo::Job),
    Sound(crate::sound_playback::Job),
    CopyImage {
        path: String,
        roots: Vec<String>,
        context: crate::clipboard::Context,
    },
    Cache(crate::sticker_cache::Job),
    Startup {
        context: crate::startup::Context,
        account: String,
    },
    Credentials(crate::keyring::CredentialSave),
    Recording(crate::recording_jobs::Preparation),
    Clipboard {
        context: crate::clipboard::Context,
        files: bool,
    },
    Save {
        path: String,
        roots: Vec<String>,
        destination: crate::files::Save,
    },
    Upload {
        files: Vec<uploads::Upload>,
        home: PathBuf,
        sent_directory: String,
        args: Value,
        caption: Value,
        options: Value,
        album: bool,
    },
}
impl Work {
    pub fn prepare_upload(
        command: &str,
        args: &Value,
        state: &State,
        home: PathBuf,
        databases: Vec<PathBuf>,
        now: f64,
        execute: &mut dyn FnMut(&Value) -> Option<Value>,
    ) -> Result<Self> {
        arg_int(args, "chatId", None, None, None)?;
        let album = command == "message.sendFiles";
        let paths = if album {
            args["paths"]
                .as_array()
                .filter(|a| (1..=10).contains(&a.len()))
                .ok_or("paths is a list of 1 to 10 files")?
                .clone()
        } else {
            vec![args["path"].clone()]
        };
        if album && args.get("asMedia").is_some_and(|v| !v.is_boolean()) {
            return Err("asMedia is true or false".into());
        }
        let caption = args
            .get("caption")
            .unwrap_or(&Value::String(String::new()))
            .as_str()
            .filter(|s| s.chars().count() <= 2048)
            .ok_or("a caption is text of at most 1024 characters")?
            .to_owned();
        let caption = sending::typed_text(&caption, 1024, "a caption", execute)?;
        reply(args)?;
        topic(args)?;
        let options = sending::options(args, state, now)?;
        // Validate the entire chosen set before emitting any worker/native work.
        let files = paths
            .iter()
            .map(|path| uploads::admit(path, &databases))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self::Upload {
            files,
            home,
            sent_directory: ".local/share/omagram/sent".into(),
            args: args.clone(),
            caption,
            options,
            album,
        })
    }
    pub fn set_sent_directory(&mut self, id: &str) -> Result<()> {
        if !crate::paths::valid_account_id(id) {
            return Err("unknown account".into());
        }
        let Self::Upload { sent_directory, .. } = self else {
            return Err("not an upload job".into());
        };
        *sent_directory = if id == "default" {
            ".local/share/omagram/sent".into()
        } else {
            format!(".local/share/omagram/accounts/{id}/sent")
        };
        Ok(())
    }
    fn run(self, cancel: &AtomicBool) -> Result<Action> {
        match self {
            Self::StartBridge(context) => context.start(cancel),
            Self::DeferredSettings { .. } => Err("invalid deferred settings worker".into()),
            Self::GlobalShortcuts(job) => job.run(cancel),
            Self::StopCapture(job) => job.run(cancel),
            Self::ProfilePhoto(job) => job.run(cancel),
            Self::Sound(job) => job.run(cancel),
            Self::CopyImage {
                path,
                roots,
                context,
            } => crate::clipboard::prepare_copy(&path, &roots, context, cancel),
            Self::Cache(job) => job.run(cancel),
            Self::Startup { context, account } => {
                Ok(Action::StartupPrepared(context.run(&account, cancel)))
            }
            Self::Credentials(save) => save.run(cancel),
            Self::Recording(preparation) => preparation.run(cancel),
            Self::Clipboard { context, files } => crate::clipboard::read(context, files, cancel),
            Self::Save {
                path,
                roots,
                destination,
            } => crate::files::save(&path, &roots, destination, cancel),
            Self::Upload {
                files,
                home,
                sent_directory,
                args,
                caption,
                options,
                album,
            } => {
                let storage = Storage::open(&home).map_err(|_| "that file is not readable")?;
                let media = if album {
                    args.get("asMedia").unwrap_or(&Value::Bool(true)) == &Value::Bool(true)
                } else {
                    args.get("asPhoto").unwrap_or(&Value::Bool(true)) == &Value::Bool(true)
                };
                let mut lease = crate::upload_lease::Lease::new(home.clone());
                let mut kinds = Vec::new();
                let mut contents = Vec::new();
                let started = Instant::now();
                let deadline = started + Duration::from_secs(150);
                for (i, mut upload) in files.into_iter().enumerate() {
                    if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
                        return Err("the connection was closed".into());
                    }
                    let kind =
                        uploads::kind(upload.path.to_str().unwrap_or(""), upload.size, media);
                    let relative = storage
                        .snapshot_upload(
                            &sent_directory,
                            &mut upload.file,
                            upload.size,
                            upload
                                .path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or(""),
                            cancel,
                            deadline,
                        )
                        .map_err(|_| "that file is not readable")?;
                    lease.add(relative.clone());
                    let snapshot = home.join(relative);
                    let snapshot = snapshot.to_str().ok_or("that file is not readable")?;
                    let probed = if matches!(kind, "video" | "audio") {
                        crate::media::probe_cancel(snapshot, cancel)
                    } else {
                        [0; 3]
                    };
                    kinds.push(kind.to_owned());
                    contents.push(uploads::content(
                        snapshot,
                        kind,
                        if i == 0 { caption.clone() } else { Value::Null },
                        probed,
                    ));
                }
                if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    return Err("the connection was closed".into());
                }
                let chat = number(arg_int(&args, "chatId", None, None, None)?);
                let topic = topic(&args)?;
                let reply = reply(&args)?;
                if !album {
                    return Ok(Action::Native {
                        query: json!({"@type":"sendMessage","chat_id":chat,"topic_id":topic,"options":options,"reply_to":reply,"reply_markup":null,"input_message_content":contents.remove(0)}),
                        answer: Answer::Upload(lease, false),
                    });
                }
                let groups = uploads::groups(&kinds);
                let count = groups.len();
                let queries=groups.into_iter().enumerate().map(|(i,group)|{
                    // Each group owns only its own inputs: another group's
                    // rejection cannot delete already-dispatched album files.
                    let mut group_lease = crate::upload_lease::Lease::new(home.clone());
                    for index in &group { group_lease.add(lease.path(*index).to_owned()); }
                    let mut q=json!({"chat_id":chat,"topic_id":topic,"options":options,"reply_to":if i==0{reply.clone()}else{Value::Null}});
                    if group.len()==1{q["@type"]=json!("sendMessage");q["reply_markup"]=Value::Null;q["input_message_content"]=contents[group[0]].clone();}else{q["@type"]=json!("sendMessageAlbum");q["input_message_contents"]=json!(group.into_iter().map(|n|contents[n].clone()).collect::<Vec<_>>());}
                    (i.to_string(),q,Answer::Upload(group_lease, true))
                }).collect();
                lease.submitted(); // ownership transferred to independent groups
                Ok(Action::Batch(Batch {
                    queries,
                    aggregate: Aggregate::Uploads { groups: count },
                }))
            }
        }
    }
}
struct Worker {
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}
pub struct Completion {
    pub token: crate::lifecycle::Token,
    pub result: Result<Action>,
}
pub struct Pool {
    workers: HashMap<crate::lifecycle::Token, Worker>,
    sender: mpsc::SyncSender<Completion>,
    receiver: mpsc::Receiver<Completion>,
}
impl Default for Pool {
    fn default() -> Self {
        let (sender, receiver) = mpsc::sync_channel(2);
        Self {
            workers: HashMap::new(),
            sender,
            receiver,
        }
    }
}
impl Pool {
    pub fn active(&self) -> usize {
        self.workers.len()
    }
    pub fn start(&mut self, token: crate::lifecycle::Token, work: Work) -> Result<()> {
        if self.workers.len() >= 2 || self.workers.contains_key(&token) {
            return Err("still preparing an earlier message; try again in a moment".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let control = cancel.clone();
        let sender = self.sender.clone();
        let thread = thread::Builder::new()
            .name("telebar-helper".into())
            .spawn(move || {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work.run(&control)))
                        .unwrap_or_else(|_| Err("the request failed".into()));
                // At most two workers/completions; never a blocking unowned publisher.
                let _ = sender.try_send(Completion { token, result });
            })
            .map_err(|_| "the request failed")?;
        self.workers.insert(token, Worker { cancel, thread });
        Ok(())
    }
    pub fn poll(&mut self, current: &dyn Fn(crate::lifecycle::Token) -> bool) -> Vec<Completion> {
        for (token, worker) in &self.workers {
            if !current(*token) {
                worker.cancel.store(true, Ordering::Relaxed);
            }
        }
        let mut results = Vec::new();
        while let Ok(done) = self.receiver.try_recv() {
            if let Some(worker) = self.workers.remove(&done.token) {
                let _ = worker.thread.join();
                results.push(done);
            }
        }
        results
    }
}
impl Drop for Pool {
    fn drop(&mut self) {
        for worker in self.workers.values() {
            worker.cancel.store(true, Ordering::Relaxed);
        }
        for (_, worker) in self.workers.drain() {
            let _ = worker.thread.join();
        }
    }
}
