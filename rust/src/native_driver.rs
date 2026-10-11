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
    startup_queue: std::collections::VecDeque<StartupOwner>,
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
        while self.jobs.active() + self.startup_jobs.active() < 2 {
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
            let next = self.complete_startup(done, Instant::now());
            let next = self.dispatch(next)?;
            effects.replies.extend(next.replies);
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
    pub fn close(&mut self, timeout: Duration) -> io::Result<Effects> {
        self.runtime.stopping = true;
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
}
