//! Single-thread account/request owner. The driver executes returned native
//! effects; this module never opens storage, loads credentials or retries sends.
use crate::{
    commands::{Action, Answer},
    lifecycle::{Admission, Pending, Requests, Token},
    projection::integer,
    state::State,
    wire::{self, Request},
};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::{collections::HashMap, time::Instant};

pub struct Account {
    pub client: i32,
    pub generation: u64,
    pub state: State,
    pub ready: bool,
}
pub struct NativeRequest {
    pub client: i32,
    pub query: Value,
}
#[derive(Default)]
pub struct Effects {
    pub replies: Vec<(u64, Value)>,
    pub native: Vec<NativeRequest>,
    pub work: Vec<(Token, crate::jobs::Work)>,
    pub disconnect: Option<u64>,
    pub credentials_saved: Vec<(String, u64)>,
}
struct Callback {
    token: Token,
    answer: Answer,
    account: String,
    generation: u64,
    batch_key: Option<String>,
}
struct BatchOwner {
    aggregate: crate::batch::Aggregate,
    values: IndexMap<String, Option<Value>>,
    remaining: std::collections::HashSet<String>,
}
pub struct Engine {
    pub accounts: IndexMap<String, Account>,
    pub active: String,
    requests: Requests,
    callbacks: HashMap<u64, Callback>,
    batches: HashMap<Token, BatchOwner>,
    next_extra: u64,
    recordings: HashMap<Token, (String, String)>,
}
impl Engine {
    pub fn reserve_external(&mut self, count: usize) {
        self.requests.reserve_external(count);
    }
    pub fn native_pending(&self) -> usize {
        self.callbacks.len()
    }
    pub fn new(active: String) -> Self {
        Self {
            accounts: IndexMap::new(),
            active,
            requests: Requests::default(),
            callbacks: HashMap::new(),
            batches: HashMap::new(),
            next_extra: 1,
            recordings: HashMap::new(),
        }
    }
    pub fn request(&mut self, peer: u64, request: Request, now: Instant) -> Effects {
        self.request_with(peer, request, now, 0.0, "en", &mut |_| None)
    }
    pub fn request_with(
        &mut self,
        peer: u64,
        request: Request,
        now: Instant,
        wall_time: f64,
        language: &str,
        execute: &mut dyn FnMut(&Value) -> Option<Value>,
    ) -> Effects {
        self.request_using(peer, request, now, &mut |command, args, state| {
            crate::sending::plan(command, args, state, wall_time, language, execute)
        })
    }
    pub fn request_using(
        &mut self,
        peer: u64,
        request: Request,
        now: Instant,
        planner: &mut dyn FnMut(&str, &Value, &State) -> crate::commands::Result<Action>,
    ) -> Effects {
        let mut effects = Effects::default();
        let rid = request.id;
        if self.requests.duplicate(peer, &rid) {
            effects = self.retire_peer(peer);
            effects.disconnect = Some(peer);
            return effects;
        }
        let Some(needs_ready) = crate::registry::needs_ready(&request.command) else {
            effects
                .replies
                .push((peer, wire::failure(rid, "unknown command", 0)));
            return effects;
        };
        let mut account = if request.account.is_null() {
            self.active.clone()
        } else if let Some(id) = request.account.as_str() {
            id.into()
        } else {
            effects
                .replies
                .push((peer, wire::failure(rid, "unknown account", 0)));
            return effects;
        };
        let Some(session) = self.accounts.get(&account) else {
            effects
                .replies
                .push((peer, wire::failure(rid, "unknown account", 0)));
            return effects;
        };
        let mut generation = session.generation;
        let token = match self.requests.admit(Pending {
            peer,
            id: rid.clone(),
            account: account.clone(),
            generation,
            started: now,
        }) {
            Ok(token) => token,
            Err(Admission::Duplicate) => {
                effects = self.retire_peer(peer);
                effects.disconnect = Some(peer);
                return effects;
            }
            Err(_) => {
                effects.replies.push((
                    peer,
                    wire::failure(rid, "Too many pending requests; wait for the service", 0),
                ));
                return effects;
            }
        };
        // Command readiness remains the current registry's contract. Unknown
        // handlers fail closed rather than being forwarded by guessed TD names.
        let result = if needs_ready && !session.ready {
            Err("not signed in".into())
        } else {
            planner(
                &request.command,
                &Value::Object(request.args),
                &session.state,
            )
        };
        let result = match result {
            Ok(Action::AccountNative {
                account: target,
                query,
                answer,
            }) => {
                if let Some(target_session) = self.accounts.get(&target) {
                    generation = target_session.generation;
                    account = target;
                    if let Some(p) = self.requests.get_mut(token) {
                        p.account = account.clone();
                        p.generation = generation;
                    }
                    Ok(Action::Native { query, answer })
                } else {
                    Err("unknown account".into())
                }
            }
            other => other,
        };
        let result = match result {
            Ok(Action::CredentialsSaved) => {
                effects
                    .credentials_saved
                    .push((account.clone(), generation));
                Ok(Action::Reply(json!({})))
            }
            Ok(Action::DiscardRecorded(record)) => {
                self.discard_recorded(&account, &record, &mut effects)
            }
            Ok(Action::PrepareRecorded {
                recording,
                preparation,
            }) => self.prepare_recorded(token, &account, recording, *preparation, &mut effects),
            Ok(Action::Recorded {
                token: record,
                content,
                options,
            }) => self.recorded_action(token, &account, record, content, options, &mut effects),
            other => other,
        };
        let session = self
            .accounts
            .get(&account)
            .expect("admitted account exists");
        match result {
            Ok(Action::CredentialsSaved)
            | Ok(Action::DiscardRecorded(_))
            | Ok(Action::PrepareRecorded { .. })
            | Ok(Action::Recorded { .. }) => {
                unreachable!("recorded action resolved")
            }
            Ok(Action::AccountNative { .. }) => unreachable!("account routing resolved"),
            Err(error) => self.finish(token, Err((error, 0)), &mut effects),
            Ok(Action::Reply(result)) => self.finish(token, Ok(result), &mut effects),
            Ok(Action::Work(work)) => {
                if let crate::jobs::Work::Recording(prep) = &work {
                    self.recordings
                        .insert(token, (account.clone(), prep.token.clone()));
                }
                effects.work.push((token, work));
            }
            Ok(Action::Batch(batch)) => {
                self.start_batch(token, batch, &account, generation, &mut effects)
            }
            Ok(Action::Native { mut query, answer }) => {
                let extra = self.next_extra;
                let Some(next) = extra.checked_add(1) else {
                    self.finish(
                        token,
                        Err((
                            "Too many pending Telegram requests; wait for the service".into(),
                            0,
                        )),
                        &mut effects,
                    );
                    return effects;
                };
                self.next_extra = next;
                if !self.requests.track_td(extra, token) {
                    self.finish(
                        token,
                        Err((
                            "Too many pending Telegram requests; wait for the service".into(),
                            0,
                        )),
                        &mut effects,
                    );
                    return effects;
                }
                query["@extra"] = json!(extra);
                effects.native.push(NativeRequest {
                    client: session.client,
                    query,
                });
                self.callbacks.insert(
                    extra,
                    Callback {
                        token,
                        answer,
                        account,
                        generation,
                        batch_key: None,
                    },
                );
            }
        }
        effects
    }
    fn start_batch(
        &mut self,
        token: Token,
        batch: crate::batch::Batch,
        account: &str,
        generation: u64,
        effects: &mut Effects,
    ) {
        let count = batch.queries.len();
        let Some(session) = self.accounts.get(account) else {
            return;
        };
        let client = session.client;
        let end = self.next_extra.checked_add(count as u64);
        let mut keys = std::collections::HashSet::new();
        if end.is_none()
            || !self.requests.can_track(token, count)
            || batch
                .queries
                .iter()
                .any(|(key, _, _)| !keys.insert(key.clone()))
        {
            self.finish(
                token,
                Err((
                    "Too many pending Telegram requests; wait for the service".into(),
                    0,
                )),
                effects,
            );
            return;
        }
        let values = batch
            .queries
            .iter()
            .map(|(key, _, _)| (key.clone(), None))
            .collect();
        let owner = BatchOwner {
            aggregate: batch.aggregate,
            values,
            remaining: keys,
        };
        if count == 0 {
            self.finish(
                token,
                owner.aggregate.complete(owner.values).map_err(|e| (e, 0)),
                effects,
            );
            return;
        }
        self.batches.insert(token, owner);
        for (key, mut query, answer) in batch.queries {
            let extra = self.next_extra;
            self.next_extra += 1;
            // Capacity and all identities were admitted before publishing effects.
            assert!(self.requests.track_td(extra, token));
            query["@extra"] = json!(extra);
            effects.native.push(NativeRequest { client, query });
            self.callbacks.insert(
                extra,
                Callback {
                    token,
                    answer,
                    account: account.into(),
                    generation,
                    batch_key: Some(key),
                },
            );
        }
    }
    fn finish(
        &mut self,
        token: Token,
        result: std::result::Result<Value, (String, i64)>,
        effects: &mut Effects,
    ) {
        if let Some((account, record)) = self.recordings.remove(&token)
            && let Some(a) = self.accounts.get_mut(&account)
        {
            let _ = a.state.retained.cancelled(&record);
            effects.replies.push((0,json!({"event":"recordedSend","account":account,"recordedSend":a.state.retained.view(&account)})));
        }
        if let Some(p) = self.requests.terminal(token) {
            let value = match result {
                Ok(v) => wire::success(p.id, v),
                Err((error, code)) => wire::failure(p.id, &error, code),
            };
            effects.replies.push((p.peer, value));
        }
        self.callbacks.retain(|_, c| c.token != token);
        self.batches.remove(&token);
    }
    pub fn receive(&mut self, event: Value) -> Effects {
        self.receive_with(event, &mut |_| None)
    }
    pub fn receive_with(
        &mut self,
        event: Value,
        execute: &mut dyn FnMut(&Value) -> Option<Value>,
    ) -> Effects {
        self.receive_at(event, Instant::now(), execute)
    }
    pub fn receive_at(
        &mut self,
        event: Value,
        now: Instant,
        execute: &mut dyn FnMut(&Value) -> Option<Value>,
    ) -> Effects {
        // Check the deadline before consuming a native answer, not after a
        // receive burst. Late success cannot start a continuation or retry.
        let mut effects = self.expire(now);
        let next = self.receive_current(event, execute);
        effects.replies.extend(next.replies);
        effects.native.extend(next.native);
        effects.work.extend(next.work);
        effects.credentials_saved.extend(next.credentials_saved);
        effects.disconnect = next.disconnect;
        effects
    }
    fn receive_current(
        &mut self,
        event: Value,
        execute: &mut dyn FnMut(&Value) -> Option<Value>,
    ) -> Effects {
        let mut effects = Effects::default();
        let client = event["@client_id"]
            .as_i64()
            .and_then(|v| i32::try_from(v).ok());
        let account = match client {
            Some(client) => self
                .accounts
                .iter()
                .find(|(_, a)| a.client == client)
                .map(|(id, _)| id.clone()),
            None if self.accounts.len() == 1 => self.accounts.keys().next().cloned(),
            _ => None,
        };
        let Some(account) = account else {
            return effects;
        };
        let Some(session) = self.accounts.get_mut(&account) else {
            return effects;
        };
        let generation = session.generation;
        let extra = event["@extra"].as_u64();
        if let Some(extra) = extra
            && let Some(callback) = self.callbacks.get(&extra)
        {
            if callback.account != account || callback.generation != generation {
                return effects;
            }
            if self
                .requests
                .td_response(extra, &account, generation)
                .is_none()
            {
                return effects;
            }
            let Some(callback) = self.callbacks.remove(&extra) else {
                return effects;
            };
            if let Some(key) = callback.batch_key {
                let value = if event["@type"] == "error" {
                    if self.batches.get(&callback.token).is_some_and(|b| {
                        matches!(b.aggregate, crate::batch::Aggregate::Uploads { .. })
                    }) {
                        let message = crate::projection::text(&event["message"], 256);
                        Some(
                            json!({"uploadError":if message.is_empty(){"Telegram did not take the files"}else{&message}}),
                        )
                    } else {
                        None
                    }
                } else {
                    callback
                        .answer
                        .complete(&event, &session.state)
                        .ok()
                        .map(|done| done.result)
                };
                if let Some(batch) = self.batches.get_mut(&callback.token) {
                    batch.values.insert(key.clone(), value);
                    batch.remaining.remove(&key);
                    if batch.remaining.is_empty() {
                        let batch = self
                            .batches
                            .remove(&callback.token)
                            .expect("retained batch owner");
                        let result = batch
                            .aggregate
                            .complete(batch.values)
                            .map_err(|error| (error, 0));
                        self.finish(callback.token, result, &mut effects);
                    }
                }
                return effects;
            }
            if let Answer::Recorded(record) = &callback.answer {
                let result = if event["@type"] == "error" {
                    let code = i64::try_from(integer(&event["code"], 0)).unwrap_or(0);
                    session
                        .state
                        .retained
                        .reject(record, code)
                        .map(|error| Err((error.into(), code)))
                        .unwrap_or_else(|error| Err((error.into(), 0)))
                } else {
                    let projected = session.state.message(&event);
                    if projected.is_null() {
                        let _ = session.state.retained.cancelled(record);
                        Err(("The recorded message may still be sent; check Telegram before discarding; retry is disabled".into(),0))
                    } else {
                        match session.state.retained.accepted(record, &event) {
                            Ok(()) => Ok(projected),
                            Err(_) => {
                                let _ = session.state.retained.cancelled(record);
                                Err(("The recorded message may still be sent; check Telegram before discarding; retry is disabled".into(),0))
                            }
                        }
                    }
                };
                effects.replies.push((0,json!({"event":"recordedSend","account":account,"recordedSend":session.state.retained.view(&account)})));
                self.recordings.remove(&callback.token);
                self.finish(callback.token, result, &mut effects);
                return effects;
            }
            if event["@type"] == "error" {
                let result = callback.answer.failed(&event).map_err(|error| {
                    (
                        error,
                        i64::try_from(integer(&event["code"], 0)).unwrap_or(0),
                    )
                });
                self.finish(callback.token, result, &mut effects);
            } else {
                let done = callback
                    .answer
                    .complete_with(&event, &session.state, execute);
                match done {
                    Err(error) => self.finish(callback.token, Err((error, 0)), &mut effects),
                    Ok(done) => {
                        let client = session.client;
                        if let Some(next) = done.next {
                            match *next {
                                Action::CredentialsSaved
                                | Action::DiscardRecorded(_)
                                | Action::PrepareRecorded { .. }
                                | Action::Recorded { .. } => self.finish(
                                    callback.token,
                                    Err(("invalid recording continuation".into(), 0)),
                                    &mut effects,
                                ),
                                Action::AccountNative { .. } => self.finish(
                                    callback.token,
                                    Err(("invalid account continuation".into(), 0)),
                                    &mut effects,
                                ),
                                Action::Work(work) => effects.work.push((callback.token, work)),
                                Action::Reply(result) => {
                                    self.finish(callback.token, Ok(result), &mut effects)
                                }
                                Action::Batch(batch) => self.start_batch(
                                    callback.token,
                                    batch,
                                    &account,
                                    generation,
                                    &mut effects,
                                ),
                                Action::Native { mut query, answer } => {
                                    let extra = self.next_extra;
                                    if let Some(next) = extra.checked_add(1)
                                        && self.requests.track_td(extra, callback.token)
                                    {
                                        self.next_extra = next;
                                        query["@extra"] = json!(extra);
                                        effects.native.push(NativeRequest { client, query });
                                        self.callbacks.insert(
                                            extra,
                                            Callback {
                                                token: callback.token,
                                                answer,
                                                account,
                                                generation,
                                                batch_key: None,
                                            },
                                        );
                                    } else {
                                        self.finish(callback.token,Err(("Too many pending Telegram requests; wait for the service".into(),0)),&mut effects);
                                    }
                                }
                            }
                            return effects;
                        }
                        self.finish(callback.token, Ok(done.result), &mut effects);
                        effects.native.extend(
                            done.followups
                                .into_iter()
                                .map(|query| NativeRequest { client, query }),
                        );
                    }
                }
            }
            return effects;
        }
        // An expired reply is not an update, and never starts more business work.
        if !event["@type"]
            .as_str()
            .is_some_and(|s| s.starts_with("update"))
        {
            return effects;
        }
        if event["@type"] == "updateAuthorizationState"
            && event["authorization_state"]["@type"] == "authorizationStateClosed"
        {
            session.ready = false;
            let roots = std::mem::take(&mut session.state.roots);
            let mut retained = std::mem::take(&mut session.state.retained);
            if let Some(record) = &mut retained.record {
                if record.status == crate::recorded_send::Status::Preparing {
                    record.status = crate::recorded_send::Status::PrepareFailed;
                } else if record.status == crate::recorded_send::Status::Sending {
                    record.status = crate::recorded_send::Status::Unknown;
                }
            }
            session.state = State::new(roots);
            session.state.retained = retained;
            // A closed native session cannot be reused. The driver must allocate
            // a new client before admitting work for a new generation.
            session.generation = session.generation.saturating_add(1);
            session.client = 0;
            for p in self.requests.retire_account(&account) {
                effects.replies.push((
                    p.peer,
                    wire::failure(p.id, "the account session was closed", 0),
                ));
            }
            self.callbacks.retain(|_, c| c.account != account);
            self.batches.retain(|token, _| self.requests.owns(*token));
            self.retire_recordings(&mut effects);
        }
        let Some(session) = self.accounts.get_mut(&account) else {
            return effects;
        };
        for mut event in session.state.apply(&event) {
            event["account"] = json!(account);
            effects.replies.push((0, event));
        }
        effects
    }
    fn discard_recorded(
        &mut self,
        account: &str,
        token: &str,
        effects: &mut Effects,
    ) -> crate::commands::Result<Action> {
        let retained = &mut self
            .accounts
            .get_mut(account)
            .ok_or("unknown account")?
            .state
            .retained;
        retained.discard(token)?;
        effects.replies.push((
            0,
            json!({"event":"recordedSend","account":account,"recordedSend":null}),
        ));
        Ok(Action::Reply(json!({})))
    }
    fn prepare_recorded(
        &mut self,
        token: Token,
        account: &str,
        recording: Option<crate::recorded_send::Recording>,
        preparation: crate::recording_jobs::Preparation,
        effects: &mut Effects,
    ) -> crate::commands::Result<Action> {
        if preparation.account != account {
            return Err("invalid recording continuation".into());
        }
        if let Some(ref recording) = recording
            && self.accounts.values().any(|a| {
                a.state
                    .retained
                    .record
                    .as_ref()
                    .is_some_and(|r| r.source.as_deref() == Some(&recording.path))
            })
        {
            return Err("that recording already belongs to a pending send".into());
        }
        let retained = &mut self
            .accounts
            .get_mut(account)
            .ok_or("unknown account")?
            .state
            .retained;
        if let Some(recording) = recording {
            if recording.token != preparation.token {
                return Err("invalid recording continuation".into());
            }
            retained.reserve(recording)?;
        } else {
            if !retained.retry(&preparation.token)? {
                return Err("invalid recording continuation".into());
            }
            retained.checked(&preparation.token)?.status = crate::recorded_send::Status::Preparing;
        }
        effects.replies.push((
            0,
            json!({"event":"recordedSend","account":account,"recordedSend":retained.view(account)}),
        ));
        self.recordings
            .insert(token, (account.into(), preparation.token.clone()));
        Ok(Action::Work(crate::jobs::Work::Recording(preparation)))
    }
    fn recorded_action(
        &mut self,
        token: Token,
        account: &str,
        record: String,
        content: Value,
        options: Value,
        effects: &mut Effects,
    ) -> crate::commands::Result<Action> {
        // Native capacity is checked before changing a retained input to
        // Sending. A local refusal is not an uncertain Telegram delivery.
        if self.next_extra.checked_add(1).is_none() || !self.requests.can_track(token, 1) {
            return Err("Too many pending Telegram requests; wait for the service".into());
        }
        let state = &mut self
            .accounts
            .get_mut(account)
            .ok_or("unknown account")?
            .state;
        let retained = state.retained.checked(&record)?;
        let chat = retained.chat;
        let args = retained.args.clone();
        let topic = crate::commands::topic(&args)?;
        let reply = crate::commands::reply(&args)?;
        if !matches!(
            retained.status,
            crate::recorded_send::Status::Preparing | crate::recorded_send::Status::Rejected
        ) {
            return Err(
                "retry requires a confirmed rejection; this message may still be sent".into(),
            );
        }
        let field = if retained.kind == "voice" {
            "voice_note"
        } else {
            "video_note"
        };
        if let Some(path) = content[field][field]["path"].as_str() {
            if retained.path != path && retained.source.is_none() {
                retained.source = Some(retained.path.clone());
            }
            retained.path = path.into();
        }
        retained.content = Some(content.clone());
        state.retained.sending(&record)?;
        self.recordings
            .insert(token, (account.into(), record.clone()));
        effects.replies.push((0,json!({"event":"recordedSend","account":account,"recordedSend":state.retained.view(account)})));
        Ok(Action::Native {
            query: json!({"@type":"sendMessage","chat_id":crate::projection::number(chat),"topic_id":topic,"reply_to":reply,"options":options,"reply_markup":null,"input_message_content":content}),
            answer: Answer::Recorded(record),
        })
    }
    pub fn job_current(&self, token: Token) -> bool {
        self.requests.owns(token)
    }
    pub fn job_complete(
        &mut self,
        token: Token,
        result: crate::commands::Result<Action>,
        now: Instant,
    ) -> Effects {
        let mut effects = self.expire(now);
        let Some(p) = self.requests.get(token) else {
            return effects;
        };
        let account = p.account.clone();
        let generation = p.generation;
        let Some(session) = self
            .accounts
            .get(&account)
            .filter(|a| a.generation == generation && a.client > 0)
        else {
            return effects;
        };
        let client = session.client;
        let result = match result {
            Ok(Action::CredentialsSaved) => {
                effects
                    .credentials_saved
                    .push((account.clone(), generation));
                Ok(Action::Reply(json!({})))
            }
            Ok(Action::DiscardRecorded(record)) => {
                self.discard_recorded(&account, &record, &mut effects)
            }
            Ok(Action::PrepareRecorded {
                recording,
                preparation,
            }) => self.prepare_recorded(token, &account, recording, *preparation, &mut effects),
            Ok(Action::Recorded {
                token: record,
                content,
                options,
            }) => self.recorded_action(token, &account, record, content, options, &mut effects),
            other => other,
        };
        match result {
            Ok(Action::CredentialsSaved)
            | Ok(Action::DiscardRecorded(_))
            | Ok(Action::PrepareRecorded { .. })
            | Ok(Action::Recorded { .. }) => {
                unreachable!("recorded action resolved")
            }
            Ok(Action::AccountNative { .. }) => self.finish(
                token,
                Err(("invalid account continuation".into(), 0)),
                &mut effects,
            ),
            Err(error) => self.finish(token, Err((error, 0)), &mut effects),
            Ok(Action::Reply(value)) => self.finish(token, Ok(value), &mut effects),
            Ok(Action::Work(work)) => {
                if let crate::jobs::Work::Recording(prep) = &work {
                    self.recordings
                        .insert(token, (account.clone(), prep.token.clone()));
                }
                effects.work.push((token, work));
            }
            Ok(Action::Batch(batch)) => {
                self.start_batch(token, batch, &account, generation, &mut effects)
            }
            Ok(Action::Native { mut query, answer }) => {
                let extra = self.next_extra;
                if let Some(next) = extra.checked_add(1)
                    && self.requests.track_td(extra, token)
                {
                    self.next_extra = next;
                    query["@extra"] = json!(extra);
                    effects.native.push(NativeRequest { client, query });
                    self.callbacks.insert(
                        extra,
                        Callback {
                            token,
                            answer,
                            account,
                            generation,
                            batch_key: None,
                        },
                    );
                } else {
                    self.finish(
                        token,
                        Err((
                            "Too many pending Telegram requests; wait for the service".into(),
                            0,
                        )),
                        &mut effects,
                    );
                }
            }
        }
        effects
    }
    pub fn expire(&mut self, now: Instant) -> Effects {
        let mut effects = Effects::default();
        for p in self.requests.expire(now) {
            effects.replies.push((
                p.peer,
                wire::failure(p.id, "Telegram did not answer within three minutes; the operation may still complete; check before retrying", 0),
            ));
        }
        self.callbacks
            .retain(|_, c| self.requests.current(c.token, &c.account, c.generation));
        self.batches.retain(|token, _| self.requests.owns(*token));
        self.retire_recordings(&mut effects);
        effects
    }
    pub fn cancel_all(&mut self, reason: &str) -> Effects {
        let mut effects = Effects::default();
        for id in self.accounts.keys() {
            for pending in self.requests.retire_account(id) {
                effects
                    .replies
                    .push((pending.peer, wire::failure(pending.id, reason, 0)));
            }
        }
        self.callbacks.clear();
        self.batches.clear();
        self.retire_recordings(&mut effects);
        effects
    }
    fn retire_recordings(&mut self, effects: &mut Effects) {
        let stale: Vec<_> = self
            .recordings
            .keys()
            .filter(|t| !self.requests.owns(**t))
            .copied()
            .collect();
        for token in stale {
            if let Some((account, record)) = self.recordings.remove(&token)
                && let Some(a) = self.accounts.get_mut(&account)
            {
                let _ = a.state.retained.cancelled(&record);
                effects.replies.push((0,json!({"event":"recordedSend","account":account,"recordedSend":a.state.retained.view(&account)})));
            }
        }
    }
    pub fn retire_peer(&mut self, peer: u64) -> Effects {
        let mut effects = Effects::default();
        self.requests.retire_peer(peer);
        self.retire_recordings(&mut effects);
        self.batches.retain(|token, _| self.requests.owns(*token));
        self.callbacks
            .retain(|_, c| self.requests.current(c.token, &c.account, c.generation));
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn engine() -> Engine {
        let mut e = Engine::new("default".into());
        for (id, client) in [("default", 1), ("other", 2)] {
            e.accounts.insert(
                id.into(),
                Account {
                    client,
                    generation: 1,
                    state: State::default(),
                    ready: true,
                },
            );
        }
        e
    }
    fn request(id: u64, account: &str) -> Request {
        Request::parse(
            json!({"id":id,"cmd":"chat.history","account":account,"args":{"chatId":42}})
                .to_string()
                .as_bytes(),
        )
        .unwrap_or_else(|_| panic!("fixture"))
    }
    #[test]
    fn duplicate_id_disconnects_even_with_unknown_command_or_account() {
        for (command, account) in [("no-such-command", "default"), ("chat.history", "missing")] {
            let mut e = engine();
            let now = Instant::now();
            e.request(1, request(7, "default"), now);
            let req = Request::parse(
                json!({"id":7,"cmd":command,"account":account,"args":{"chatId":42}})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
            assert_eq!(e.request(1, req, now).disconnect, Some(1));
            assert!(e.callbacks.is_empty());
        }
    }
    #[test]
    fn native_arrival_checks_deadline_before_continuation() {
        let mut e = engine();
        let now = Instant::now();
        let req = Request::parse(
            br#"{"id":8,"cmd":"notifications.set","args":{"scope":"groups","muted":true}}"#,
        )
        .unwrap();
        let sent = e.request(1, req, now);
        let done=e.receive_at(json!({"@type":"scopeNotificationSettings","@client_id":1,"@extra":sent.native[0].query["@extra"]}),now+crate::lifecycle::REQUEST_TIMEOUT,&mut |_|None);
        assert!(done.native.is_empty());
        assert_eq!(done.replies.len(), 1);
        assert_eq!(done.replies[0].1["ok"], false);
    }
    #[test]
    fn batch_responses_are_ordered_owned_and_terminal_once() {
        let mut e = engine();
        let now = Instant::now();
        let request = Request::parse(br#"{"id":7,"cmd":"notifications.get"}"#).unwrap();
        let sent = e.request(1, request, now);
        assert_eq!(sent.native.len(), 3);
        let wrong=e.receive(json!({"@type":"error","@client_id":2,"@extra":sent.native[0].query["@extra"],"message":"wrong"}));
        assert!(wrong.replies.is_empty());
        for index in [2, 0] {
            let done=e.receive(json!({"@type":"scopeNotificationSettings","@client_id":1,"@extra":sent.native[index].query["@extra"],"mute_for":index}));
            assert!(done.replies.is_empty());
        }
        let last = json!({"@type":"error","@client_id":1,"@extra":sent.native[1].query["@extra"],"message":"synthetic rejection","code":400});
        let done = e.receive(last.clone());
        assert_eq!(done.replies.len(), 1);
        let scopes = done.replies[0].1["result"]["scopes"].as_object().unwrap();
        assert_eq!(
            scopes.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["private", "groups", "channels"]
        );
        assert!(scopes["groups"].is_null());
        assert!(e.receive(last).replies.is_empty());
        assert!(e.batches.is_empty() && e.callbacks.is_empty());
    }
    #[test]
    fn batch_admission_is_atomic_and_expiry_retires_every_sibling() {
        let mut e = engine();
        let now = Instant::now();
        let req = || Request::parse(br#"{"id":7,"cmd":"privacy.get"}"#).unwrap();
        e.next_extra = u64::MAX - 3;
        let rejected = e.request(1, req(), now);
        assert!(rejected.native.is_empty());
        assert_eq!(rejected.replies.len(), 1);
        assert!(e.batches.is_empty() && e.callbacks.is_empty());
        e.next_extra = 1;
        let sent = e.request(1, req(), now);
        assert_eq!(sent.native.len(), 9);
        let timed = e.expire(now + crate::lifecycle::REQUEST_TIMEOUT);
        assert_eq!(timed.replies.len(), 1);
        assert!(e.batches.is_empty() && e.callbacks.is_empty());
        e.request(1, req(), now + crate::lifecycle::REQUEST_TIMEOUT);
        for native in sent.native {
            let late=e.receive(json!({"@type":"userPrivacySettingRules","@client_id":1,"@extra":native.query["@extra"],"rules":[]}));
            assert!(late.native.is_empty() && late.replies.is_empty());
        }
        e.retire_peer(1);
        assert!(e.batches.is_empty() && e.callbacks.is_empty());
    }
    #[test]
    fn ordered_account_updates_and_native_reply() {
        let mut e = engine();
        let now = Instant::now();
        let sent = e.request(1, request(7, "default"), now);
        let extra = sent.native[0].query["@extra"].clone();
        assert!(
            e.receive(json!({"@type":"messages","@client_id":2,"@extra":extra,"messages":[]}))
                .replies
                .is_empty()
        );
        let update=e.receive(json!({"@type":"updateConnectionState","@client_id":1,"state":{"@type":"connectionStateReady"}}));
        assert_eq!(
            update.replies[0].1,
            json!({"event":"connection","account":"default","state":"ready"})
        );
        let done =
            e.receive(json!({"@type":"messages","@client_id":1,"@extra":extra,"messages":[]}));
        assert_eq!(
            done.replies[0].1,
            json!({"id":7,"ok":true,"result":{"chatId":42,"messages":[]}})
        );
        assert!(
            e.receive(json!({"@client_id":1,"@extra":extra}))
                .replies
                .is_empty()
        );
    }
    #[test]
    fn expired_archive_never_loads_or_completes_reused_id() {
        let mut e = engine();
        let now = Instant::now();
        let req = Request::parse(
            br#"{"id":7,"cmd":"chat.archive","args":{"chatId":42,"archived":true}}"#,
        )
        .unwrap_or_else(|_| panic!("fixture"));
        let sent = e.request(1, req, now);
        let extra = sent.native[0].query["@extra"].clone();
        assert_eq!(
            e.expire(now + crate::lifecycle::REQUEST_TIMEOUT)
                .replies
                .len(),
            1
        );
        let new = e.request(
            1,
            request(7, "default"),
            now + crate::lifecycle::REQUEST_TIMEOUT,
        );
        let old = e.receive(json!({"@type":"ok","@client_id":1,"@extra":extra}));
        assert!(old.replies.is_empty() && old.native.is_empty());
        assert!(!new.native.is_empty());
    }
    #[test]
    fn expired_continuation_never_changes_settings() {
        let mut e = engine();
        let now = Instant::now();
        let req = Request::parse(
            br#"{"id":8,"cmd":"notifications.set","args":{"scope":"groups","muted":true}}"#,
        )
        .unwrap_or_else(|_| panic!("fixture"));
        let sent = e.request(1, req, now);
        let extra = sent.native[0].query["@extra"].clone();
        let done=e.receive(json!({"@type":"scopeNotificationSettings","@client_id":1,"@extra":extra,"show_preview":true}));
        assert!(done.replies.is_empty());
        assert_eq!(
            done.native[0].query["@type"],
            "setScopeNotificationSettings"
        );
        let next = done.native[0].query["@extra"].clone();
        assert_ne!(extra, next);
        e.expire(now + crate::lifecycle::REQUEST_TIMEOUT);
        let late = e.receive(json!({"@type":"ok","@client_id":1,"@extra":next}));
        assert!(late.replies.is_empty() && late.native.is_empty());
        assert_eq!(e.accounts["other"].generation, 1);
    }
    #[test]
    fn send_timeout_never_retries_native_mutation() {
        let mut e = engine();
        let now = Instant::now();
        let req = Request::parse(
            br#"{"id":8,"cmd":"message.send","args":{"chatId":42,"text":"synthetic"}}"#,
        )
        .unwrap_or_else(|_| panic!("fixture"));
        let sent = e.request(1, req, now);
        assert_eq!(sent.native.len(), 1);
        assert_eq!(sent.native[0].query["@type"], "sendMessage");
        let extra = sent.native[0].query["@extra"].clone();
        let timeout = e.expire(now + crate::lifecycle::REQUEST_TIMEOUT);
        assert!(timeout.native.is_empty());
        assert!(
            timeout.replies[0].1["error"]
                .as_str()
                .unwrap()
                .contains("may still complete")
        );
        let late = e.receive(json!({"@type":"message","@client_id":1,"@extra":extra,"id":42}));
        assert!(late.native.is_empty() && late.replies.is_empty());
    }
    #[test]
    fn duplicate_disconnect_and_closed_account_retire_callbacks() {
        let mut e = engine();
        let now = Instant::now();
        e.request(1, request(7, "default"), now);
        assert_eq!(e.request(1, request(7, "default"), now).disconnect, Some(1));
        let sent = e.request(2, request(7, "other"), now);
        let closed=e.receive(json!({"@type":"updateAuthorizationState","@client_id":2,"authorization_state":{"@type":"authorizationStateClosed"}}));
        assert_eq!(closed.replies.len(), 1);
        let old = e.receive(
            json!({"@type":"messages","@client_id":2,"@extra":sent.native[0].query["@extra"]}),
        );
        assert!(old.replies.is_empty());
        assert!(!e.accounts["other"].ready);
        assert_eq!(e.accounts["default"].generation, 1);
    }
}
