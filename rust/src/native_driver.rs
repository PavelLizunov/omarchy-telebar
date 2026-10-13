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
struct Native(Option<Td>);
impl Native {
    fn create_client(&mut self) -> io::Result<i32> {
        self.0
            .as_mut()
            .ok_or_else(|| io::Error::other("Telegram is not available"))?
            .create_client()
    }
    fn send(&mut self, client: i32, query: &Value) -> io::Result<()> {
        self.0
            .as_mut()
            .ok_or_else(|| io::Error::other("Telegram is not available"))?
            .send(client, query)
    }
    fn execute(&mut self, query: &Value) -> io::Result<Option<Value>> {
        self.0.as_mut().map_or(Ok(None), |td| td.execute(query))
    }
    fn receive(&mut self, timeout: Duration) -> io::Result<Option<Value>> {
        self.0.as_mut().map_or(Ok(None), |td| td.receive(timeout))
    }
    fn close(&mut self, timeout: Duration) -> io::Result<()> {
        self.0.as_mut().map_or(Ok(()), |td| td.close(timeout))
    }
}
pub struct Driver {
    pub runtime: Runtime,
    td: Native,
    language: String,
    jobs: crate::jobs::Pool,
    global_jobs: crate::jobs::Pool,
    global_owner: Option<(crate::lifecycle::Token, Option<crate::lifecycle::Token>)>,
    global_queue: std::collections::VecDeque<(Option<crate::lifecycle::Token>, crate::jobs::Work)>,
    global_done: Vec<(
        Option<crate::lifecycle::Token>,
        crate::commands::Result<crate::commands::Action>,
    )>,
    global_serial: u64,
    bridge_owners:
        std::collections::HashMap<crate::lifecycle::Token, (String, crate::bridge::Operation)>,
    bridge_done: Vec<crate::jobs::Completion>,
    bridge_operations: Vec<(crate::lifecycle::Token, crate::bridge::Operation)>,
    bridge_mutations: Vec<(crate::lifecycle::Token, crate::bridge::Mutation)>,
    bridge_restore: std::collections::VecDeque<String>,
    bridge_restore_serial: u64,
    desktop_helpers: Vec<crate::desktop::Pending>,
    last_receive: Option<Instant>,
    received_events: u64,
    startup_jobs: crate::jobs::Pool,
    startup_context: Option<crate::startup::Context>,
    startup_owners: std::collections::HashMap<crate::lifecycle::Token, StartupOwner>,
    startup_serial: u64,
    startup_queue: std::collections::VecDeque<StartupOwner>,
    sound: Option<(crate::process::Managed, crate::sound_playback::Prepared)>,
    pub notification_transport: Option<crate::notification_bus::Transport>,
    notification_updates: Vec<(String, Value)>,
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
        Ok(Self::with_native(
            Some(Td::open(library)?),
            active,
            language,
        ))
    }
    pub fn without_native(active: String, language: String) -> Self {
        Self::with_native(None, active, language)
    }
    pub fn open_optional(library: &Path, active: String, language: String) -> Self {
        Self::with_native(Td::open(library).ok(), active, language)
    }
    fn with_native(td: Option<Td>, active: String, language: String) -> Self {
        Self {
            runtime: Runtime::new(active),
            td: Native(td),
            language,
            jobs: crate::jobs::Pool::default(),
            global_jobs: Default::default(),
            global_owner: None,
            global_queue: Default::default(),
            global_done: Vec::new(),
            global_serial: 1,
            bridge_owners: Default::default(),
            bridge_done: Vec::new(),
            bridge_operations: Vec::new(),
            bridge_mutations: Vec::new(),
            bridge_restore: Default::default(),
            bridge_restore_serial: 1,
            desktop_helpers: Vec::new(),
            last_receive: None,
            received_events: 0,
            startup_jobs: crate::jobs::Pool::default(),
            startup_context: None,
            startup_owners: std::collections::HashMap::new(),
            startup_serial: 1,
            startup_queue: std::collections::VecDeque::new(),
            sound: None,
            notification_transport: None,
            notification_updates: vec![],
            capture_context: None,
            capture: None,
            file_openers: vec![],
            sound_jobs: Default::default(),
            sound_owners: Default::default(),
            clipboard_owner: None,
            playback_serial: 0,
            player: None,
        }
    }
    fn worker_count(&self) -> usize {
        self.jobs.active()
            + self.startup_jobs.active()
            + self.sound_jobs.active()
            + self.global_jobs.active()
    }
    fn global_current(runtime: &Runtime, token: crate::lifecycle::Token) -> bool {
        !runtime.stopping
            && runtime
                .engine
                .job_owner(token)
                .is_some_and(|(id, generation)| {
                    runtime
                        .engine
                        .accounts
                        .get(&id)
                        .is_some_and(|a| a.generation == generation)
                })
    }
    fn native_current(runtime: &Runtime, token: crate::lifecycle::Token) -> bool {
        Self::global_current(runtime, token)
            && runtime.engine.job_owner(token).is_some_and(|(id, _)| {
                runtime
                    .engine
                    .accounts
                    .get(&id)
                    .is_some_and(|a| a.client > 0)
            })
    }
    /// Startup reapplication has no IPC peer, but uses the same single owner.
    pub fn apply_initial_globals(
        &mut self,
        app: &mut crate::application::Application,
    ) -> crate::commands::Result<()> {
        if self.global_owner.is_some()
            || !self.global_queue.is_empty()
            || app.settings.globals_pending
        {
            return Err("shortcut application already pending".into());
        }
        let crate::commands::Action::Work(work) = app.settings.globals_work()? else {
            unreachable!()
        };
        self.global_queue.push_back((None, work));
        Ok(())
    }
    fn pump_globals(
        &mut self,
        app: &mut crate::application::Application,
        home: &crate::storage::Storage,
        effects: &mut Effects,
    ) {
        // Queue time is part of IPC admission; expire before any persisted
        // mutation or external helper, not only at final acknowledgement.
        effects
            .replies
            .extend(self.runtime.expire(Instant::now()).replies);
        for (request, result) in std::mem::take(&mut self.global_done) {
            // Completed settings are committed even if the submitting peer has
            // gone. The disconnected peer must not get a reused-ID response.
            let event = match &result {
                Ok(crate::commands::Action::Reply(view)) => app.settings.complete_globals(view),
                _ => Some(app.settings.fail_globals()),
            };
            if let Some(token) = request {
                effects.replies.extend(
                    self.runtime
                        .engine
                        .job_complete(token, result, Instant::now())
                        .replies,
                );
            }
            if let Some(event) = event {
                effects.replies.push((0, event));
            }
        }
        if self.runtime.stopping || self.global_owner.is_some() {
            return;
        }
        for _ in 0..64 {
            let Some((request, work)) = self.global_queue.pop_front() else {
                break;
            };
            if request.is_some_and(|token| !Self::global_current(&self.runtime, token)) {
                // A persisted mutation still needs a terminal status for its
                // observers. A deferred command has not saved anything yet.
                if matches!(&work, crate::jobs::Work::GlobalShortcuts(_)) {
                    effects.replies.push((0, app.settings.fail_globals()));
                }
                continue;
            }
            match work {
                crate::jobs::Work::DeferredSettings { command, args } => {
                    let token = request.expect("deferred IPC owner");
                    let action = self
                        .runtime
                        .engine
                        .job_owner(token)
                        .and_then(|(id, _)| self.runtime.engine.accounts.get(&id))
                        .map(|account| {
                            app.settings
                                .command(home, &command, &args, &account.state)
                                .unwrap_or_else(|| Err("unknown command".into()))
                        });
                    if let Some(action) = action {
                        let mut next =
                            self.runtime
                                .engine
                                .job_complete(token, action, Instant::now());
                        // The queue reserves the position of this mutation. A
                        // new global job must precede later deferred requests.
                        for (token, work) in next.work.drain(..).rev() {
                            self.global_queue.push_front((Some(token), work));
                        }
                        effects.replies.extend(next.replies);
                        effects.replies.extend(
                            std::mem::take(&mut app.settings.events)
                                .into_iter()
                                .map(|event| (0, event)),
                        );
                    }
                }
                work @ crate::jobs::Work::GlobalShortcuts(_) => {
                    if self.worker_count() >= 2 {
                        self.global_queue.push_front((request, work));
                        break;
                    }
                    let Some(next) = self.global_serial.checked_add(1) else {
                        self.global_done
                            .push((request, Err("shortcut worker identity exhausted".into())));
                        break;
                    };
                    let worker = crate::lifecycle::Token::startup(self.global_serial);
                    self.global_serial = next;
                    match self.global_jobs.start(worker, work) {
                        Ok(()) => self.global_owner = Some((worker, request)),
                        Err(error) => self.global_done.push((request, Err(error))),
                    }
                    break;
                }
                _ => unreachable!("only settings work enters the serial queue"),
            }
        }
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
        while self.worker_count() < 2 {
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
        if self.td.0.is_none() {
            if !crate::paths::valid_account_id(&id)
                || self.runtime.engine.accounts.contains_key(&id)
            {
                return Err(io::Error::other("invalid account client registration"));
            }
            self.runtime.engine.accounts.insert(
                id.clone(),
                crate::engine::Account {
                    client: 0,
                    generation: 1,
                    state: crate::state::State::new(roots),
                    ready: false,
                },
            );
            let mut meta = crate::authorization::Metadata::new(name);
            meta.auth = json!({"state":"noLibrary","reason":"TDLib is not installed or could not be loaded"});
            self.runtime.metadata.insert(id, meta);
            return Ok(Effects::default());
        }
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
        self.bridge_operations
            .append(&mut effects.bridge_operations);
        self.bridge_mutations.append(&mut effects.bridge_mutations);
        for (token, work) in std::mem::take(&mut effects.work) {
            if matches!(
                &work,
                crate::jobs::Work::GlobalShortcuts(_) | crate::jobs::Work::DeferredSettings { .. }
            ) {
                if self.global_queue.len() < 64 {
                    self.global_queue.push_back((Some(token), work));
                } else if matches!(&work, crate::jobs::Work::GlobalShortcuts(_)) {
                    self.global_done.push((
                        Some(token),
                        Err("too many pending settings requests".into()),
                    ));
                } else {
                    effects.replies.extend(
                        self.runtime
                            .engine
                            .job_complete(
                                token,
                                Err("too many pending settings requests".into()),
                                Instant::now(),
                            )
                            .replies,
                    );
                }
                continue;
            }
            let started = if self.worker_count() >= 2 {
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
            if self
                .runtime
                .engine
                .native_dispatched(&native.query)
                .is_err()
            {
                // No native request has been dispatched. A failed durable input
                // journal must not submit a file or terminate unrelated accounts.
                let refused = json!({"@type":"error","@client_id":native.client,"@extra":native.query["@extra"],"code":400,"message":"the generated input could not be retained; no message was submitted"});
                effects
                    .replies
                    .extend(self.runtime.engine.receive(refused).replies);
                continue;
            }
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
        if matches!(
            request.command.as_str(),
            "bridge.status" | "bridge.test" | "bridge.set"
        ) {
            let account = self
                .runtime
                .engine
                .routed_account(&request)
                .unwrap_or_default();
            let status = application.bridge.status(&account);
            let busy = if request.command == "bridge.test" {
                !application.bridge.busy.is_empty()
            } else {
                application.bridge.busy.contains(&account)
            };
            let configured = application.bridge.context.is_some();
            let effects = self.runtime.engine.request_using(
                peer,
                request,
                Instant::now(),
                &mut |command, args, _| {
                    if command == "bridge.status" {
                        return Ok(crate::commands::Action::Reply(status.clone()));
                    }
                    if command == "bridge.set" && !args["enabled"].is_boolean() {
                        return Err("enabled must be true or false".into());
                    }
                    if !configured {
                        return Err("the bundled bridge is not available".into());
                    }
                    if busy {
                        return Err("a bridge operation is already running".into());
                    }
                    Ok(crate::commands::Action::BridgeOperation(
                        if command == "bridge.test" {
                            crate::bridge::Operation::Test
                        } else if args["enabled"] == true {
                            crate::bridge::Operation::Enable
                        } else {
                            crate::bridge::Operation::Disable
                        },
                    ))
                },
            );
            let mut effects = self.dispatch(effects)?;
            self.pump_bridge(application, home, &mut effects)?;
            return Ok(effects);
        }
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
            let capacity = self.worker_count() < 2;
            let capture = &mut self.capture;
            let mut transition = None;
            let aid = self.runtime.engine.routed_account(&request);
            let generation = aid
                .as_ref()
                .and_then(|id| self.runtime.engine.accounts.get(id))
                .map(|a| a.generation);
            let wall = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            let mut effects=self.runtime.engine.request_using(peer,request,Instant::now(),&mut|_,args,state|{
                let context=context.as_ref().ok_or("explicit recorder context missing")?;let aid=aid.clone().ok_or("unknown account")?;let generation=generation.ok_or("unknown account")?;
                if starting{
                    if state.retained.record.is_some() || state.retained.recovery_blocked(){return Err("a recorded message is still pending; resolve or discard it first".into());}
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
                    if sending&&(state.retained.record.is_some() || state.retained.recovery_blocked()){return Err("a recorded message is still pending; resolve or discard it first".into());}
                    let options=if sending{crate::commands::reply(args)?;crate::commands::topic(args)?;crate::sending::options(args,state,wall)?}else{Value::Null};
                    let selected=capture.take().expect("admitted capture");
                    transition=Some(json!({"event":"recording","state":"idle","account":aid}));
                    if sending{selected.prepare_send(args.clone(),context.home.clone(),options)}else{selected.stop(args.clone(),context.home.clone(),options)}
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
        let mut effects = self.dispatch(effects)?;
        self.pump_globals(application, home, &mut effects);
        self.start_pending_sounds(application);
        Ok(effects)
    }
    /// One restoration attempt per stored account after startup parameters.
    /// Failure never loops/retries a native mutation automatically.
    pub fn restore_saved_bridge(&mut self, app: &crate::application::Application) {
        self.bridge_restore = app
            .bridge
            .metadata
            .as_object()
            .into_iter()
            .flat_map(|m| m.keys().cloned())
            .take(10)
            .collect();
    }
    fn bridge_action(
        &self,
        app: &crate::application::Application,
        account: &str,
        operation: crate::bridge::Operation,
    ) -> crate::commands::Result<crate::commands::Action> {
        use crate::{
            bridge::{Answer, Operation},
            commands::{Action, Answer as NativeAnswer},
        };
        let entry = app
            .bridge
            .metadata
            .get(account)
            .cloned()
            .unwrap_or(Value::Null);
        Ok(match operation {
            Operation::Test => Action::Native {
                query: json!({"@type":"pingProxy","proxy":app.bridge.running.as_ref().ok_or("could not start the bundled bridge")?.proxy()}),
                answer: NativeAnswer::Seconds,
            },
            Operation::Enable => Action::Native {
                query: json!({"@type":"getProxies"}),
                answer: NativeAnswer::Bridge(Answer::EnableList {
                    proxy: app
                        .bridge
                        .running
                        .as_ref()
                        .ok_or("could not start the bundled bridge")?
                        .proxy(),
                    entry,
                }),
            },
            Operation::Disable if entry.is_object() => Action::Native {
                query: json!({"@type":"getProxies"}),
                answer: NativeAnswer::Bridge(Answer::DisableList { entry }),
            },
            Operation::Disable => Action::Reply(app.bridge.status(account)),
        })
    }
    fn pump_bridge(
        &mut self,
        app: &mut crate::application::Application,
        home: &crate::storage::Storage,
        effects: &mut Effects,
    ) -> io::Result<()> {
        self.pump_bridge_at(app, home, effects, Instant::now())
    }
    fn pump_bridge_at(
        &mut self,
        app: &mut crate::application::Application,
        home: &crate::storage::Storage,
        effects: &mut Effects,
        now: Instant,
    ) -> io::Result<()> {
        effects.replies.extend(self.runtime.expire(now).replies);
        app.bridge.poll_at(now);
        if app.bridge.restore_after_loss(now) {
            self.restore_saved_bridge(app);
        }
        for done in std::mem::take(&mut self.bridge_done) {
            let current = Self::native_current(&self.runtime, done.token);
            let result = match done.result {
                Ok(crate::commands::Action::BridgeReady(running)) if current => {
                    app.bridge.running = Some(running);
                    let (account, operation) = &self.bridge_owners[&done.token];
                    self.bridge_action(app, account, *operation)
                }
                Err(error) => Err(error),
                _ => Err("the connection was closed".into()),
            };
            let next = self
                .runtime
                .engine
                .job_complete(done.token, result, Instant::now());
            let next = self.dispatch(next)?;
            effects.replies.extend(next.replies);
        }
        for (token, operation) in std::mem::take(&mut self.bridge_operations) {
            let Some((account, _)) = self
                .runtime
                .engine
                .job_owner(token)
                .filter(|_| Self::native_current(&self.runtime, token))
            else {
                effects.replies.extend(
                    self.runtime
                        .engine
                        .job_complete(
                            token,
                            Err("Telegram is not available".into()),
                            Instant::now(),
                        )
                        .replies,
                );
                continue;
            };
            let admitted = if app.bridge.busy.contains(&account)
                || (app.bridge.running.is_none() && !app.bridge.busy.is_empty())
            {
                Err("a bridge operation is already running".into())
            } else if !matches!(operation, crate::bridge::Operation::Disable)
                && app.bridge.running.is_none()
                && self.worker_count() >= 2
            {
                Err("wait for the current background operation".into())
            } else {
                Ok(())
            };
            if let Err(error) = admitted {
                effects.replies.extend(
                    self.runtime
                        .engine
                        .job_complete(token, Err(error), Instant::now())
                        .replies,
                );
                continue;
            }
            app.bridge.busy.insert(account.clone());
            self.bridge_owners
                .insert(token, (account.clone(), operation));
            app.bridge.idle_at = Some(Instant::now() + Duration::from_secs(180));
            if !matches!(operation, crate::bridge::Operation::Disable)
                && app.bridge.running.is_none()
            {
                let started = app
                    .bridge
                    .context
                    .clone()
                    .ok_or("the bundled bridge is not available".to_owned())
                    .and_then(|context| {
                        self.jobs
                            .start(token, crate::jobs::Work::StartBridge(context))
                    });
                if let Err(error) = started {
                    effects.replies.extend(
                        self.runtime
                            .engine
                            .job_complete(token, Err(error), Instant::now())
                            .replies,
                    );
                }
            } else {
                let action = self.bridge_action(app, &account, operation);
                let next = self
                    .runtime
                    .engine
                    .job_complete(token, action, Instant::now());
                let next = self.dispatch(next)?;
                effects.replies.extend(next.replies);
            }
        }
        for (token, mutation) in std::mem::take(&mut self.bridge_mutations) {
            let Some((account, _)) = self
                .bridge_owners
                .get(&token)
                .filter(|_| Self::native_current(&self.runtime, token))
            else {
                continue;
            };
            let mut metadata = app.bridge.metadata.clone();
            let disabling = matches!(&mutation, crate::bridge::Mutation::Disable);
            match mutation {
                crate::bridge::Mutation::Enable(entry) => metadata[account] = entry,
                crate::bridge::Mutation::Disable => {
                    metadata
                        .as_object_mut()
                        .expect("metadata object")
                        .remove(account);
                }
            }
            let saved = home
                .write(
                    ".config/omagram/bridge.json",
                    metadata.to_string().as_bytes(),
                    0o600,
                )
                .map_err(|_| "could not save the bundled bridge".to_owned());
            if saved.is_ok() {
                app.bridge.metadata = metadata;
                if disabling && app.bridge.metadata.as_object().is_none_or(|m| m.is_empty()) {
                    app.bridge.running = None;
                }
            }
            let result = saved.map(|_| crate::commands::Action::Reply(app.bridge.status(account)));
            effects.replies.extend(
                self.runtime
                    .engine
                    .job_complete(token, result, Instant::now())
                    .replies,
            );
        }
        let retired: Vec<_> = self
            .bridge_owners
            .keys()
            .filter(|token| !Self::native_current(&self.runtime, **token))
            .copied()
            .collect();
        for token in retired {
            if let Some((account, _)) = self.bridge_owners.remove(&token) {
                app.bridge.busy.remove(&account);
            }
        }
        // An unready first account must not starve a ready later profile.
        self.bridge_restore.retain(|id| {
            self.runtime.engine.accounts.contains_key(id) && app.bridge.metadata.get(id).is_some()
        });
        if self.worker_count() < 2
            && app.bridge.busy.is_empty()
            && let Some(index) = self.bridge_restore.iter().position(|id| {
                self.runtime
                    .metadata
                    .get(id)
                    .is_some_and(|meta| meta.bridge_ready)
            })
        {
            self.bridge_restore.rotate_left(index);
        }
        if let Some(account) = self.bridge_restore.front().cloned() {
            if !self.runtime.engine.accounts.contains_key(&account)
                || app.bridge.metadata.get(&account).is_none()
            {
                self.bridge_restore.pop_front();
            } else if self.worker_count() < 2
                && app.bridge.busy.is_empty()
                && self
                    .runtime
                    .metadata
                    .get(&account)
                    .is_some_and(|meta| meta.bridge_ready)
            {
                self.bridge_restore.pop_front();
                let serial = self.bridge_restore_serial;
                self.bridge_restore_serial = serial.saturating_add(1);
                let request = Request::parse(json!({"id":serial,"cmd":"bridge.set","account":account,"args":{"enabled":true}}).to_string().as_bytes()).map_err(|_| io::Error::other("invalid internal bridge request"))?;
                let next = self.runtime.engine.request_using(
                    u64::MAX,
                    request,
                    Instant::now(),
                    &mut |_, _, _| {
                        Ok(crate::commands::Action::BridgeOperation(
                            crate::bridge::Operation::Enable,
                        ))
                    },
                );
                let next = self.dispatch(next)?;
                effects.replies.extend(next.replies);
            }
        }
        effects.replies.retain(|(peer, _)| *peer != u64::MAX);
        Ok(())
    }
    fn start_pending_sounds(&mut self, application: &mut crate::application::Application) {
        let account = self.runtime.engine.active.clone();
        for seed in std::mem::take(&mut application.settings.sounds_pending) {
            self.start_sound(application, &account, seed);
        }
    }
    fn start_sound(&mut self, application: &crate::application::Application, id: &str, seed: i128) {
        if self.runtime.stopping {
            return;
        }
        let Some((home, _)) = &application.upload_context else {
            return;
        };
        let Some(context) = &application.clipboard_context else {
            return;
        };
        let Some(job) = crate::sound_playback::Job::prepare(
            home.clone(),
            seed,
            &application.settings.value,
            context.environment.clone(),
        ) else {
            return;
        };
        if self.worker_count() >= 2 {
            return;
        }
        let Some(account) = self
            .runtime
            .engine
            .accounts
            .get(id)
            .filter(|a| a.client > 0)
        else {
            return;
        };
        let Some(next) = self.startup_serial.checked_add(1) else {
            return;
        };
        let token = crate::lifecycle::Token::startup(self.startup_serial);
        self.startup_serial = next;
        if self
            .sound_jobs
            .start(token, crate::jobs::Work::Sound(job))
            .is_ok()
        {
            self.sound_owners
                .insert(token, (id.into(), account.generation));
        }
    }
    /// Ordered native update observation, bounded until application polling.
    pub fn observe_notification(&mut self, event: &Value) {
        if event["@type"] == "updateNotificationGroup"
            && self.notification_updates.len() < 128
            && let Some(id) = self
                .runtime
                .engine
                .accounts
                .iter()
                .find(|(_, a)| event["@client_id"].as_i64() == Some(i64::from(a.client)))
                .map(|(id, _)| id.clone())
        {
            self.notification_updates.push((id, event.clone()));
        }
    }
    pub fn pump_notifications(
        &mut self,
        app: &mut crate::application::Application,
        home: &crate::storage::Storage,
        effects: &mut Effects,
    ) {
        let Some(transport) = &self.notification_transport else {
            self.notification_updates.clear();
            return;
        };
        let stale: Vec<_> = app
            .notification_policy
            .shown_generations
            .iter()
            .filter(|(key, g)| {
                !self
                    .runtime
                    .engine
                    .accounts
                    .get(&key.account)
                    .is_some_and(|a| a.client > 0 && a.generation == **g)
            })
            .map(|(key, _)| key.clone())
            .collect();
        for key in stale {
            app.notification_policy.shown_generations.remove(&key);
            app.notification_policy.messages.remove(&key);
            if let Some(id) = app.notifier.withdraw(&key) {
                transport.close(id);
            }
        }
        for id in std::mem::take(&mut app.notifications_to_close) {
            transport.close(id);
        }
        for (account, event) in std::mem::take(&mut self.notification_updates) {
            match app.notification_policy.group(
                &self.runtime,
                &app.notifier,
                &app.settings.value,
                &account,
                &event,
            ) {
                crate::notification_policy::Decision::None => {}
                crate::notification_policy::Decision::Close(key) => {
                    app.notification_policy.pending.remove(&key);
                    if let Some(id) = app.notifier.withdraw(&key) {
                        transport.close(id);
                    }
                }
                crate::notification_policy::Decision::Show(mut show) => {
                    if let (Some((root, _)), Some(owner)) = (
                        &app.upload_context,
                        self.runtime.engine.accounts.get(&account),
                    ) {
                        let message = event["added_notifications"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .take(100)
                            .filter(|n| {
                                n["is_silent"] != true
                                    && n["type"]["@type"] == "notificationTypeNewMessage"
                            })
                            .max_by_key(|n| crate::projection::integer(&n["id"], 0));
                        let chat = owner.state.chats.get(&show.key.chat);
                        let mut candidates = vec![];
                        if let Some(notification) =
                            message.filter(|n| n["type"]["show_preview"] == true)
                        {
                            let shown = owner.state.message(&notification["type"]["message"]);
                            let media = &shown["content"]["media"];
                            if matches!(
                                shown["content"]["kind"].as_str(),
                                Some("sticker" | "photo" | "gif" | "video" | "videoNote")
                            ) {
                                if matches!(
                                    media["thumb"]["format"].as_str(),
                                    Some("jpeg" | "png" | "webp")
                                ) {
                                    candidates.push(media["thumb"]["file"].clone());
                                }
                                if shown["content"]["kind"] == "photo"
                                    || shown["content"]["kind"] == "sticker"
                                        && media["format"] == "webp"
                                {
                                    candidates.push(media["file"].clone());
                                }
                                candidates.push(media["mini"].clone());
                            }
                        }
                        if let Some(chat) = chat {
                            candidates.extend([
                                chat["photo"]["file"].clone(),
                                chat["photo"]["mini"].clone(),
                            ]);
                        }
                        if let Some(path) = crate::notification_policy::picture(
                            home,
                            root,
                            &owner.state.roots,
                            &candidates,
                        ) {
                            show.notification.image = path.to_string_lossy().into_owned();
                        }
                    }
                    if app.notification_policy.pending.len() >= 512 {
                        continue;
                    }
                    let Some(serial) = app.notification_policy.serial.checked_add(1) else {
                        continue;
                    };
                    app.notification_policy.serial = serial;
                    if transport.show(show.key.clone(), serial, show.notification) {
                        app.notification_policy
                            .pending
                            .insert(show.key, (serial, show.generation, show.message, show.seed));
                    }
                }
            }
        }
        let mut sounds = vec![];
        let mut opens = vec![];
        for event in transport.poll() {
            match event {
                crate::notification_bus::Event::Shown(key, serial, id) => {
                    let pending = app.notification_policy.pending.get(&key).copied();
                    if pending.is_some_and(|(s, _, _, _)| s != serial) {
                        continue;
                    }
                    if !pending.is_some_and(|(s, g, _, _)| {
                        s == serial
                            && !self.runtime.stopping
                            && self
                                .runtime
                                .engine
                                .accounts
                                .get(&key.account)
                                .is_some_and(|a| a.client > 0 && a.generation == g)
                    }) || app.notifier.focused.as_ref() == Some(&key)
                        || app.settings.value["quiet"] == true
                    {
                        transport.close(id);
                        continue;
                    }
                    let (_, generation, message, seed) =
                        app.notification_policy.pending.remove(&key).unwrap();
                    if app.notification_policy.shown_generations.len() >= 512 {
                        app.notification_policy.shown_generations.clear();
                    }
                    app.notification_policy
                        .shown_generations
                        .insert(key.clone(), generation);
                    if app.notifier.shown(key.clone(), id)
                        && message != 0
                        && app.notification_policy.sound(
                            seed,
                            &app.settings.value,
                            Some(home),
                            Instant::now(),
                        )
                    {
                        sounds.push((key.account, seed));
                    }
                }
                crate::notification_bus::Event::Closed(id) => app.notifier.closed(id),
                crate::notification_bus::Event::Action(id, action) => {
                    if let Some(key) = app.notifier.action(id, &action) {
                        if !app
                            .notification_policy
                            .shown_generations
                            .get(&key)
                            .is_some_and(|g| {
                                self.runtime
                                    .engine
                                    .accounts
                                    .get(&key.account)
                                    .is_some_and(|a| a.client > 0 && a.generation == *g)
                            })
                        {
                            continue;
                        }
                        if let Some(query) =
                            app.notification_policy.button(&key, &action, &self.runtime)
                        {
                            let next = self.runtime.notification_button(
                                &key.account,
                                query,
                                Instant::now(),
                            );
                            effects.native.extend(next.native);
                            if let Some(id) = app.notifier.withdraw(&key) {
                                transport.close(id);
                            }
                        } else if matches!(action.as_str(), "default" | "reply")
                            && self.runtime.engine.accounts.contains_key(&key.account)
                        {
                            let target = json!({"chatId":number(key.chat),"reply":action=="reply","account":key.account});
                            if let (Some(_), Some(generation)) = (
                                &app.desktop_context,
                                app.notification_policy.shown_generations.get(&key),
                            ) {
                                opens.push((
                                    key.account.clone(),
                                    *generation,
                                    target.clone(),
                                    action == "reply",
                                ));
                            }
                            if action == "reply" && app.desktop_context.is_some() {
                                // The overlay gets first refusal. Only an owned
                                // failed summon may fall back to the window.
                            } else if app.windows.is_empty() {
                                app.pending_open = Some((Instant::now(), target));
                            } else {
                                for peer in &app.windows {
                                    let mut event = target.clone();
                                    event["event"] = json!("open");
                                    effects.replies.push((*peer, event));
                                }
                            }
                        }
                    }
                }
                crate::notification_bus::Event::Unavailable => {}
            }
        }
        for (account, seed) in sounds {
            self.start_sound(app, &account, seed);
        }
        if let Some(context) = &app.desktop_context {
            for (account, generation, target, reply) in opens {
                if self.desktop_helpers.len() < 8
                    && let Ok(helper) =
                        crate::desktop::start(context, account, generation, target, reply)
                {
                    self.desktop_helpers.push(helper);
                }
            }
        }
    }
    fn poll_desktop(&mut self, app: &mut crate::application::Application, effects: &mut Effects) {
        let mut fallback = Vec::new();
        self.desktop_helpers.retain_mut(|owner| {
            if self.runtime.stopping
                || !self
                    .runtime
                    .engine
                    .accounts
                    .get(&owner.account)
                    .is_some_and(|a| a.client > 0 && a.generation == owner.generation)
            {
                return false;
            }
            match owner.helper.poll() {
                Ok(None) => true,
                Ok(Some(output)) => {
                    if !output.ok() && owner.fallback {
                        fallback.push((
                            owner.account.clone(),
                            owner.generation,
                            owner.target.clone(),
                        ));
                    }
                    false
                }
                Err(_) => {
                    if owner.fallback {
                        fallback.push((
                            owner.account.clone(),
                            owner.generation,
                            owner.target.clone(),
                        ));
                    }
                    false
                }
            }
        });
        for (account, generation, target) in fallback {
            if app.windows.is_empty() {
                app.pending_open = Some((Instant::now(), target.clone()));
            } else {
                for peer in &app.windows {
                    let mut event = target.clone();
                    event["event"] = json!("open");
                    effects.replies.push((*peer, event));
                }
            }
            if let Some(context) = &app.desktop_context
                && self.desktop_helpers.len() < 8
                && let Ok(helper) =
                    crate::desktop::start(context, account, generation, target, false)
            {
                self.desktop_helpers.push(helper);
            }
        }
    }
    pub fn tick_application(
        &mut self,
        application: &mut crate::application::Application,
        home: &crate::storage::Storage,
        parameters: &mut dyn FnMut(&str) -> Result<Option<Value>, String>,
    ) -> io::Result<Effects> {
        let mut effects = self.tick(parameters)?;
        self.pump_globals(application, home, &mut effects);
        self.pump_bridge(application, home, &mut effects)?;
        self.pump_notifications(application, home, &mut effects);
        self.poll_desktop(application, &mut effects);
        let effects = self.dispatch(effects)?;
        self.start_pending_sounds(application);
        application.diagnostic_runtime["jobs"] = json!(self.worker_count());
        application.diagnostic_runtime["task_queue"] =
            json!(self.startup_queue.len() + self.global_queue.len());
        application.diagnostic_runtime["receiver_alive"] = json!(!self.runtime.stopping);
        application.diagnostic_runtime["td_idle_ms"] = self
            .last_receive
            .map_or(Value::Null, |time| json!(time.elapsed().as_millis()));
        application.diagnostic_runtime["counts"]["td_events"] = json!(self.received_events);
        application.diagnostic_runtime["playing"] = json!(self.player.is_some());
        application.diagnostic_runtime["recording"] = json!(self.capture.is_some());
        application.diagnostic_runtime["children"] = json!(
            usize::from(self.player.is_some())
                + self.file_openers.len()
                + self.desktop_helpers.len()
                + usize::from(application.bridge.running.is_some())
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
            self.last_receive = Some(Instant::now());
            self.received_events = self.received_events.saturating_add(1);
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
            self.observe_notification(&event);
            let next = self
                .runtime
                .receive(event, Instant::now(), parameters, &mut |query| {
                    self.td.execute(query).ok().flatten()
                });
            let next = self.dispatch(next)?;
            effects.replies.extend(next.replies);
            if !self.runtime.stopping && self.td.0.is_some() {
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
            if self.bridge_owners.contains_key(&done.token) {
                self.bridge_done.push(done);
                continue;
            }
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
        let completed = self.global_jobs.poll(&|worker| {
            !self.runtime.stopping
                && self.global_owner.is_some_and(|(owned, request)| {
                    worker == owned
                        && request.is_none_or(|token| Self::global_current(&self.runtime, token))
                })
        });
        for done in completed {
            if let Some((worker, request)) = self.global_owner.take()
                && worker == done.token
            {
                self.global_done.push((request, done.result));
            }
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
        if self
            .capture
            .as_mut()
            .is_some_and(crate::capture::Capture::poll)
        {
            let mut capture = self.capture.take().expect("polled capture");
            let account = capture.account.clone();
            let record = capture.retain_stopped();
            if let Some(owner) = self.runtime.engine.accounts.get_mut(&account) {
                owner
                    .state
                    .retained
                    .recover_capture(record, capture.owned_file());
                effects.replies.push((
                    0,
                    json!({"event":"recording","state":"idle","account":account}),
                ));
                effects.replies.push((0, json!({"event":"recordedSend","account":account,"recordedSend":owner.state.retained.view(&account)})));
            }
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
        self.notification_transport = None;
        self.notification_updates.clear();
        self.file_openers.clear();
        self.sound_jobs = Default::default();
        self.sound_owners.clear();
        self.clipboard_owner = None;
        self.jobs = crate::jobs::Pool::default();
        self.global_jobs = Default::default();
        self.global_owner = None;
        self.global_queue.clear();
        self.global_done.clear();
        self.bridge_owners.clear();
        self.bridge_done.clear();
        self.bridge_operations.clear();
        self.bridge_mutations.clear();
        self.bridge_restore.clear();
        self.desktop_helpers.clear();
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
    #[ignore = "requires explicit inert proxy runtime;only loopback readiness,no Telegram"]
    fn actual_proxy_child_loss_and_saved_bridge_recovery_preserve_native_identity() {
        let binary = std::env::var_os("TELEBAR_TEST_PROXY_BINARY").expect("exact inert runtime");
        let root =
            std::env::temp_dir().join(format!("telebar-bridge-child-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let home = crate::storage::Storage::open(&root).unwrap();
        let mut app = crate::application::Application::load(&home).unwrap();
        app.bridge.context = Some(crate::bridge::Context {
            binary: binary.into(),
            environment: Default::default(),
        });
        let mut driver = Driver::without_native("default".into(), "en".into());
        let now = Instant::now();
        for (id, client) in [("default", 41), ("other", 42)] {
            driver
                .runtime
                .register(id.into(), id.into(), client, vec![], now)
                .unwrap();
        }
        driver
            .runtime
            .metadata
            .get_mut("other")
            .unwrap()
            .bridge_ready = true;
        app.bridge.metadata = json!({"default":{"proxy_id":7,"previous_id":4},"other":{"proxy_id":9,"previous_id":5}});
        let crate::commands::Action::BridgeReady(running) = app
            .bridge
            .context
            .clone()
            .unwrap()
            .start(&std::sync::atomic::AtomicBool::new(false))
            .unwrap()
        else {
            panic!("proxy readiness");
        };
        let old = running.helper.pid();
        app.bridge.running = Some(running);
        app.bridge
            .running
            .as_mut()
            .unwrap()
            .terminate_owned()
            .unwrap();
        let until = Instant::now() + Duration::from_secs(3);
        while app.bridge.running.is_some() && Instant::now() < until {
            app.bridge.poll_at(now);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(app.bridge.running.is_none());
        assert!(!Path::new(&format!("/proc/{old}")).exists());
        assert_eq!(
            app.bridge.restart_after,
            Some(now + Duration::from_secs(30))
        );
        let mut effects = Effects::default();
        driver
            .pump_bridge_at(&mut app, &home, &mut effects, now + Duration::from_secs(29))
            .unwrap();
        assert!(driver.bridge_restore.is_empty());
        // Advance one explicit owner clock: confirmed child loss, not native
        // request uncertainty or a sleeping retry loop. Unready first account
        // cannot starve later account; capture effects BEFORE native dispatch.
        driver
            .pump_bridge_at(&mut app, &home, &mut effects, now + Duration::from_secs(31))
            .unwrap();
        assert_eq!(driver.bridge_operations.len(), 1);
        let (token, operation) = driver.bridge_operations.pop().unwrap();
        let (account, generation) = driver.runtime.engine.job_owner(token).unwrap();
        assert_eq!(account, "other");
        assert!(matches!(operation, crate::bridge::Operation::Enable));
        assert_eq!(generation, 1);
        assert!(app.bridge.restart_after.is_none());
        let crate::commands::Action::BridgeReady(running) = app
            .bridge
            .context
            .clone()
            .unwrap()
            .start(&std::sync::atomic::AtomicBool::new(false))
            .unwrap()
        else {
            panic!("proxy recovery");
        };
        let new = running.helper.pid();
        assert_ne!(new, old);
        app.bridge.running = Some(running);
        let action = driver.bridge_action(&app, &account, operation).unwrap();
        let first = driver
            .runtime
            .engine
            .job_complete(token, Ok(action), Instant::now());
        assert_eq!(first.native[0].query["@type"], "getProxies");
        let reply = json!({"@type":"proxies","@client_id":42,"@extra":first.native[0].query["@extra"],"proxies":[{"id":9,"is_enabled":true},{"id":5}]});
        let next = driver.runtime.engine.receive(reply);
        assert_eq!(next.native[0].query["@type"], "editProxy");
        assert_eq!(next.native[0].query["proxy_id"], 9);
        assert_eq!(
            next.native[0].query["proxy"]["port"],
            app.bridge.running.as_ref().unwrap().port
        );
        let completed = driver.runtime.engine.receive(
            json!({"@type":"proxy","id":9,"@client_id":42,"@extra":next.native[0].query["@extra"]}),
        );
        driver
            .bridge_owners
            .insert(token, (account.clone(), operation));
        app.bridge.busy.insert(account);
        driver.bridge_mutations = completed.bridge_mutations;
        driver
            .pump_bridge_at(&mut app, &home, &mut effects, Instant::now())
            .unwrap();
        assert_eq!(
            app.bridge.metadata["other"],
            json!({"proxy_id":9,"previous_id":5})
        );
        assert!(app.bridge.busy.is_empty());
        assert!(
            home.read(".config/omagram/bridge.json", 8192)
                .unwrap()
                .is_some()
        );
        // Remaining unready profile stays queued and no repeated native
        // mutation is submitted after one settled recovery.
        driver
            .pump_bridge_at(&mut app, &home, &mut effects, Instant::now())
            .unwrap();
        assert!(driver.bridge_operations.is_empty());
        assert_eq!(
            driver.bridge_restore.iter().cloned().collect::<Vec<_>>(),
            ["default"]
        );
        app.bridge.running = None;
        assert!(!Path::new(&format!("/proc/{new}")).exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn spontaneous_capture_exit_is_polled_retained_and_never_sent() {
        let root =
            std::env::temp_dir().join(format!("telebar-capture-exit-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let home = crate::storage::Storage::open(&root).unwrap();
        let mut app = crate::application::Application::load(&home).unwrap();
        let context = crate::session::Context::new(
            root.clone(),
            root.clone(),
            Default::default(),
            "en".into(),
        )
        .unwrap();
        context.configure_application(&mut app).unwrap();
        let mut driver = Driver::without_native("default".into(), "en".into());
        driver
            .register("default".into(), "Synthetic".into(), vec![])
            .unwrap();
        let account = driver.runtime.engine.accounts.get_mut("default").unwrap();
        account.client = 41;
        account.ready = true;
        account
            .state
            .retained
            .attach(crate::recording_checkpoint::Checkpoint::open(&root, &root, "default").unwrap())
            .unwrap();
        let capture = crate::capture::Capture::start_with(
            &crate::capture::Context {
                home: root.clone(),
                runtime: root.clone(),
                environment: Default::default(),
            },
            "default".into(),
            1,
            "voice",
            101,
            Some(
                [
                    "-re",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=440:duration=0.4",
                ]
                .map(str::to_owned)
                .to_vec(),
            ),
        )
        .unwrap();
        let path = capture.path.clone();
        let pid = capture.pid();
        driver.capture = Some(capture);
        let until = Instant::now() + Duration::from_secs(2);
        let mut frames = Vec::new();
        loop {
            let effects = driver
                .tick_application(&mut app, &home, &mut |_| panic!("no startup"))
                .unwrap();
            assert!(
                effects.native.is_empty() && effects.work.is_empty(),
                "natural exit must not send or prepare"
            );
            frames.extend(effects.replies.into_iter().map(|(_, frame)| frame));
            if driver.capture.is_none() {
                break;
            }
            assert!(
                Instant::now() < until,
                "spontaneous recorder exit remained recording"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            frames
                .iter()
                .any(|frame| frame["event"] == "recording" && frame["state"] == "idle")
        );
        assert_eq!(app.diagnostic_runtime["recording"], false);
        assert!(path.exists());
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        let view = driver.runtime.engine.accounts["default"]
            .state
            .retained
            .view("default");
        assert_eq!(view["state"], "prepareFailed");
        assert_eq!(view["chatId"], 101);
        let token = view["token"].as_str().unwrap();
        let original = path.with_extension("original");
        std::fs::rename(&path, &original).unwrap();
        std::fs::write(&path, b"substituted retry bytes").unwrap();
        let plan = crate::recording_jobs::plan(
            "recording.retry",
            &json!({"token":token}),
            &driver.runtime.engine.accounts["default"].state,
            root.clone(),
            &root,
            "default",
            0.0,
        );
        assert!(
            plan.is_err(),
            "retry admitted a substituted input descriptor"
        );
        assert!(
            driver.jobs.active() == 0,
            "no preparation or native send at rejection"
        );
        std::fs::remove_file(&path).unwrap();
        std::fs::rename(&original, &path).unwrap();
        assert!(
            crate::recording_jobs::plan(
                "recording.retry",
                &json!({"token":token}),
                &driver.runtime.engine.accounts["default"].state,
                root.clone(),
                &root,
                "default",
                0.0
            )
            .is_ok(),
            "unchanged original can be explicitly retried"
        );
        let restored = crate::recording_checkpoint::Checkpoint::open(&root, &root, "default")
            .unwrap()
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(restored.token, token);
        assert_eq!(restored.path, path.to_str().unwrap());
        let request = Request::parse(
            json!({"id":1,"cmd":"recording.discard","args":{"token":token}})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        let discarded = driver
            .request_application(&mut app, &home, 7, request)
            .unwrap();
        assert!(
            discarded
                .replies
                .iter()
                .any(|(_, frame)| frame["id"] == 1 && frame["ok"] == true)
        );
        assert!(!path.exists());
        if let Some(fixture) = std::env::var_os("TELEBAR_CAPTURE_EXIT_EXPORT") {
            // Explicit development fixture preserves the exact Driver-generated
            // checkpoint for native Socket/Main recovery checks, never dispatch.
            let root = std::path::PathBuf::from(fixture);
            let store = crate::storage::Storage::open(&root).unwrap();
            let mut owner = Driver::without_native("default".into(), "en".into());
            owner
                .register("default".into(), "Synthetic UI".into(), vec![])
                .unwrap();
            let account = owner.runtime.engine.accounts.get_mut("default").unwrap();
            account.client = 41;
            account.ready = true;
            account
                .state
                .retained
                .attach(
                    crate::recording_checkpoint::Checkpoint::open(&root, &root, "default").unwrap(),
                )
                .unwrap();
            owner.capture = Some(
                crate::capture::Capture::start_with(
                    &crate::capture::Context {
                        home: root.clone(),
                        runtime: root.clone(),
                        environment: Default::default(),
                    },
                    "default".into(),
                    1,
                    "voice",
                    101,
                    Some(
                        [
                            "-re",
                            "-f",
                            "lavfi",
                            "-i",
                            "sine=frequency=440:duration=0.2",
                        ]
                        .map(str::to_owned)
                        .to_vec(),
                    ),
                )
                .unwrap(),
            );
            let until = Instant::now() + Duration::from_secs(2);
            while owner.capture.is_some() {
                let effects = owner.tick(&mut |_| panic!("no startup")).unwrap();
                assert!(effects.native.is_empty() && effects.work.is_empty());
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(
                owner.runtime.engine.accounts["default"]
                    .state
                    .retained
                    .view("default")["state"],
                "prepareFailed"
            );
            assert!(
                store
                    .read(".local/share/omagram/retained-recording-v1.json", 32768)
                    .unwrap()
                    .is_some()
            );
            owner
                .runtime
                .engine
                .accounts
                .get_mut("default")
                .unwrap()
                .client = 0;
            owner.close(Duration::ZERO).unwrap();
        }
        let mut pids = vec![];
        for fault in ["input", "root", "checkpoint"] {
            let context = crate::capture::Context {
                home: root.clone(),
                runtime: root.clone(),
                environment: Default::default(),
            };
            let capture = crate::capture::Capture::start_with(
                &context,
                "default".into(),
                1,
                "voice",
                101,
                Some(
                    [
                        "-re",
                        "-f",
                        "lavfi",
                        "-i",
                        "sine=frequency=440:duration=0.15",
                    ]
                    .map(str::to_owned)
                    .to_vec(),
                ),
            )
            .unwrap();
            let relative = capture.path.strip_prefix(&root).unwrap().to_owned();
            let raw = capture.path.clone();
            let moved = root.with_extension("moved");
            let mut original = None;
            if fault == "input" {
                let saved = raw.with_extension("original");
                std::fs::rename(&raw, &saved).unwrap();
                std::fs::write(&raw, b"foreign replacement").unwrap();
                original = Some(saved);
            } else if fault == "root" {
                std::fs::rename(&root, &moved).unwrap();
                std::fs::create_dir(&root).unwrap();
                crate::storage::Storage::open(&root)
                    .unwrap()
                    .write(relative.to_str().unwrap(), b"foreign root", 0o600)
                    .unwrap();
            } else {
                std::fs::remove_file(root.join(".local/share/omagram/retained-recording-v1.json"))
                    .unwrap();
                home.ensure_dir(".local/share/omagram/retained-recording-v1.json")
                    .unwrap();
            }
            pids.push(capture.pid());
            driver.capture = Some(capture);
            let until = Instant::now() + Duration::from_secs(2);
            while driver.capture.is_some() {
                let effects = driver
                    .tick_application(&mut app, &home, &mut |_| panic!("no startup"))
                    .unwrap();
                assert!(effects.native.is_empty() && effects.work.is_empty());
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(2));
            }
            let view = driver.runtime.engine.accounts["default"]
                .state
                .retained
                .view("default");
            let token = view["token"].as_str().unwrap();
            if fault != "checkpoint" {
                assert_eq!(view["state"], "unknown");
                if fault == "input" {
                    assert_eq!(
                        crate::recording_checkpoint::Checkpoint::open(&root, &root, "default")
                            .unwrap()
                            .load()
                            .unwrap()
                            .unwrap()
                            .status,
                        crate::recorded_send::Status::Unknown
                    );
                } else {
                    let saved: Value = serde_json::from_slice(
                        &home
                            .read(".local/share/omagram/retained-recording-v1.json", 32768)
                            .unwrap()
                            .unwrap(),
                    )
                    .unwrap();
                    assert_eq!(saved["state"], "unknown");
                    assert_eq!(saved["input"]["path"], raw.to_str().unwrap());
                    assert!(saved["input"]["ino"].as_u64().is_some());
                }
                assert_eq!(
                    std::fs::read(&raw).unwrap(),
                    if fault == "root" {
                        b"foreign root".as_slice()
                    } else {
                        b"foreign replacement".as_slice()
                    }
                );
            } else {
                assert_eq!(view["state"], "prepareFailed");
                assert!(raw.exists());
                std::fs::remove_dir(root.join(".local/share/omagram/retained-recording-v1.json"))
                    .unwrap();
                driver.runtime.engine.accounts["default"]
                    .state
                    .retained
                    .persist()
                    .unwrap();
            }
            let request = Request::parse(
                json!({"id":2,"cmd":"recording.discard","args":{"token":token}})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
            let result = driver
                .request_application(&mut app, &home, 7, request)
                .unwrap();
            assert!(
                result
                    .replies
                    .iter()
                    .any(|(_, f)| f["id"] == 2 && f["ok"] == true)
            );
            if fault == "checkpoint" {
                assert!(!raw.exists());
            }
            if let Some(original) = original {
                assert!(original.exists());
            }
            if fault == "root" {
                assert!(moved.join(&relative).exists());
                std::fs::remove_dir_all(&root).unwrap();
                std::fs::rename(&moved, &root).unwrap();
            }
        }
        for pid in pids {
            assert!(!Path::new(&format!("/proc/{pid}")).exists());
        }
        driver
            .runtime
            .engine
            .accounts
            .get_mut("default")
            .unwrap()
            .client = 0;
        driver.close(Duration::ZERO).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "requires an explicitly provided verified TDLib library"]
    fn bridge_disable_persists_only_current_owner_and_releases_admission() {
        let library = std::env::var_os("TELEBAR_TEST_TDLIB").expect("verified library");
        let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
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
        let root =
            std::env::temp_dir().join(format!("telebar-bridge-owner-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let home = crate::storage::Storage::open(&root).unwrap();
        let mut app = crate::application::Application::load(&home).unwrap();
        let request = || {
            Request::parse(b"{\"id\":1,\"cmd\":\"bridge.set\",\"args\":{\"enabled\":false}}")
                .unwrap()
        };
        let admitted = |engine: &mut crate::engine::Engine, request| {
            engine.request_using(7, request, Instant::now(), &mut |_, _, _| {
                Ok(crate::commands::Action::BridgeOperation(
                    crate::bridge::Operation::Disable,
                ))
            })
        };
        let mut effects = admitted(&mut driver.runtime.engine, request());
        let token = effects.bridge_operations[0].0;
        driver
            .bridge_owners
            .insert(token, ("default".into(), crate::bridge::Operation::Disable));
        app.bridge.busy.insert("default".into());
        app.bridge.metadata = json!({"default":{"proxy_id":3,"previous_id":4}});
        driver.runtime.engine.retire_peer(7);
        driver
            .bridge_mutations
            .push((token, crate::bridge::Mutation::Disable));
        driver.pump_bridge(&mut app, &home, &mut effects).unwrap();
        assert!(app.bridge.metadata.get("default").is_some());
        assert!(app.bridge.busy.is_empty());
        assert!(
            home.read(".config/omagram/bridge.json", 8192)
                .unwrap()
                .is_none()
        );
        let mut effects = admitted(&mut driver.runtime.engine, request());
        let token = effects.bridge_operations[0].0;
        driver
            .bridge_owners
            .insert(token, ("default".into(), crate::bridge::Operation::Disable));
        app.bridge.busy.insert("default".into());
        driver
            .bridge_mutations
            .push((token, crate::bridge::Mutation::Disable));
        effects.bridge_operations.clear();
        driver.pump_bridge(&mut app, &home, &mut effects).unwrap();
        assert_eq!(effects.replies.len(), 1);
        assert_eq!(effects.replies[0].1["ok"], true);
        assert!(app.bridge.busy.is_empty());
        assert_eq!(
            home.read(".config/omagram/bridge.json", 8192)
                .unwrap()
                .unwrap(),
            b"{}"
        );
        driver.runtime.stopping = true;
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "requires an explicitly provided verified TDLib library"]
    fn globals_retire_before_dispatch_and_cannot_ack_a_reused_request() {
        let library = std::env::var_os("TELEBAR_TEST_TDLIB").expect("verified library");
        let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
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
        let root = std::env::temp_dir().join(format!("telebar-globals-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let home = crate::storage::Storage::open(&root).unwrap();
        let mut app = crate::application::Application::load(&home).unwrap();
        app.settings.global_context = Some(crate::global_shortcuts::Context {
            plugin: root.clone(),
            environment: Default::default(),
        });
        let req = |command: &str, args: Value| {
            Request::parse(
                json!({"id":1,"cmd":command,"args":args})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap()
        };
        // No tick: retirement removes the saved operation before any helper.
        let mut effect = app.request(
            &home,
            &mut driver.runtime,
            7,
            req(
                "settings.set",
                json!({"settings":{"globalShortcuts":{"global.panel":"SUPER + P"}}}),
            ),
        );
        driver.global_queue.extend(
            effect
                .work
                .drain(..)
                .map(|(token, work)| (Some(token), work)),
        );
        driver.runtime.engine.retire_peer(7);
        driver.pump_globals(&mut app, &home, &mut effect);
        assert!(driver.global_owner.is_none());
        assert!(driver.global_queue.is_empty());
        assert_eq!(app.settings.global_status["global.panel"], "failed");
        assert!(!app.settings.globals_pending);
        assert!(effect.replies.iter().all(|(peer, _)| *peer == 0));
        // Capture the old token's outcome, then reuse its wire ID. Completion
        // cannot terminal the new request; observers still get the final status.
        let mut effect = app.request(
            &home,
            &mut driver.runtime,
            7,
            req("shortcuts.apply", json!({})),
        );
        let (old, _) = effect.work.pop().unwrap();
        driver.runtime.engine.retire_peer(7);
        let answer = json!({"settings":app.settings.value,"globalStatus":{"global.panel":"unavailable","global.quickReply":"off","global.openWindow":"off"}});
        driver
            .global_done
            .push((Some(old), Ok(crate::commands::Action::Reply(answer))));
        let mut next = app.request(
            &home,
            &mut driver.runtime,
            7,
            req("settings.quiet", json!({"quiet":true})),
        );
        driver
            .global_queue
            .extend(next.work.drain(..).map(|(token, work)| (Some(token), work)));
        driver.pump_globals(&mut app, &home, &mut next);
        let replies: Vec<_> = next.replies.iter().filter(|(peer, _)| *peer == 7).collect();
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].1["result"]["settings"]["quiet"], true);
        assert_eq!(app.settings.global_status["global.panel"], "unavailable");
        driver.runtime.stopping = true;
        std::fs::remove_dir_all(root).unwrap();
    }
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
        let runtime = std::env::var("XDG_RUNTIME_DIR").expect("isolated runtime");
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
            ("XDG_RUNTIME_DIR".into(), runtime.clone()),
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
        // Actual clipboard.image/files request owners read only this private
        // selection, not merely the offer-verification helper.
        let private_home = crate::storage::Storage::open(Path::new(&runtime)).unwrap();
        let mut app = crate::application::Application::load(&private_home).unwrap();
        app.upload_context = Some((Path::new(&runtime).to_owned(), vec![]));
        app.clipboard_context = Some(crate::clipboard::Context {
            runtime: Path::new(&runtime).to_owned(),
            databases: vec![],
            environment: environment.clone(),
        });
        for (rid, command) in [(31, "clipboard.image"), (32, "clipboard.files")] {
            let request = Request::parse(
                json!({"id":rid,"cmd":command,"args":{}})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
            let mut effects = driver
                .request_application(&mut app, &private_home, 11, request)
                .unwrap();
            let until = Instant::now() + Duration::from_secs(8);
            let answer = loop {
                if let Some((_, answer)) = effects.replies.into_iter().find(|(_, v)| v["id"] == rid)
                {
                    break answer;
                }
                assert!(Instant::now() < until, "privateclipboard reply");
                effects = driver
                    .tick_application(&mut app, &private_home, &mut |_| panic!("no startup"))
                    .unwrap();
                std::thread::sleep(Duration::from_millis(2));
            };
            assert_eq!(answer["ok"], true, "{command}: {answer}");
            let saved = if command == "clipboard.image" {
                answer["result"]["path"].as_str().unwrap()
            } else {
                assert_eq!(answer["result"]["skipped"], 0);
                answer["result"]["paths"][0].as_str().unwrap()
            };
            assert!(Path::new(saved).starts_with(Path::new(&runtime).join("omagram/rec")));
            assert_eq!(std::fs::read(saved).unwrap(), data);
        }
        // Replace the exact offer with a foreground URI-list owner, then verify
        // whole-list admission excludes the private synthetic database and a
        // missing selection while preserving and deduplicating chosen paths.
        driver.clipboard_owner = None;
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        private_home
            .write("chosen.txt", b"chosen original", 0o600)
            .unwrap();
        private_home
            .write("database/private", b"synthetic database", 0o600)
            .unwrap();
        app.clipboard_context.as_mut().unwrap().databases =
            vec![Path::new(&runtime).join("database")];
        let uri = format!(
            "file://{runtime}/chosen.txt\nfile://{runtime}/database/private\nfile://{runtime}/missing\nfile://{runtime}/chosen.txt\n"
        );
        let mut owner = crate::process::Managed::start_owned(
            "/usr/bin/wl-copy",
            &["--foreground", "--type", "text/uri-list"],
            &environment,
            uri.as_bytes(),
            4096,
        )
        .unwrap();
        let uri_pid = owner.pid();
        let until = Instant::now() + Duration::from_secs(2);
        while !owner.input_sent() {
            assert!(Instant::now() < until);
            assert!(owner.poll().unwrap().is_none());
            std::thread::sleep(Duration::from_millis(2));
        }
        // Confirm native selection bytes before the admitted request; no retry
        // of application work or unrelated session clipboard discovery.
        let registered = Instant::now() + Duration::from_millis(120);
        while Instant::now() < registered {
            assert!(owner.poll().unwrap().is_none());
            std::thread::sleep(Duration::from_millis(2));
        }
        let verified = crate::process::run(
            "/usr/bin/wl-paste",
            &["--no-newline", "--type", "text/uri-list"],
            &environment,
            &[],
            Duration::from_secs(3),
            16384,
        )
        .unwrap();
        assert!(
            verified.ok(),
            "private URI registration: {}",
            String::from_utf8_lossy(&verified.stderr)
        );
        assert_eq!(verified.stdout, uri.as_bytes());
        let request = Request::parse(br#"{"id":33,"cmd":"clipboard.files","args":{}}"#).unwrap();
        let mut effects = driver
            .request_application(&mut app, &private_home, 11, request)
            .unwrap();
        let until = Instant::now() + Duration::from_secs(8);
        let answer = loop {
            if let Some((_, v)) = effects.replies.into_iter().find(|(_, v)| v["id"] == 33) {
                break v;
            }
            assert!(Instant::now() < until);
            effects = driver
                .tick_application(&mut app, &private_home, &mut |_| panic!("no startup"))
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(answer["ok"], true);
        assert_eq!(
            answer["result"]["paths"],
            json!([format!("{runtime}/chosen.txt")])
        );
        assert_eq!(answer["result"]["skipped"], 2);
        assert_eq!(
            std::fs::read(Path::new(&runtime).join("chosen.txt")).unwrap(),
            b"chosen original"
        );
        assert_eq!(
            std::fs::read(Path::new(&runtime).join("database/private")).unwrap(),
            b"synthetic database"
        );
        drop(owner);
        assert!(!Path::new(&format!("/proc/{uri_pid}")).exists());
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
