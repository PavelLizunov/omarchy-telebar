//! One native/thread driver. Explicit account registrations and parameter
//! provider; no discovery of working accounts, keyring or real-home storage.
use crate::projection::number;
use crate::{authorization::Runtime, engine::Effects, td::Td, wire::Request};
use serde_json::{Value, json};
use std::{
    io,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub type LocalHandler<'a> = dyn FnMut(
        &str,
        &Value,
        &crate::state::State,
    ) -> Option<crate::commands::Result<crate::commands::Action>>
    + 'a;
struct ClipboardOwner {
    token: Option<crate::lifecycle::Token>,
    helper: crate::process::Managed,
    started: Instant,
    verification: Option<crate::process::Managed>,
    environment: std::collections::BTreeMap<String, String>,
    digest: [u8; 32],
}
struct StartupOwner {
    account: String,
    client: i32,
    generation: u64,
    serial: u64,
    auth: Value,
    started: Instant,
}
impl StartupOwner {
    fn current(&self, runtime: &Runtime) -> bool {
        !runtime.stopping
            && runtime
                .engine
                .accounts
                .get(&self.account)
                .is_some_and(|a| a.client == self.client && a.generation == self.generation)
            && runtime
                .metadata
                .get(&self.account)
                .is_some_and(|m| m.parameters_serial == self.serial && m.auth == self.auth)
    }
}
pub struct Driver {
    pub runtime: Runtime,
    td: Td,
    language: String,
    jobs: crate::jobs::Pool,
    startup_jobs: crate::jobs::Pool,
    startup_context: Option<crate::startup::Context>,
    startup_owners: std::collections::HashMap<crate::lifecycle::Token, StartupOwner>,
    startup_serial: u64,
    startup_queue: std::collections::VecDeque<StartupOwner>,
    sound: Option<(crate::process::Managed, crate::sound_playback::Prepared)>,
    pub capture_context: Option<crate::capture::Context>,
    capture: Option<crate::capture::Capture>,
    file_openers: Vec<(crate::process::Managed, std::fs::File)>,
    sound_jobs: crate::jobs::Pool,
    sound_owners: std::collections::HashMap<crate::lifecycle::Token, (String, u64)>,
    clipboard_owner: Option<ClipboardOwner>,
    playback_serial: u64,
    player: Option<(String, u64, crate::process::Managed, std::fs::File)>,
}
impl Driver {
    pub fn open(library: &Path, active: String, language: String) -> io::Result<Self> {
        Ok(Self {
            runtime: Runtime::new(active),
            td: Td::open(library)?,
            language,
            jobs: crate::jobs::Pool::default(),
            startup_jobs: crate::jobs::Pool::default(),
            startup_context: None,
            startup_owners: std::collections::HashMap::new(),
            startup_serial: 1,
            startup_queue: std::collections::VecDeque::new(),
            sound: None,
            capture_context: None,
            capture: None,
            file_openers: vec![],
            sound_jobs: Default::default(),
            sound_owners: Default::default(),
            clipboard_owner: None,
            playback_serial: 0,
            player: None,
        })
    }
    pub fn set_startup_context(&mut self, context: crate::startup::Context) {
        self.startup_context = Some(context);
    }
    fn start_parameters(&mut self, id: &str) -> Effects {
        let effects = Effects::default();
        if self.startup_context.is_none() {
            return effects;
        }
        if self.runtime.stopping || self.runtime.parameters_pending(id) {
            return effects;
        }
        let Some(account) = self
            .runtime
            .engine
            .accounts
            .get(id)
            .filter(|a| a.client > 0 && !a.ready)
        else {
            return effects;
        };
        if self
            .startup_owners
            .values()
            .chain(self.startup_queue.iter())
            .any(|p| p.account == id && p.current(&self.runtime))
        {
            return effects;
        }
        self.startup_queue.retain(|p| p.account != id);
        if self.startup_queue.len() >= 10 {
            return self.runtime.configure(
                id,
                Err("the startup worker capacity was exhausted".into()),
                Instant::now(),
            );
        }
        let Some(meta) = self.runtime.metadata.get_mut(id) else {
            return effects;
        };
        meta.parameters_serial = meta.parameters_serial.saturating_add(1);
        let owner = StartupOwner {
            account: id.into(),
            client: account.client,
            generation: account.generation,
            serial: meta.parameters_serial,
            auth: meta.auth.clone(),
            started: Instant::now(),
        };
        self.startup_queue.push_back(owner);
        effects
    }
    fn dispatch_startup(&mut self) -> Effects {
        let mut effects = Effects::default();
        self.startup_queue.retain(|p| p.current(&self.runtime));
        // Queue time is part of the attempt, not a fresh budget on dispatch.
        let mut waiting = std::collections::VecDeque::new();
        for owner in self.startup_queue.drain(..) {
            if owner.started.elapsed() >= Duration::from_secs(180) {
                let failed = self.runtime.configure(
                    &owner.account,
                    Err("the startup credential preparation deadline was exceeded".into()),
                    Instant::now(),
                );
                effects.replies.extend(failed.replies);
            } else {
                waiting.push_back(owner);
            }
        }
        self.startup_queue = waiting;
        while self.jobs.active() + self.startup_jobs.active() + self.sound_jobs.active() < 2 {
            let Some(owner) = self.startup_queue.pop_front() else {
                break;
            };
            let id = owner.account.clone();
            let Some(next) = self.startup_serial.checked_add(1) else {
                let failed = self.runtime.configure(
                    &id,
                    Err("the startup worker capacity was exhausted".into()),
                    Instant::now(),
                );
                effects.replies.extend(failed.replies);
                continue;
            };
            let token = crate::lifecycle::Token::startup(self.startup_serial);
            self.startup_serial = next;
            let Some(context) = self.startup_context.clone() else {
                continue;
            };
            let work = crate::jobs::Work::Startup {
                context,
                account: id.clone(),
            };
            if self.startup_jobs.start(token, work).is_ok() {
                self.startup_owners.insert(token, owner);
            } else {
                let failed = self.runtime.configure(
                    &id,
                    Err("the startup worker could not be started".into()),
                    Instant::now(),
                );
                effects.replies.extend(failed.replies);
            }
        }
        effects
    }
    pub fn version(&mut self) -> io::Result<Option<Value>> {
        self.td
            .execute(&json!({"@type":"getOption","name":"version"}))
    }
    pub fn register(
        &mut self,
        id: String,
        name: String,
        roots: Vec<String>,
    ) -> io::Result<Effects> {
        let client = self.td.create_client()?;
        let effects = self
            .runtime
            .register(id, name, client, roots, Instant::now())
            .map_err(io::Error::other)?;
        self.dispatch(effects)
    }
    fn dispatch(&mut self, mut effects: Effects) -> io::Result<Effects> {
        for (token, start) in std::mem::take(&mut effects.playback) {
            let next = self.complete_playback(token, start, Instant::now());
            effects.replies.extend(next.replies);
            effects.native.extend(next.native);
        }
        for (token, start) in std::mem::take(&mut effects.open_files) {
            effects
                .replies
                .extend(self.runtime.expire(Instant::now()).replies);
            let current = !self.runtime.stopping
                && self
                    .runtime
                    .engine
                    .job_owner(token)
                    .is_some_and(|(id, generation)| {
                        self.runtime
                            .engine
                            .accounts
                            .get(&id)
                            .is_some_and(|a| a.client > 0 && a.generation == generation)
                    });
            if !current {
                continue;
            }
            let result = if self.file_openers.len() >= 8 {
                Err("still opening an earlier file".into())
            } else {
                start.spawn().map(|helper| {
                    self.file_openers.push(helper);
                    crate::commands::Action::Reply(json!({}))
                })
            };
            effects.replies.extend(
                self.runtime
                    .engine
                    .job_complete(token, result, Instant::now())
                    .replies,
            );
        }
        for (token, prepared) in std::mem::take(&mut effects.sounds) {
            effects
                .replies
                .extend(self.runtime.expire(Instant::now()).replies);
            let current = !self.runtime.stopping
                && self
                    .runtime
                    .engine
                    .job_owner(token)
                    .is_some_and(|(id, generation)| {
                        self.runtime
                            .engine
                            .accounts
                            .get(&id)
                            .is_some_and(|a| a.client > 0 && a.generation == generation)
                    });
            if !current {
                continue;
            }
            self.sound = None;
            if let Ok(helper) = prepared.spawn() {
                self.sound = Some((helper, prepared));
            }
            // Sound previews are best-effort, just as the original command:
            // lack of a local player does not undo persisted settings.
            effects.replies.extend(
                self.runtime
                    .engine
                    .job_complete(
                        token,
                        Ok(crate::commands::Action::Reply(json!({}))),
                        Instant::now(),
                    )
                    .replies,
            );
        }
        for (token, offer) in std::mem::take(&mut effects.clipboard) {
            effects
                .replies
                .extend(self.runtime.expire(Instant::now()).replies);
            let current = !self.runtime.stopping
                && self
                    .runtime
                    .engine
                    .job_owner(token)
                    .is_some_and(|(id, generation)| {
                        self.runtime
                            .engine
                            .accounts
                            .get(&id)
                            .is_some_and(|a| a.client > 0 && a.generation == generation)
                    });
            if !current {
                effects.replies.extend(
                    self.runtime
                        .engine
                        .job_complete(
                            token,
                            Err("the account session was closed".into()),
                            Instant::now(),
                        )
                        .replies,
                );
                continue;
            }
            if let Some(old) = self.clipboard_owner.take()
                && let Some(token) = old.token
            {
                let done = self.runtime.engine.job_complete(
                    token,
                    Err("the clipboard selection changed".into()),
                    Instant::now(),
                );
                effects.replies.extend(done.replies);
            }
            match offer.spawn() {
                Ok(helper) => {
                    use sha2::Digest;
                    self.clipboard_owner = Some(ClipboardOwner {
                        token: Some(token),
                        helper,
                        started: Instant::now(),
                        verification: None,
                        environment: offer.environment,
                        digest: sha2::Sha256::digest(&offer.data).into(),
                    })
                }
                Err(e) => effects.replies.extend(
                    self.runtime
                        .engine
                        .job_complete(token, Err(e), Instant::now())
                        .replies,
                ),
            }
        }
        for (token, work) in std::mem::take(&mut effects.work) {
            let started = if self.jobs.active()
                + self.startup_jobs.active()
                + self.sound_jobs.active()
                >= 2
            {
                Err("still preparing an earlier message; try again in a moment".into())
            } else {
                self.jobs.start(token, work)
            };
            if let Err(error) = started {
                let rejected = self
                    .runtime
                    .engine
                    .job_complete(token, Err(error), Instant::now());
                effects.replies.extend(rejected.replies);
            }
        }
        for native in std::mem::take(&mut effects.native) {
            self.runtime.engine.native_dispatched(&native.query);
            self.td.send(native.client, &native.query)?;
        }
        Ok(effects)
    }
    fn complete_playback(
        &mut self,
        token: crate::lifecycle::Token,
        start: crate::playback::Start,
        now: Instant,
    ) -> Effects {
        let mut effects = self.runtime.expire(now);
        let Some((account, generation)) = self.runtime.engine.job_owner(token) else {
            return effects;
        };
        let current = !self.runtime.stopping
            && self
                .runtime
                .engine
                .accounts
                .get(&account)
                .is_some_and(|a| a.client > 0 && a.generation == generation);
        let result = if !current {
            Err("the account session was closed".into())
        } else if start.selection.serial != self.playback_serial {
            Err("the playback selection changed".into())
        } else {
            if let Some((old, _, _, _)) = self.player.take() {
                effects
                    .replies
                    .push((0, json!({"event":"playing","fileId":0,"account":old})));
            }
            start.spawn().map(|(helper,source)|{
                    let started=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
                    self.player=Some((account.clone(),generation,helper,source));
                    let seen=self.runtime.open_content(&account,start.selection.chat,start.selection.message,now);
                    effects.native.extend(seen.native);
                    effects.replies.push((0,json!({"event":"playing","fileId":number(start.selection.file),"chatId":number(start.selection.chat),"messageId":number(start.selection.message),"startedAt":started,"rate":start.selection.rate,"account":account})));
                    crate::commands::Action::Reply(json!({"startedAt":started}))
                })
        };
        let done = self.runtime.engine.job_complete(token, result, now);
        effects.replies.extend(done.replies);
        effects
    }
    fn poll_player(&mut self) -> Effects {
        let mut effects = Effects::default();
        let stop = if let Some((account, generation, helper, _)) = &mut self.player {
            self.runtime.stopping
                || !self
                    .runtime
                    .engine
                    .accounts
                    .get(account)
                    .is_some_and(|a| a.client > 0 && a.generation == *generation)
                || helper.poll().map_or(true, |out| out.is_some())
        } else {
            false
        };
        if stop && let Some((account, _, _, _)) = self.player.take() {
            effects
                .replies
                .push((0, json!({"event":"playing","fileId":0,"account":account})));
        }
        effects
    }
    pub fn request(&mut self, peer: u64, request: Request) -> io::Result<Effects> {
        self.request_with_local(peer, request, &mut |_, _, _| None)
    }
    pub fn request_with_local(
        &mut self,
        peer: u64,
        request: Request,
        local: &mut LocalHandler<'_>,
    ) -> io::Result<Effects> {
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_secs_f64();
        self.runtime
            .engine
            .reserve_external(self.runtime.internal_pending());
        let effects = self.runtime.engine.request_using(
            peer,
            request,
            Instant::now(),
            &mut |command, args, state| {
                if let Some(action) = local(command, args, state) {
                    return action;
                }
                crate::sending::plan(command, args, state, wall, &self.language, &mut |query| {
                    self.td.execute(query).ok().flatten()
                })
            },
        );
        self.dispatch(effects)
    }
    pub fn request_application(
        &mut self,
        application: &mut crate::application::Application,
        home: &crate::storage::Storage,
        peer: u64,
        request: Request,
    ) -> io::Result<Effects> {
        self.runtime
            .engine
            .reserve_external(self.runtime.internal_pending());
        if matches!(request.command.as_str(), "media.play" | "media.stop") {
            let stopping = request.command == "media.stop";
            let mut selected = false;
            let next = self.playback_serial.checked_add(1);
            let environment = application
                .clipboard_context
                .as_ref()
                .map(|c| c.environment.clone());
            let rate = application.settings.value["playbackRate"]
                .as_f64()
                .unwrap_or(1.0);
            let mut effects = self.runtime.engine.request_using(
                peer,
                request,
                Instant::now(),
                &mut |command, args, _| {
                    let serial = next.ok_or("the playback selection changed")?;
                    let action = if command == "media.play" {
                        crate::playback::Selection::prepare(
                            args,
                            serial,
                            rate,
                            environment
                                .clone()
                                .ok_or("explicit playback context missing")?,
                        )?
                    } else {
                        crate::commands::Action::Reply(json!({}))
                    };
                    selected = true;
                    Ok(action)
                },
            );
            if selected {
                self.playback_serial = next.expect("admitted serial");
                if stopping && let Some((account, _, _, _)) = self.player.take() {
                    effects.replies.insert(
                        0,
                        (0, json!({"event":"playing","fileId":0,"account":account})),
                    );
                }
            }
            return self.dispatch(effects);
        }
        if matches!(
            request.command.as_str(),
            "voice.start" | "videonote.record" | "voice.stop" | "videonote.stop"
        ) {
            let starting = matches!(request.command.as_str(), "voice.start" | "videonote.record");
            let kind = if request.command.starts_with("voice.") {
                "voice"
            } else {
                "video"
            };
            let context = self.capture_context.clone();
            let capture = &mut self.capture;
            let mut transition = None;
            let aid = self.runtime.engine.routed_account(&request);
            let generation = aid
                .as_ref()
                .and_then(|id| self.runtime.engine.accounts.get(id))
                .map(|a| a.generation);
            let capacity =
                self.jobs.active() + self.startup_jobs.active() + self.sound_jobs.active() < 2;
            let wall = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            let mut effects=self.runtime.engine.request_using(peer,request,Instant::now(),&mut|_,args,state|{
                let context=context.as_ref().ok_or("explicit recorder context missing")?;let aid=aid.clone().ok_or("unknown account")?;let generation=generation.ok_or("unknown account")?;
                if starting{
                    if state.retained.record.is_some(){return Err("a recorded message is still pending; resolve or discard it first".into());}
                    let chat=crate::commands::arg_int(args,"chatId",None,None,None)?;
                    if capture.is_some(){return Err("a voice or video message is already being recorded".into());}
                    let selected=crate::capture::Capture::start(context,aid,generation,kind,chat)?;
                    let mut answer=json!({"startedAt":selected.started});let mut event=json!({"event":"recording","state":kind,"chatId":number(chat),"startedAt":selected.started,"account":selected.account});if let Some(preview)=&selected.preview{answer["preview"]=json!(preview);event["preview"]=json!(preview);}
                    *capture=Some(selected);transition=Some(event);Ok(crate::commands::Action::Reply(answer))
                }else{
                    let current=capture.as_ref().filter(|c|c.kind==kind).ok_or(if kind=="voice"{"no voice message is being recorded"}else{"no video message is being recorded"})?;
                    if current.account!=aid{return Err("the recording belongs to another account".into());}
                    if !capacity{return Err("still preparing an earlier message; try stopping again in a moment".into());}
                    let sending=args["send"]==true;
                    if sending&&state.retained.record.is_some(){return Err("a recorded message is still pending; resolve or discard it first".into());}
                    let options=if sending{crate::commands::reply(args)?;crate::commands::topic(args)?;crate::sending::options(args,state,wall)?}else{Value::Null};
                    let selected=capture.take().expect("admitted capture");let action=if sending{selected.prepare_send(args.clone(),context.home.clone(),options)?}else{selected.stop(args.clone(),context.home.clone(),options)?};
                    transition=Some(json!({"event":"recording","state":"idle","account":aid}));Ok(action)
                }
            });
            if let Some(event) = transition {
                effects.replies.insert(0, (0, event));
            }
            return self.dispatch(effects);
        }
        if request.command == "profile.setPhoto" {
            let context = application.upload_context.clone();
            let runtime = application.recording_runtime.clone();
            let effects = self.runtime.engine.request_using(
                peer,
                request,
                Instant::now(),
                &mut |_, args, _| {
                    let (_, databases) = context
                        .as_ref()
                        .ok_or("explicit profile photo context missing")?;
                    let runtime = runtime
                        .clone()
                        .ok_or("explicit profile photo context missing")?;
                    let source = crate::uploads::admit(&args["path"], databases)?;
                    Ok(crate::commands::Action::Work(
                        crate::jobs::Work::ProfilePhoto(crate::profile_photo::Job {
                            source,
                            runtime,
                        }),
                    ))
                },
            );
            return self.dispatch(effects);
        }
        if request.command == "file.open" {
            let environment = application
                .clipboard_context
                .as_ref()
                .map(|c| c.environment.clone());
            let effects = self.runtime.engine.request_using(
                peer,
                request,
                Instant::now(),
                &mut |_, args, _| {
                    crate::file_open::Selection::prepare(
                        args,
                        environment
                            .clone()
                            .ok_or("explicit opener context missing")?,
                    )
                },
            );
            return self.dispatch(effects);
        }
        if request.command == "credentials.set" {
            let environment = application.keyring_environment.clone();
            let effects = self.runtime.engine.request_using(
                peer,
                request,
                Instant::now(),
                &mut |_, args, _| {
                    let environment = environment
                        .clone()
                        .ok_or("the keyring did not accept the credentials")?;
                    Ok(crate::commands::Action::Work(
                        crate::jobs::Work::Credentials(crate::keyring::CredentialSave::prepare(
                            args,
                            environment,
                        )?),
                    ))
                },
            );
            return self.dispatch(effects);
        }
        if matches!(
            request.command.as_str(),
            "clipboard.files" | "clipboard.image"
        ) {
            let context = application.clipboard_context.clone();
            let effects = self.runtime.engine.request_using(
                peer,
                request,
                Instant::now(),
                &mut |command, _, _| {
                    let context = context.clone().ok_or("the clipboard could not be read")?;
                    Ok(crate::commands::Action::Work(
                        crate::jobs::Work::Clipboard {
                            context,
                            files: command == "clipboard.files",
                        },
                    ))
                },
            );
            return self.dispatch(effects);
        }
        if matches!(
            request.command.as_str(),
            "videonote.send" | "videonote.discard" | "recording.retry" | "recording.discard"
        ) {
            let context = application.upload_context.clone();
            let runtime = application.recording_runtime.clone();
            let protected: Vec<String> = self
                .runtime
                .engine
                .accounts
                .values()
                .filter_map(|a| a.state.retained.record.as_ref())
                .flat_map(|r| std::iter::once(r.path.clone()).chain(r.source.clone()))
                .collect();
            let account = if request.account.is_null() {
                self.runtime.engine.active.clone()
            } else {
                request.account.as_str().unwrap_or("").into()
            };
            let wall = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_secs_f64();
            let effects = self.runtime.engine.request_using(
                peer,
                request,
                Instant::now(),
                &mut |command, args, state| {
                    let home = context
                        .as_ref()
                        .ok_or("explicit recording context missing")?
                        .0
                        .clone();
                    let runtime = runtime
                        .as_ref()
                        .ok_or("explicit recording context missing")?;
                    if command == "videonote.discard"
                        && args["path"]
                            .as_str()
                            .is_some_and(|path| protected.iter().any(|p| p == path))
                    {
                        return Err(
                            "the recording is owned by a pending send; resolve it by token".into(),
                        );
                    }
                    crate::recording_jobs::plan(command, args, state, home, runtime, &account, wall)
                },
            );
            return self.dispatch(effects);
        }
        if request.command == "account.add" {
            let effects = application.add(home, &mut self.runtime, peer, request, &mut || {
                self.td.create_client()
            });
            return self.dispatch(effects);
        }
        if matches!(
            request.command.as_str(),
            "message.sendFile" | "message.sendFiles"
        ) {
            let context = application
                .upload_context
                .clone()
                .ok_or_else(|| io::Error::other("explicit upload context missing"))?;
            let wall = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_secs_f64();
            let target = if request.account.is_null() {
                self.runtime.engine.active.clone()
            } else {
                request.account.as_str().unwrap_or("").into()
            };
            let effects = self.runtime.engine.request_using(
                peer,
                request,
                Instant::now(),
                &mut |command, args, state| {
                    crate::jobs::Work::prepare_upload(
                        command,
                        args,
                        state,
                        context.0.clone(),
                        context.1.clone(),
                        wall,
                        &mut |q| self.td.execute(q).ok().flatten(),
                    )
                    .and_then(|mut work| {
                        work.set_sent_directory(&target)?;
                        Ok(crate::commands::Action::Work(work))
                    })
                },
            );
            return self.dispatch(effects);
        }
        if matches!(
            request.command.as_str(),
            "sticker.lottie" | "videonote.animation" | "storage.stats" | "storage.clear"
        ) {
            let context = application.upload_context.as_ref().map(|c| c.0.clone());
            let effects = self.runtime.engine.request_using(
                peer,
                request,
                Instant::now(),
                &mut |command, args, _| {
                    let home = context.clone().ok_or("explicit cache context missing")?;
                    crate::sticker_cache::plan(command, args, home)
                        .unwrap_or_else(|| Err("unknown command".into()))
                },
            );
            return self.dispatch(effects);
        }
        if request.command == "file.copyImage" {
            let context = application.clipboard_context.clone();
            let effects = self.runtime.engine.request_using(
                peer,
                request,
                Instant::now(),
                &mut |_, args, _| {
                    crate::export::prepare_copy(
                        args,
                        context
                            .clone()
                            .ok_or("explicit clipboard context missing")?,
                    )
                },
            );
            return self.dispatch(effects);
        }
        if request.command == "file.save" {
            let context = application
                .upload_context
                .as_ref()
                .ok_or_else(|| io::Error::other("explicit file context missing"))?;
            let effects = self.runtime.engine.request_using(
                peer,
                request,
                Instant::now(),
                &mut |_, args, _| crate::export::prepare_save(args, context.0.clone()),
            );
            return self.dispatch(effects);
        }
        let effects = if crate::application::Application::owns(&request.command) {
            application.request(home, &mut self.runtime, peer, request)
        } else {
            return self.request(peer, request);
        };
        let effects = self.dispatch(effects)?;
        self.start_pending_sounds(application);
        Ok(effects)
    }
    fn start_pending_sounds(&mut self, application: &mut crate::application::Application) {
        if self.runtime.stopping {
            application.settings.sounds_pending.clear();
            return;
        }
        for seed in std::mem::take(&mut application.settings.sounds_pending) {
            let Some((home, _)) = &application.upload_context else {
                continue;
            };
            let Some(context) = &application.clipboard_context else {
                continue;
            };
            let Some(job) = crate::sound_playback::Job::prepare(
                home.clone(),
                seed,
                &application.settings.value,
                context.environment.clone(),
            ) else {
                continue;
            };
            if self.jobs.active() + self.startup_jobs.active() + self.sound_jobs.active() >= 2 {
                continue;
            }
            let id = self.runtime.engine.active.clone();
            let Some(account) = self
                .runtime
                .engine
                .accounts
                .get(&id)
                .filter(|a| a.client > 0)
            else {
                continue;
            };
            let Some(next) = self.startup_serial.checked_add(1) else {
                continue;
            };
            let token = crate::lifecycle::Token::startup(self.startup_serial);
            self.startup_serial = next;
            if self
                .sound_jobs
                .start(token, crate::jobs::Work::Sound(job))
                .is_ok()
            {
                self.sound_owners.insert(token, (id, account.generation));
            }
        }
    }
    pub fn tick_application(
        &mut self,
        application: &mut crate::application::Application,
        home: &crate::storage::Storage,
        parameters: &mut dyn FnMut(&str) -> Result<Option<Value>, String>,
    ) -> io::Result<Effects> {
        let effects = self.tick(parameters)?;
        application.diagnostic_runtime["jobs"] =
            json!(self.jobs.active() + self.startup_jobs.active() + self.sound_jobs.active());
        application.diagnostic_runtime["task_queue"] = json!(self.startup_queue.len());
        application.diagnostic_runtime["receiver_alive"] = json!(!self.runtime.stopping);
        application.diagnostic_runtime["playing"] = json!(self.player.is_some());
        application.diagnostic_runtime["recording"] = json!(self.capture.is_some());
        application.diagnostic_runtime["children"] = json!(
            usize::from(self.player.is_some())
                + self.file_openers.len()
                + usize::from(self.capture.is_some())
                + usize::from(self.sound.is_some())
                + self
                    .clipboard_owner
                    .as_ref()
                    .map_or(0, |o| 1 + usize::from(o.verification.is_some()))
        );
        // Catalog I/O failure must not tear down TDLib or discard received
        // events. The application retains the previous catalog and failure bit.
        let _ = application.sync_accounts(home, &self.runtime, &effects);
        Ok(effects)
    }
    /// Finite receive work yields back to socket peers and helper completions.
    pub fn tick(
        &mut self,
        parameters: &mut dyn FnMut(&str) -> Result<Option<Value>, String>,
    ) -> io::Result<Effects> {
        let mut effects = Effects::default();
        for _ in 0..128 {
            let Some(event) = self.td.receive(Duration::ZERO)? else {
                break;
            };
            self.runtime
                .engine
                .reserve_external(self.runtime.internal_pending());
            if self.startup_context.is_some()
                && event["@type"] == "updateAuthorizationState"
                && event["authorization_state"]["@type"] == "authorizationStateWaitTdlibParameters"
            {
                if let Some(id) = self
                    .runtime
                    .engine
                    .accounts
                    .iter()
                    .find(|(_, a)| event["@client_id"].as_i64() == Some(i64::from(a.client)))
                    .map(|(id, _)| id.clone())
                {
                    let next = self.start_parameters(&id);
                    let next = self.dispatch(next)?;
                    effects.replies.extend(next.replies);
                }
                continue;
            }
            let next = self
                .runtime
                .receive(event, Instant::now(), parameters, &mut |query| {
                    self.td.execute(query).ok().flatten()
                });
            let next = self.dispatch(next)?;
            effects.replies.extend(next.replies);
            if !self.runtime.stopping {
                let closed: Vec<_> = self
                    .runtime
                    .engine
                    .accounts
                    .iter()
                    .filter_map(|(id, a)| (a.client == 0).then_some(id.clone()))
                    .collect();
                for id in closed {
                    let client = self.td.create_client()?;
                    let next = self
                        .runtime
                        .replace_closed_client(&id, client, Instant::now())
                        .map_err(io::Error::other)?;
                    let next = self.dispatch(next)?;
                    effects.replies.extend(next.replies);
                }
            }
        }
        // Retire expired user work before the worker cancellation predicate.
        let expired = self.runtime.expire(Instant::now());
        effects.replies.extend(expired.replies);
        let completed = self
            .jobs
            .poll(&|token| self.runtime.engine.job_current(token));
        for done in completed {
            self.runtime
                .engine
                .reserve_external(self.runtime.internal_pending());
            let mut next =
                self.runtime
                    .engine
                    .job_complete(done.token, done.result, Instant::now());
            for (id, generation) in std::mem::take(&mut next.credentials_saved) {
                let configured = if self.startup_context.is_some() {
                    let mut configured = self.runtime.credentials_start(&id, generation);
                    if !configured.replies.is_empty() {
                        let queued = self.start_parameters(&id);
                        configured.replies.extend(queued.replies);
                    }
                    configured
                } else {
                    self.runtime
                        .credentials_saved(&id, generation, Instant::now(), parameters)
                };
                // The admitted save's acknowledgement precedes session startup.
                next.replies.extend(configured.replies);
                next.native.extend(configured.native);
            }
            let next = self.dispatch(next)?;
            effects.replies.extend(next.replies);
        }
        let completed = self.startup_jobs.poll(&|token| {
            self.startup_owners.get(&token).is_some_and(|p| {
                p.current(&self.runtime) && p.started.elapsed() < Duration::from_secs(180)
            })
        });
        for done in completed {
            let next = self.complete_startup(done, Instant::now());
            let next = self.dispatch(next)?;
            effects.replies.extend(next.replies);
        }
        let completed = self.sound_jobs.poll(&|token| {
            !self.runtime.stopping
                && self
                    .sound_owners
                    .get(&token)
                    .is_some_and(|(id, generation)| {
                        self.runtime
                            .engine
                            .accounts
                            .get(id)
                            .is_some_and(|a| a.client > 0 && a.generation == *generation)
                    })
        });
        for done in completed {
            let current = self
                .sound_owners
                .remove(&done.token)
                .is_some_and(|(id, generation)| {
                    !self.runtime.stopping
                        && self
                            .runtime
                            .engine
                            .accounts
                            .get(&id)
                            .is_some_and(|a| a.client > 0 && a.generation == generation)
                });
            if current && let Ok(crate::commands::Action::SoundReady(prepared)) = done.result {
                self.sound = None;
                if let Ok(helper) = prepared.spawn() {
                    self.sound = Some((helper, prepared));
                }
            }
        }
        effects.replies.extend(self.poll_player().replies);
        effects.replies.extend(self.poll_clipboard().replies);
        let retire_capture = self.capture.as_ref().is_some_and(|c| {
            self.runtime.stopping
                || !self
                    .runtime
                    .engine
                    .accounts
                    .get(&c.account)
                    .is_some_and(|a| a.client > 0 && a.generation == c.generation)
        });
        if retire_capture && let Some(capture) = self.capture.take() {
            let account = capture.account.clone();
            capture.discard();
            effects.replies.push((
                0,
                json!({"event":"recording","state":"idle","account":account}),
            ));
        }
        self.file_openers.retain_mut(|(helper, _)| {
            !self.runtime.stopping && helper.poll().is_ok_and(|out| out.is_none())
        });
        if self.runtime.stopping
            || self
                .sound
                .as_mut()
                .is_some_and(|(helper, _)| helper.poll().map_or(true, |out| out.is_some()))
        {
            self.sound = None;
        }
        let queued = self.dispatch_startup();
        effects.replies.extend(queued.replies);
        let expired = self.runtime.expire(Instant::now());
        effects.replies.extend(expired.replies);
        Ok(effects)
    }
    fn complete_startup(&mut self, done: crate::jobs::Completion, now: Instant) -> Effects {
        let Some(owner) = self.startup_owners.remove(&done.token) else {
            return Effects::default();
        };
        if !owner.current(&self.runtime) {
            return Effects::default();
        }
        let query = if now.saturating_duration_since(owner.started) >= Duration::from_secs(180) {
            Err("the startup credential preparation deadline was exceeded".into())
        } else {
            match done.result {
                Ok(crate::commands::Action::StartupPrepared(prepared)) => prepared.0,
                _ => Err("the startup credentials could not be prepared".into()),
            }
        };
        self.runtime.configure(&owner.account, query, now)
    }
    fn poll_clipboard(&mut self) -> Effects {
        let mut effects = self.runtime.expire(Instant::now());
        let mut retire = false;
        if let Some(owner) = &mut self.clipboard_owner {
            let current = !self.runtime.stopping
                && owner.token.is_none_or(|t| {
                    self.runtime
                        .engine
                        .job_owner(t)
                        .is_some_and(|(id, generation)| {
                            self.runtime
                                .engine
                                .accounts
                                .get(&id)
                                .is_some_and(|a| a.client > 0 && a.generation == generation)
                        })
                });
            // Retirement must precede even a pipe write: a disconnected offer
            // cannot publish another chunk before the owned child is stopped.
            let ended = !current || owner.helper.poll().map_or(true, |v| v.is_some());
            if !current
                || ended
                || (owner.token.is_some() && owner.started.elapsed() >= Duration::from_secs(10))
            {
                if let Some(token) = owner.token.take() {
                    effects.replies.extend(
                        self.runtime
                            .engine
                            .job_complete(
                                token,
                                Err("the image could not be copied to the clipboard".into()),
                                Instant::now(),
                            )
                            .replies,
                    );
                }
                retire = true;
            } else if owner.token.is_some() {
                let verified = if let Some(probe) = &mut owner.verification {
                    match probe.poll() {
                        Ok(Some(output)) => {
                            use sha2::Digest;
                            Some(
                                output.ok()
                                    && <[u8; 32]>::from(sha2::Sha256::digest(&output.stdout))
                                        == owner.digest,
                            )
                        }
                        Err(_) => Some(false),
                        Ok(None) => None,
                    }
                } else if owner.helper.input_sent()
                    && owner.started.elapsed() >= Duration::from_millis(100)
                {
                    // One actual selection read, no retries or ambient clipboard
                    // discovery. Bytes stay local and are compared only by hash.
                    match crate::process::Managed::start(
                        "/usr/bin/wl-paste",
                        &["--no-newline", "--type", "image/png"],
                        &owner.environment,
                        &[],
                        Duration::from_secs(5),
                        10 * 1024 * 1024,
                    ) {
                        Ok(probe) => {
                            owner.verification = Some(probe);
                            None
                        }
                        Err(_) => Some(false),
                    }
                } else {
                    None
                };
                if let Some(accepted) = verified {
                    owner.verification = None;
                    let token = owner.token.take().expect("pending offer");
                    let result = if accepted {
                        Ok(crate::commands::Action::Reply(json!({})))
                    } else {
                        Err("the image could not be copied to the clipboard".into())
                    };
                    let done = self
                        .runtime
                        .engine
                        .job_complete(token, result, Instant::now());
                    retire = !accepted || !done.replies.iter().any(|(_, v)| v["ok"] == true);
                    effects.replies.extend(done.replies);
                }
            }
        }
        if retire {
            self.clipboard_owner = None;
        }
        effects
    }
    pub fn close(&mut self, timeout: Duration) -> io::Result<Effects> {
        self.runtime.stopping = true;
        self.player = None;
        self.sound = None;
        if let Some(capture) = self.capture.take() {
            capture.discard();
        }
        self.file_openers.clear();
        self.sound_jobs = Default::default();
        self.sound_owners.clear();
        self.clipboard_owner = None;
        self.jobs = crate::jobs::Pool::default();
        self.startup_jobs = crate::jobs::Pool::default();
        self.startup_owners.clear();
        self.startup_queue.clear();
        let mut effects = self.runtime.engine.cancel_all("the connection was closed");
        for account in self
            .runtime
            .engine
            .accounts
            .values()
            .filter(|a| a.client > 0)
        {
            self.td.send(account.client, &json!({"@type":"close"}))?;
        }
        let deadline = Instant::now() + timeout;
        while self.runtime.engine.accounts.values().any(|a| a.client > 0)
            && Instant::now() < deadline
        {
            if let Some(event) = self.td.receive(Duration::from_millis(20))? {
                let next =
                    self.runtime
                        .receive(event, Instant::now(), &mut |_| Ok(None), &mut |_| None);
                // Shutdown never launches startup/business effects.
                effects.replies.extend(next.replies);
            }
        }
        if self.runtime.engine.accounts.values().any(|a| a.client > 0) {
            return Err(io::Error::other("native driver close deadline exceeded"));
        }
        self.td.close(Duration::ZERO)?;
        Ok(effects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an explicitly provided verified TDLib library"]
    fn startup_queue_and_completion_own_account_auth_serial_and_deadline() {
        let library = std::env::var_os("TELEBAR_TEST_TDLIB").expect("verified library");
        let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
        // These synthetic registrations never allocate or send to native clients.
        driver
            .runtime
            .register(
                "default".into(),
                "Synthetic".into(),
                41,
                vec![],
                Instant::now(),
            )
            .unwrap();
        driver.set_startup_context(crate::startup::Context {
            home: "/fixture/not-home".into(),
            environment: Default::default(),
            version: "fixture".into(),
            language: "en".into(),
        });
        driver.start_parameters("default");
        driver.start_parameters("default");
        assert_eq!(driver.startup_queue.len(), 1);
        let token = crate::lifecycle::Token::startup(900);
        let owner = driver.startup_queue.pop_front().unwrap();
        driver.startup_owners.insert(token, owner);
        let done = |value| crate::jobs::Completion {
            token,
            result: Ok(crate::commands::Action::StartupPrepared(
                crate::startup::Prepared(Ok(Some(value))),
            )),
        };
        let effects = driver.complete_startup(
            done(json!({"@type":"setTdlibParameters","fixture":true})),
            Instant::now(),
        );
        assert_eq!(effects.native.len(), 1);
        assert_eq!(effects.native[0].client, 41);
        assert!(driver.runtime.parameters_pending("default"));
        driver.start_parameters("default");
        assert!(driver.startup_queue.is_empty());
        assert!(
            driver
                .complete_startup(done(json!({})), Instant::now())
                .native
                .is_empty()
        );
        driver
            .runtime
            .expire(Instant::now() + Duration::from_secs(181));
        assert!(!driver.runtime.parameters_pending("default"));
        for fault in [
            "auth",
            "serial",
            "generation",
            "client",
            "stopping",
            "deadline",
        ] {
            let old = driver.runtime.metadata["default"].auth.clone();
            driver.runtime.stopping = false;
            driver
                .runtime
                .engine
                .accounts
                .get_mut("default")
                .unwrap()
                .client = 41;
            driver.start_parameters("default");
            let mut owner = driver.startup_queue.pop_front().unwrap();
            match fault {
                "auth" => {
                    driver.runtime.metadata.get_mut("default").unwrap().auth =
                        json!({"state":"ready"})
                }
                "serial" => {
                    driver
                        .runtime
                        .metadata
                        .get_mut("default")
                        .unwrap()
                        .parameters_serial += 1
                }
                "generation" => {
                    driver
                        .runtime
                        .engine
                        .accounts
                        .get_mut("default")
                        .unwrap()
                        .generation += 1
                }
                "client" => {
                    driver
                        .runtime
                        .engine
                        .accounts
                        .get_mut("default")
                        .unwrap()
                        .client = 0
                }
                "stopping" => driver.runtime.stopping = true,
                "deadline" => owner.started = Instant::now() - Duration::from_secs(181),
                _ => unreachable!(),
            }
            driver.startup_owners.insert(token, owner);
            let effects = driver
                .complete_startup(done(json!({"@type":"setTdlibParameters"})), Instant::now());
            assert!(effects.native.is_empty(), "{fault}");
            assert!(driver.startup_owners.is_empty(), "{fault}");
            if fault == "deadline" {
                assert_eq!(
                    driver.runtime.metadata["default"].auth["reason"],
                    "the startup credential preparation deadline was exceeded"
                );
            } else {
                assert!(effects.replies.is_empty(), "{fault}");
            }
            driver.runtime.metadata.get_mut("default").unwrap().auth = old;
        }
        driver.start_parameters("default");
        driver.startup_queue.front_mut().unwrap().started =
            Instant::now() - Duration::from_secs(181);
        let expired = driver.dispatch_startup();
        assert!(!expired.replies.is_empty());
        assert_eq!(driver.startup_jobs.active(), 0);
        assert!(driver.startup_queue.is_empty());
        driver.start_parameters("default");
        driver.runtime.metadata.get_mut("default").unwrap().auth = json!({"state":"ready"});
        assert!(driver.dispatch_startup().replies.is_empty());
        assert!(driver.startup_queue.is_empty());
        driver.runtime.metadata.get_mut("default").unwrap().auth = json!({"state":"starting"});
        for (id, client) in [("second", 42), ("third", 43)] {
            driver
                .runtime
                .register(
                    id.into(),
                    "Synthetic".into(),
                    client,
                    vec![],
                    Instant::now(),
                )
                .unwrap();
        }
        for id in ["default", "second", "third"] {
            driver.start_parameters(id);
        }
        assert_eq!(driver.startup_queue.len(), 3);
        assert!(driver.dispatch_startup().replies.is_empty());
        assert_eq!(driver.startup_jobs.active(), 2);
        assert_eq!(driver.startup_queue.len(), 1);
        assert_eq!(driver.startup_queue[0].account, "third");
        // No fake native identities reach TDLib; close still joins owned work.
        for account in driver.runtime.engine.accounts.values_mut() {
            account.client = 0;
        }
        driver.close(Duration::ZERO).unwrap();
        assert!(driver.startup_queue.is_empty() && driver.startup_owners.is_empty());
        assert_eq!(driver.startup_jobs.active(), 0);
    }
    #[test]
    #[ignore = "requires an explicitly provided verified TDLib library"]
    fn playback_driver_retires_stale_selections_and_owned_helpers() {
        use crate::{
            engine::Account,
            playback::{Selection, Start},
            wire::Request,
        };
        use std::{fs, os::unix::fs::PermissionsExt};
        let library = std::env::var_os("TELEBAR_TEST_TDLIB").expect("verified library");
        let root =
            std::env::temp_dir().join(format!("telebar-driver-playback-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let bytes = crate::sounds::render(&crate::sounds::motif("synthetic", 0), "pop").unwrap();
        let path = root.join("voice.wav");
        fs::write(&path, bytes).unwrap();
        let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
        driver.runtime.engine.accounts.insert(
            "default".into(),
            Account {
                client: 41,
                generation: 1,
                state: crate::state::State::default(),
                ready: true,
            },
        );
        let mut app =
            crate::application::Application::load(&crate::storage::Storage::open(&root).unwrap())
                .unwrap();
        let home = crate::storage::Storage::open(&root).unwrap();
        let make = |id, serial| {
            let request = Request::parse(
                json!({"id":id,"cmd":"media.play","args":{}})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
            let start = Start {
                selection: Selection {
                    serial,
                    chat: 42,
                    message: 7,
                    file: 3,
                    rate: 1.0,
                    environment: std::collections::BTreeMap::from([
                        ("SDL_AUDIODRIVER".into(), "dummy".into()),
                        ("SDL_VIDEODRIVER".into(), "dummy".into()),
                    ]),
                },
                path: path.to_str().unwrap().into(),
                roots: vec![root.to_str().unwrap().into()],
            };
            (request, start)
        };
        driver.playback_serial = 1;
        let (request, start) = make(1, 1);
        let mut start = Some(start);
        let mut e =
            driver
                .runtime
                .engine
                .request_using(7, request, Instant::now(), &mut |_, _, _| {
                    Ok(crate::commands::Action::Playback(start.take().unwrap()))
                });
        let (token, start) = e.playback.pop().unwrap();
        let e = driver.complete_playback(token, start, Instant::now());
        assert_eq!(e.replies[0].1["event"], "playing");
        assert_eq!(e.replies[1].1["ok"], true);
        assert_eq!(e.native[0].query["@type"], "openMessageContent");
        let pid = driver.player.as_ref().unwrap().2.pid();
        let (request, start) = make(2, 1);
        let mut start = Some(start);
        let mut e =
            driver
                .runtime
                .engine
                .request_using(7, request, Instant::now(), &mut |_, _, _| {
                    Ok(crate::commands::Action::Playback(start.take().unwrap()))
                });
        let (token, start) = e.playback.pop().unwrap();
        driver.playback_serial = 2;
        let stale = driver.complete_playback(token, start, Instant::now());
        assert!(stale.native.is_empty());
        assert_eq!(
            stale.replies[0].1["error"],
            "the playback selection changed"
        );
        assert_eq!(driver.player.as_ref().unwrap().2.pid(), pid);
        let stop = Request::parse(br#"{"id":3,"cmd":"media.stop"}"#).unwrap();
        let stopped = driver
            .request_application(&mut app, &home, 7, stop)
            .unwrap();
        assert_eq!(stopped.replies[0].1["event"], "playing");
        assert_eq!(stopped.replies[1].1["ok"], true);
        assert!(driver.player.is_none());
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        let (request, start) = make(4, driver.playback_serial);
        let mut start = Some(start);
        let mut e =
            driver
                .runtime
                .engine
                .request_using(7, request, Instant::now(), &mut |_, _, _| {
                    Ok(crate::commands::Action::Playback(start.take().unwrap()))
                });
        let (token, start) = e.playback.pop().unwrap();
        driver.complete_playback(token, start, Instant::now());
        let pid = driver.player.as_ref().unwrap().2.pid();
        driver
            .runtime
            .engine
            .accounts
            .get_mut("default")
            .unwrap()
            .generation += 1;
        let retired = driver.poll_player();
        assert_eq!(retired.replies[0].1["fileId"], 0);
        assert!(driver.player.is_none());
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        driver
            .runtime
            .engine
            .accounts
            .get_mut("default")
            .unwrap()
            .client = 0;
        driver.close(Duration::ZERO).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "requires the reviewed isolated headless Wayland fixture"]
    fn clipboard_offer_is_verified_by_the_actual_isolated_consumer() {
        use sha2::Digest;
        assert_eq!(
            std::env::var("TELEBAR_ISOLATED_WAYLAND").as_deref(),
            Ok("1")
        );
        let library = std::env::var_os("TELEBAR_TEST_TDLIB").expect("verified library");
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").expect("isolated runtime");
        assert_eq!(Path::new(&runtime).parent(), Some(Path::new("/tmp")));
        assert!(
            Path::new(&runtime)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("telebar-wayland-")
        );
        let environment: std::collections::BTreeMap<String, String> = [
            ("XDG_RUNTIME_DIR".into(), runtime.into_string().unwrap()),
            (
                "WAYLAND_DISPLAY".into(),
                std::env::var("WAYLAND_DISPLAY").unwrap(),
            ),
        ]
        .into_iter()
        .collect();
        let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
        driver.runtime.engine.accounts.insert(
            "default".into(),
            crate::engine::Account {
                client: 41,
                generation: 1,
                state: crate::state::State::default(),
                ready: true,
            },
        );
        use base64::Engine;
        let data=base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=").unwrap();
        let request = Request::parse(
            json!({"id":1,"cmd":"file.copyImage","args":{}})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        let effects =
            driver
                .runtime
                .engine
                .request_using(7, request, Instant::now(), &mut |_, _, _| {
                    Ok(crate::commands::Action::ClipboardOffer(
                        crate::clipboard::Offer {
                            data: data.clone(),
                            environment: environment.clone(),
                        },
                    ))
                });
        let token = effects.clipboard[0].0;
        driver.dispatch(effects).unwrap();
        let pid = driver.clipboard_owner.as_ref().unwrap().helper.pid();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut replies = Vec::new();
        while driver.runtime.engine.job_current(token) && Instant::now() < deadline {
            replies.extend(driver.poll_clipboard().replies);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].1["ok"], true);
        assert!(driver.clipboard_owner.as_ref().unwrap().token.is_none());
        // A second independent consumer reads after acknowledgement and IPC
        // disconnect. The foreground owner must continue serving exact bytes.
        driver.runtime.engine.retire_peer(7);
        driver.poll_clipboard();
        let pasted = crate::process::run(
            "/usr/bin/wl-paste",
            &["--no-newline", "--type", "image/png"],
            &environment,
            &[],
            Duration::from_secs(5),
            10 * 1024 * 1024,
        )
        .unwrap();
        assert!(pasted.ok());
        assert_eq!(pasted.stdout, data);
        let request = Request::parse(
            json!({"id":4,"cmd":"file.copyImage","args":{}})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        let effects =
            driver
                .runtime
                .engine
                .request_using(8, request, Instant::now(), &mut |_, _, _| {
                    Ok(crate::commands::Action::ClipboardOffer(
                        crate::clipboard::Offer {
                            data: data.clone(),
                            environment: environment.clone(),
                        },
                    ))
                });
        driver.dispatch(effects).unwrap();
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "replacement reaps the exact previous selection owner"
        );
        let pid = driver.clipboard_owner.as_ref().unwrap().helper.pid();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut accepted = false;
        while driver
            .clipboard_owner
            .as_ref()
            .is_some_and(|o| o.token.is_some())
            && Instant::now() < deadline
        {
            accepted |= driver
                .poll_clipboard()
                .replies
                .iter()
                .any(|(_, v)| v["id"] == 4 && v["ok"] == true);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(accepted);
        // An explicitly mismatching read cannot produce success. Inject only a
        // finite inert reader; it does not touch another clipboard selection.
        let request = Request::parse(
            json!({"id":2,"cmd":"file.copyImage","args":{}})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        let mut effects =
            driver
                .runtime
                .engine
                .request_using(8, request, Instant::now(), &mut |_, _, _| {
                    Ok(crate::commands::Action::ClipboardOffer(
                        crate::clipboard::Offer {
                            data: data.clone(),
                            environment: environment.clone(),
                        },
                    ))
                });
        let token = effects.clipboard.pop().unwrap().0;
        let owner = driver.clipboard_owner.as_mut().unwrap();
        owner.token = Some(token);
        owner.digest = sha2::Sha256::digest(&data).into();
        owner.started = Instant::now();
        owner.verification = Some(
            crate::process::Managed::start(
                "/usr/bin/cat",
                &[],
                &Default::default(),
                b"different",
                Duration::from_secs(2),
                64,
            )
            .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut rejected = false;
        while driver.clipboard_owner.is_some() && Instant::now() < deadline {
            rejected |= driver
                .poll_clipboard()
                .replies
                .iter()
                .any(|(_, v)| v["id"] == 2 && v["ok"] == false);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(rejected);
        assert!(driver.clipboard_owner.is_none());
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        let request = Request::parse(
            json!({"id":3,"cmd":"file.copyImage","args":{}})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        let effects =
            driver
                .runtime
                .engine
                .request_using(9, request, Instant::now(), &mut |_, _, _| {
                    Ok(crate::commands::Action::ClipboardOffer(
                        crate::clipboard::Offer {
                            data: data.clone(),
                            environment: environment.clone(),
                        },
                    ))
                });
        driver.dispatch(effects).unwrap();
        let owner_pid = driver.clipboard_owner.as_ref().unwrap().helper.pid();
        // Shutdown also owns any still-pending selection verification helper.
        driver.clipboard_owner.as_mut().unwrap().verification = Some(
            crate::process::Managed::start(
                "/usr/bin/sleep",
                &["10"],
                &Default::default(),
                &[],
                Duration::from_secs(12),
                64,
            )
            .unwrap(),
        );
        let reader_pid = driver
            .clipboard_owner
            .as_ref()
            .unwrap()
            .verification
            .as_ref()
            .unwrap()
            .pid();
        driver
            .runtime
            .engine
            .accounts
            .get_mut("default")
            .unwrap()
            .client = 0;
        let closed = driver.close(Duration::ZERO).unwrap();
        assert!(
            closed
                .replies
                .iter()
                .any(|(_, v)| v["id"] == 3 && v["ok"] == false)
        );
        assert!(!Path::new(&format!("/proc/{owner_pid}")).exists());
        assert!(!Path::new(&format!("/proc/{reader_pid}")).exists());
    }
    #[test]
    #[ignore = "requires an explicitly provided verified TDLib library"]
    fn sound_preview_workers_are_owned_without_a_real_audio_session() {
        let library = std::env::var_os("TELEBAR_TEST_TDLIB").expect("verified library");
        let root =
            std::env::temp_dir().join(format!("telebar-driver-sound-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let home = crate::storage::Storage::open(&root).unwrap();
        let mut app = crate::application::Application::load(&home).unwrap();
        app.upload_context = Some((root.clone(), vec![]));
        app.clipboard_context = Some(crate::clipboard::Context {
            runtime: root.clone(),
            databases: vec![],
            environment: Default::default(),
        });
        let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
        driver.runtime.engine.accounts.insert(
            "default".into(),
            crate::engine::Account {
                client: 41,
                generation: 1,
                state: Default::default(),
                ready: true,
            },
        );
        driver.runtime.metadata.insert(
            "default".into(),
            crate::authorization::Metadata::new("Synthetic".into()),
        );
        driver
            .runtime
            .engine
            .accounts
            .get_mut("default")
            .unwrap()
            .state
            .me_id = 7;
        let request = crate::wire::Request::parse(
            br#"{"id":1,"cmd":"settings.sounds","args":{"style":"pop"}}"#,
        )
        .unwrap();
        let done = driver
            .request_application(&mut app, &home, 7, request)
            .unwrap();
        assert_eq!(done.replies[0].1["ok"], true);
        assert_eq!(driver.sound_jobs.active(), 1);
        assert_eq!(driver.sound_owners.len(), 1);
        assert!(app.settings.sounds_pending.is_empty());
        // Retire the captured generation before polling completed rendering: no
        // pw-play/native account dispatch must follow a stale background job.
        driver
            .runtime
            .engine
            .accounts
            .get_mut("default")
            .unwrap()
            .generation += 1;
        let until = Instant::now() + Duration::from_secs(2);
        while driver.sound_jobs.active() > 0 && Instant::now() < until {
            driver.tick(&mut |_| Ok(None)).unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(driver.sound_jobs.active(), 0);
        assert!(driver.sound_owners.is_empty());
        assert!(driver.sound.is_none());
        app.settings.sounds_pending = vec![7];
        driver.start_pending_sounds(&mut app);
        assert_eq!(driver.sound_jobs.active(), 1);
        driver.capture_context = Some(crate::capture::Context {
            home: root.clone(),
            runtime: root.clone(),
            environment: Default::default(),
        });
        let synthetic = || {
            ["-re", "-f", "lavfi", "-i", "sine=frequency=400:duration=3"]
                .map(str::to_owned)
                .to_vec()
        };
        let capture = crate::capture::Capture::start_with(
            driver.capture_context.as_ref().unwrap(),
            "default".into(),
            2,
            "voice",
            42,
            Some(synthetic()),
        )
        .unwrap();
        let pid = capture.pid();
        let path = capture.path.clone();
        driver.capture = Some(capture);
        let request = crate::wire::Request::parse(
            br#"{"id":9,"cmd":"voice.stop","args":{"send":false},"account":"other"}"#,
        )
        .unwrap();
        assert_eq!(
            driver
                .request_application(&mut app, &home, 7, request)
                .unwrap()
                .replies[0]
                .1["error"],
            "unknown account"
        );
        assert!(driver.capture.is_some());
        let request = crate::wire::Request::parse(
            br#"{"id":10,"cmd":"voice.stop","args":{"send":true,"replyToMessageId":false}}"#,
        )
        .unwrap();
        let result = driver
            .request_application(&mut app, &home, 7, request)
            .unwrap();
        assert_eq!(result.replies[0].1["ok"], false);
        assert!(driver.capture.is_some());
        let request =
            crate::wire::Request::parse(br#"{"id":11,"cmd":"voice.stop","args":{"send":false}}"#)
                .unwrap();
        let result = driver
            .request_application(&mut app, &home, 7, request)
            .unwrap();
        assert!(
            result
                .replies
                .iter()
                .any(|(_, e)| e["event"] == "recording" && e["state"] == "idle")
        );
        assert!(driver.capture.is_none());
        let until = Instant::now() + Duration::from_secs(2);
        while driver.jobs.active() > 0 && Instant::now() < until {
            driver.tick(&mut |_| Ok(None)).unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!path.exists());
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        let capture = crate::capture::Capture::start_with(
            driver.capture_context.as_ref().unwrap(),
            "default".into(),
            2,
            "voice",
            42,
            Some(synthetic()),
        )
        .unwrap();
        let path = capture.path.clone();
        driver.capture = Some(capture);
        driver
            .runtime
            .engine
            .accounts
            .get_mut("default")
            .unwrap()
            .generation = 3;
        driver.tick(&mut |_| Ok(None)).unwrap();
        assert!(driver.capture.is_none());
        assert!(!path.exists());
        driver
            .runtime
            .engine
            .accounts
            .get_mut("default")
            .unwrap()
            .client = 0;
        driver.close(Duration::ZERO).unwrap();
        assert!(driver.sound_owners.is_empty());
        assert_eq!(driver.sound_jobs.active(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "requires an explicitly provided verified TDLib library"]
    fn clipboard_owner_retires_without_touching_the_session_clipboard() {
        let library = std::env::var_os("TELEBAR_TEST_TDLIB").expect("verified library");
        let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
        driver.runtime.engine.accounts.insert(
            "default".into(),
            crate::engine::Account {
                client: 41,
                generation: 1,
                state: crate::state::State::default(),
                ready: true,
            },
        );
        let admit = |engine: &mut crate::engine::Engine, id| {
            let request = Request::parse(
                json!({"id":id,"cmd":"file.copyImage","args":{}})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
            let mut effects = engine.request_using(7, request, Instant::now(), &mut |_, _, _| {
                Ok(crate::commands::Action::ClipboardOffer(
                    crate::clipboard::Offer {
                        data: vec![],
                        environment: Default::default(),
                    },
                ))
            });
            effects.clipboard.pop().unwrap().0
        };
        let token = admit(&mut driver.runtime.engine, 1);
        // Cat consumes input then exits: even successful helper exit is not a
        // persistent clipboard offer. No wl-copy or real Wayland is contacted.
        let helper = crate::process::Managed::start(
            "/usr/bin/cat",
            &[],
            &Default::default(),
            b"synthetic",
            Duration::from_secs(2),
            64,
        )
        .unwrap();
        let pid = helper.pid();
        driver.clipboard_owner = Some(ClipboardOwner {
            token: Some(token),
            helper,
            started: Instant::now(),
            verification: None,
            environment: Default::default(),
            digest: [0; 32],
        });
        let until = Instant::now() + Duration::from_secs(2);
        let mut failure = false;
        while Instant::now() < until {
            let e = driver.poll_clipboard();
            failure |= e
                .replies
                .iter()
                .any(|(_, v)| v["id"] == 1 && v["ok"] == false);
            if driver.clipboard_owner.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(failure);
        assert!(driver.clipboard_owner.is_none());
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        let token = admit(&mut driver.runtime.engine, 2);
        let helper = crate::process::Managed::start(
            "/usr/bin/sleep",
            &["10"],
            &Default::default(),
            &[],
            Duration::from_secs(12),
            64,
        )
        .unwrap();
        let pid = helper.pid();
        driver.clipboard_owner = Some(ClipboardOwner {
            token: Some(token),
            helper,
            started: Instant::now(),
            verification: None,
            environment: Default::default(),
            digest: [0; 32],
        });
        driver.runtime.engine.retire_peer(7);
        assert!(driver.poll_clipboard().replies.is_empty());
        assert!(driver.clipboard_owner.is_none());
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        let token = admit(&mut driver.runtime.engine, 3);
        let helper = crate::process::Managed::start(
            "/usr/bin/sleep",
            &["10"],
            &Default::default(),
            &[],
            Duration::from_secs(12),
            64,
        )
        .unwrap();
        let pid = helper.pid();
        driver.clipboard_owner = Some(ClipboardOwner {
            token: Some(token),
            helper,
            started: Instant::now(),
            verification: None,
            environment: Default::default(),
            digest: [0; 32],
        });
        driver
            .runtime
            .engine
            .accounts
            .get_mut("default")
            .unwrap()
            .generation += 1;
        let retired = driver.poll_clipboard();
        assert!(
            retired.replies.is_empty(),
            "stale generation cannot deliver a completion"
        );
        assert!(driver.clipboard_owner.is_none());
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        let token = admit(&mut driver.runtime.engine, 4);
        driver
            .runtime
            .engine
            .expire(Instant::now() + Duration::from_secs(181));
        let mut late = Effects::default();
        late.clipboard.push((
            token,
            crate::clipboard::Offer {
                data: vec![],
                environment: Default::default(),
            },
        ));
        assert!(driver.dispatch(late).unwrap().replies.is_empty());
        assert!(driver.clipboard_owner.is_none());
        driver
            .runtime
            .engine
            .accounts
            .get_mut("default")
            .unwrap()
            .client = 0;
        driver.close(Duration::ZERO).unwrap();
    }
}
