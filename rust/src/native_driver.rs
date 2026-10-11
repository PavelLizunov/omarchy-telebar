//! One native/thread driver. Explicit account registrations and parameter
//! provider; no discovery of working accounts, keyring or real-home storage.
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
        })
    }
    pub fn set_startup_context(&mut self, context: crate::startup::Context) {
        self.startup_context = Some(context);
    }
    fn start_parameters(&mut self, id: &str) -> Effects {
        let mut effects = Effects::default();
        let Some(context) = self.startup_context.clone() else {
            return effects;
        };
        if self.runtime.stopping {
            return effects;
        }
        if self.jobs.active() + self.startup_jobs.active() >= 2 {
            return self.runtime.configure(
                id,
                Err("still preparing an earlier request; try again in a moment".into()),
                Instant::now(),
            );
        }
        let Some(account) = self
            .runtime
            .engine
            .accounts
            .get(id)
            .filter(|a| a.client > 0)
        else {
            return effects;
        };
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
        let Some(next) = self.startup_serial.checked_add(1) else {
            return self.runtime.configure(
                id,
                Err("the startup worker capacity was exhausted".into()),
                Instant::now(),
            );
        };
        let token = crate::lifecycle::Token::startup(self.startup_serial);
        self.startup_serial = next;
        match self.startup_jobs.start(
            token,
            crate::jobs::Work::Startup {
                context,
                account: id.into(),
            },
        ) {
            Ok(()) => {
                self.startup_owners.insert(token, owner);
            }
            Err(_) => {
                effects = self.runtime.configure(
                    id,
                    Err("still preparing an earlier request; try again in a moment".into()),
                    Instant::now(),
                );
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
        for (token, work) in std::mem::take(&mut effects.work) {
            let started = if self.jobs.active() + self.startup_jobs.active() >= 2 {
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
            self.td.send(native.client, &native.query)?;
        }
        Ok(effects)
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
            "videonote.send" | "recording.retry" | "recording.discard"
        ) {
            let context = application.upload_context.clone();
            let runtime = application.recording_runtime.clone();
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
        self.dispatch(effects)
    }
    pub fn tick_application(
        &mut self,
        application: &mut crate::application::Application,
        home: &crate::storage::Storage,
        parameters: &mut dyn FnMut(&str) -> Result<Option<Value>, String>,
    ) -> io::Result<Effects> {
        let effects = self.tick(parameters)?;
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
            let Some(owner) = self.startup_owners.remove(&done.token) else {
                continue;
            };
            if !owner.current(&self.runtime) {
                continue;
            }
            let query = if owner.started.elapsed() >= Duration::from_secs(180) {
                Err("the startup credential preparation deadline was exceeded".into())
            } else {
                match done.result {
                    Ok(crate::commands::Action::StartupPrepared(prepared)) => prepared.0,
                    _ => Err("the startup credentials could not be prepared".into()),
                }
            };
            let next = self
                .runtime
                .configure(&owner.account, query, Instant::now());
            let next = self.dispatch(next)?;
            effects.replies.extend(next.replies);
        }
        let expired = self.runtime.expire(Instant::now());
        effects.replies.extend(expired.replies);
        Ok(effects)
    }
    pub fn close(&mut self, timeout: Duration) -> io::Result<Effects> {
        self.runtime.stopping = true;
        self.jobs = crate::jobs::Pool::default();
        self.startup_jobs = crate::jobs::Pool::default();
        self.startup_owners.clear();
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
