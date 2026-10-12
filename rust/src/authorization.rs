//! Account startup ownership, separate from user IPC. Internal requests carry
//! native client + generation + deadline. No implicit credentials/profile access.
use crate::{
    engine::{Account, Effects, Engine, NativeRequest},
    projection::{integer, number, text},
    state::State,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
const STARTUP_TIMEOUT: Duration = Duration::from_secs(180);
#[derive(Debug)]
pub struct Metadata {
    pub name: String,
    pub phone: String,
    pub auth: Value,
    pub unread: Option<i128>,
    pub parameters_serial: u64,
    pub bridge_ready: bool,
}
impl Metadata {
    pub fn new(name: String) -> Self {
        Self {
            name,
            phone: String::new(),
            auth: json!({"state":"starting"}),
            unread: None,
            parameters_serial: 0,
            bridge_ready: false,
        }
    }
}
enum Internal {
    Parameters { serial: u64, auth: Value },
    Me,
    Scope(String),
    Stories(usize),
    Ignore,
}
struct Pending {
    account: String,
    client: i32,
    generation: u64,
    started: Instant,
    answer: Internal,
}
pub struct Runtime {
    pub engine: Engine,
    pub metadata: HashMap<String, Metadata>,
    pending: HashMap<String, Pending>,
    next: u64,
    pub stopping: bool,
}
impl Runtime {
    pub fn notification_button(&mut self, id: &str, query: Value, now: Instant) -> Effects {
        let mut effects = Effects::default();
        if !self.stopping
            && self.engine.accounts.get(id).is_some_and(|a| a.ready)
            && matches!(
                query["@type"].as_str(),
                Some("viewMessages" | "setChatNotificationSettings" | "addMessageReaction")
            )
        {
            self.query(id, query, Internal::Ignore, now, &mut effects);
        }
        effects
    }
    pub fn open_content(&mut self, id: &str, chat: i128, message: i128, now: Instant) -> Effects {
        let mut effects = Effects::default();
        if !self.stopping {
            self.query(id,json!({"@type":"openMessageContent","chat_id":number(chat),"message_id":number(message)}),Internal::Ignore,now,&mut effects);
        }
        effects
    }
    pub fn read_reactions(&mut self, id: &str, chat: i128, now: Instant) -> Effects {
        let mut effects = Effects::default();
        if !self.stopping && self.engine.accounts.get(id).is_some_and(|a| a.ready) {
            self.query(
                id,
                json!({"@type":"readAllChatReactions","chat_id":number(chat)}),
                Internal::Ignore,
                now,
                &mut effects,
            );
        }
        effects
    }
    pub fn parameters_pending(&self, id: &str) -> bool {
        self.pending.values().any(|p| p.account == id && matches!(&p.answer, Internal::Parameters { serial, auth }
            if self.metadata.get(id).is_some_and(|m| m.parameters_serial == *serial && m.auth == *auth)
            && self.engine.accounts.get(id).is_some_and(|a| a.client == p.client && a.generation == p.generation)))
    }
    pub fn internal_pending(&self) -> usize {
        self.pending.len()
    }
    pub fn new(active: String) -> Self {
        Self {
            engine: Engine::new(active),
            metadata: HashMap::new(),
            pending: HashMap::new(),
            next: 1,
            stopping: false,
        }
    }
    pub fn register(
        &mut self,
        id: String,
        name: String,
        client: i32,
        roots: Vec<String>,
        now: Instant,
    ) -> Result<Effects, &'static str> {
        if client <= 0
            || !crate::paths::valid_account_id(&id)
            || self.engine.accounts.values().any(|a| a.client == client)
            || self.engine.accounts.contains_key(&id)
        {
            return Err("invalid account client registration");
        }
        self.metadata.insert(id.clone(), Metadata::new(name));
        self.engine.accounts.insert(
            id.clone(),
            Account {
                client,
                generation: 1,
                state: State::new(roots),
                ready: false,
            },
        );
        let mut effects = Effects::default();
        self.query(
            &id,
            json!({"@type":"getOption","name":"version"}),
            Internal::Ignore,
            now,
            &mut effects,
        );
        Ok(effects)
    }
    pub fn replace_closed_client(
        &mut self,
        id: &str,
        client: i32,
        now: Instant,
    ) -> Result<Effects, &'static str> {
        if self.stopping || client <= 0 || self.engine.accounts.values().any(|a| a.client == client)
        {
            return Err("invalid replacement client");
        }
        let account = self.engine.accounts.get_mut(id).ok_or("unknown account")?;
        if account.client != 0 {
            return Err("account is not closed");
        }
        account.client = client;
        let mut effects = Effects::default();
        self.query(
            id,
            json!({"@type":"getOption","name":"version"}),
            Internal::Ignore,
            now,
            &mut effects,
        );
        Ok(effects)
    }
    fn query(
        &mut self,
        id: &str,
        mut query: Value,
        answer: Internal,
        now: Instant,
        effects: &mut Effects,
    ) {
        let Some(account) = self.engine.accounts.get(id).filter(|a| a.client > 0) else {
            return;
        };
        // Combined driver budget includes engine-owned user callbacks.
        if self.pending.len() + self.engine.native_pending() >= crate::wire::TD_PENDING_MAX {
            return;
        }
        let Some(next) = self.next.checked_add(1) else {
            return;
        };
        let extra = format!("startup:{}", self.next);
        self.next = next;
        query["@extra"] = json!(extra);
        effects.native.push(NativeRequest {
            client: account.client,
            query,
        });
        self.pending.insert(
            extra,
            Pending {
                account: id.into(),
                client: account.client,
                generation: account.generation,
                started: now,
                answer,
            },
        );
    }
    fn auth(&mut self, id: &str, value: Value, effects: &mut Effects) {
        if let Some(metadata) = self.metadata.get_mut(id) {
            metadata.auth = value.clone();
            effects
                .replies
                .push((0, json!({"event":"auth","account":id,"auth":value})));
        }
    }
    pub fn configure(
        &mut self,
        id: &str,
        query: Result<Option<Value>, String>,
        now: Instant,
    ) -> Effects {
        let mut effects = Effects::default();
        let Some(meta) = self.metadata.get_mut(id) else {
            return effects;
        };
        meta.parameters_serial = meta.parameters_serial.saturating_add(1);
        let serial = meta.parameters_serial;
        let auth = meta.auth.clone();
        match query {
            Ok(Some(query)) => {
                let before = self.pending.len();
                self.query(
                    id,
                    query,
                    Internal::Parameters { serial, auth },
                    now,
                    &mut effects,
                );
                if self.pending.len() == before {
                    self.auth(id,json!({"state":"error","reason":"Too many pending Telegram requests; wait for the service"}),&mut effects);
                }
            }
            Ok(None) => self.auth(id, json!({"state":"needCredentials"}), &mut effects),
            Err(reason) => self.auth(
                id,
                json!({"state":"error","reason":reason.chars().take(200).collect::<String>()}),
                &mut effects,
            ),
        }
        effects
    }
    pub fn credentials_saved(
        &mut self,
        id: &str,
        generation: u64,
        now: Instant,
        parameters: &mut dyn FnMut(&str) -> Result<Option<Value>, String>,
    ) -> Effects {
        let mut effects = self.credentials_start(id, generation);
        if effects.replies.is_empty() {
            return effects;
        }
        let configured = self.configure(id, parameters(id), now);
        effects.replies.extend(configured.replies);
        effects.native.extend(configured.native);
        effects
    }
    pub fn credentials_start(&mut self, id: &str, generation: u64) -> Effects {
        let mut effects = Effects::default();
        if self.stopping
            || !self
                .engine
                .accounts
                .get(id)
                .is_some_and(|a| a.generation == generation && a.client > 0)
            || !self.metadata.get(id).is_some_and(|m| {
                matches!(m.auth["state"].as_str(), Some("needCredentials" | "error"))
            })
        {
            return effects;
        }
        self.auth(id, json!({"state":"starting"}), &mut effects);
        effects
    }
    /// Parameter provider is explicit and injected. An inert consumer returns
    /// None; the final runtime owns verified keyring/storage preparation.
    pub fn receive(
        &mut self,
        event: Value,
        now: Instant,
        parameters: &mut dyn FnMut(&str) -> Result<Option<Value>, String>,
        execute: &mut dyn FnMut(&Value) -> Option<Value>,
    ) -> Effects {
        let mut effects = Effects::default();
        if self.stopping {
            if event["@type"] == "updateAuthorizationState"
                && event["authorization_state"]["@type"] == "authorizationStateClosed"
            {
                return self.engine.receive_at(event, now, execute);
            }
            // Do not resume pending startup or user business work while closing.
            return effects;
        }
        if let Some(extra) = event["@extra"].as_str() {
            let Some(p) = self.pending.get(extra) else {
                return effects;
            };
            let valid = self
                .engine
                .accounts
                .get(&p.account)
                .is_some_and(|a| a.client == p.client && a.generation == p.generation)
                && event["@client_id"].as_i64() == Some(i64::from(p.client));
            if !valid {
                return effects;
            }
            let p = self.pending.remove(extra).expect("current internal owner");
            // Arrival after deadline must not dispatch follow-up business work.
            if now.saturating_duration_since(p.started) >= STARTUP_TIMEOUT {
                if let Internal::Parameters { serial, auth } = &p.answer
                    && self.metadata[&p.account].parameters_serial == *serial
                    && self.metadata[&p.account].auth == *auth
                {
                    self.auth(&p.account,json!({"state":"error","reason":"Telegram transport failed; the operation may still complete; check before retrying"}),&mut effects);
                }
                return effects;
            }
            match p.answer {
                Internal::Parameters { serial, auth } => {
                    if self.metadata[&p.account].parameters_serial != serial {
                        return effects;
                    }
                    if event["@type"] == "error" {
                        if self.metadata[&p.account].auth != auth {
                            return effects;
                        }
                        let code = integer(&event["code"], 0);
                        let suffix = if (1..=i32::MAX as i128).contains(&code) {
                            format!(" (error {code})")
                        } else {
                            String::new()
                        };
                        self.auth(&p.account,json!({"state":"error","reason":format!("Telegram rejected the startup parameters{suffix}; check API credentials and TDLib compatibility")}),&mut effects);
                    } else {
                        self.metadata.get_mut(&p.account).unwrap().bridge_ready = true;
                    }
                }
                Internal::Me if event["@type"] != "error" => {
                    let id = integer(&event["id"], 0);
                    if id <= 0 {
                        return effects;
                    }
                    let account = self.engine.accounts.get_mut(&p.account).unwrap();
                    account.state.me_id = id;
                    account
                        .state
                        .apply(&json!({"@type":"updateUser","user":event}));
                    let meta = self.metadata.get_mut(&p.account).unwrap();
                    meta.phone = text(&event["phone_number"], 32);
                    let name = format!(
                        "{} {}",
                        text(&event["first_name"], 128),
                        text(&event["last_name"], 128)
                    );
                    if !name.trim().is_empty() {
                        meta.name = name.trim().into();
                    }
                    effects.replies.push((
                        0,
                        json!({"event":"me","account":p.account,"meId":number(id)}),
                    ));
                }
                Internal::Scope(kind) if event["@type"] != "error" => {
                    let account = self.engine.accounts.get_mut(&p.account).unwrap();
                    for mut update in account.state.apply(&json!({"@type":"updateScopeNotificationSettings","scope":{"@type":kind},"notification_settings":event})){update["account"]=json!(p.account);effects.replies.push((0,update));}
                }
                Internal::Stories(page) if event["@type"] != "error" && page + 1 < 20 => self
                    .query(
                        &p.account,
                        json!({"@type":"loadActiveStories","story_list":{"@type":"storyListMain"}}),
                        Internal::Stories(page + 1),
                        now,
                        &mut effects,
                    ),
                _ => {}
            }
            return effects;
        }
        let account = event["@client_id"].as_i64().and_then(|cid| {
            self.engine
                .accounts
                .iter()
                .find(|(_, a)| i64::from(a.client) == cid)
                .map(|(id, _)| id.clone())
        });
        if event["@type"] == "updateAuthorizationState" {
            let Some(id) = account else {
                return effects;
            };
            let kind = event["authorization_state"]["@type"].as_str().unwrap_or("");
            if kind == "authorizationStateClosed" {
                effects = self.engine.receive(event);
                self.pending.retain(|_, p| p.account != id);
                let meta = self.metadata.get_mut(&id).unwrap();
                meta.phone.clear();
                meta.unread = Some(0);
                meta.bridge_ready = false;
                if !self.stopping {
                    self.auth(&id, json!({"state":"starting"}), &mut effects);
                }
                return effects;
            }
            if kind == "authorizationStateWaitTdlibParameters" {
                return self.configure(&id, parameters(&id), now);
            }
            if kind == "authorizationStateReady" {
                if self.engine.accounts[&id].ready {
                    return effects;
                }
                self.engine.accounts.get_mut(&id).unwrap().ready = true;
                self.auth(&id, json!({"state":"ready"}), &mut effects);
                self.query(
                    &id,
                    json!({"@type":"loadChats","chat_list":{"@type":"chatListMain"},"limit":100}),
                    Internal::Ignore,
                    now,
                    &mut effects,
                );
                self.query(
                    &id,
                    json!({"@type":"getMe"}),
                    Internal::Me,
                    now,
                    &mut effects,
                );
                self.query(
                    &id,
                    json!({"@type":"loadActiveStories","story_list":{"@type":"storyListMain"}}),
                    Internal::Stories(0),
                    now,
                    &mut effects,
                );
                for (_, kind) in crate::batch::SCOPES {
                    self.query(
                        &id,
                        json!({"@type":"getScopeNotificationSettings","scope":{"@type":kind}}),
                        Internal::Scope(kind.into()),
                        now,
                        &mut effects,
                    );
                }
                self.query(&id,json!({"@type":"setOption","name":"notification_group_count_max","value":{"@type":"optionValueInteger","value":10}}),Internal::Ignore,now,&mut effects);
            } else {
                self.engine.accounts.get_mut(&id).unwrap().ready = false;
                self.auth(
                    &id,
                    crate::projection::auth_view(&event["authorization_state"]),
                    &mut effects,
                );
            }
            return effects;
        }
        if event["@type"] == "updateUnreadMessageCount"
            && crate::projection::list_key(&event["chat_list"]) == "main"
            && let Some(id) = account
        {
            self.metadata.get_mut(&id).unwrap().unread =
                Some(integer(&event["unread_unmuted_count"], 0).max(0));
        }
        self.engine.receive_at(event, now, execute)
    }
    pub fn expire(&mut self, now: Instant) -> Effects {
        let mut effects = self.engine.expire(now);
        let expired: Vec<_> = self
            .pending
            .iter()
            .filter_map(|(extra, p)| {
                (now.saturating_duration_since(p.started) >= STARTUP_TIMEOUT)
                    .then_some(extra.clone())
            })
            .collect();
        for extra in expired {
            if let Some(p) = self.pending.remove(&extra)
                && let Internal::Parameters { serial, auth } = p.answer
                && self.metadata[&p.account].parameters_serial == serial
                && self.metadata[&p.account].auth == auth
            {
                self.auth(&p.account,json!({"state":"error","reason":"Telegram transport failed; the operation may still complete; check before retrying"}),&mut effects);
            }
        }
        effects
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn runtime() -> Runtime {
        let mut r = Runtime::new("default".into());
        r.register(
            "default".into(),
            "Account 1".into(),
            1,
            vec![],
            Instant::now(),
        )
        .unwrap();
        r
    }
    #[test]
    fn startup_and_closed_generations_never_open_profiles_implicitly() {
        let mut r = runtime();
        let now = Instant::now();
        let mut absent = |_: &str| Ok(None);
        let mut execute = |_: &Value| None;
        let effect=r.receive(json!({"@type":"updateAuthorizationState","@client_id":1,"authorization_state":{"@type":"authorizationStateWaitTdlibParameters"}}),now,&mut absent,&mut execute);
        assert!(effect.native.is_empty());
        assert_eq!(r.metadata["default"].auth["state"], "needCredentials");
        let ready=r.receive(json!({"@type":"updateAuthorizationState","@client_id":1,"authorization_state":{"@type":"authorizationStateReady"}}),now,&mut absent,&mut execute);
        assert!(r.engine.accounts["default"].ready);
        assert_eq!(ready.native.len(), 7);
        let me = ready
            .native
            .iter()
            .find(|n| n.query["@type"] == "getMe")
            .unwrap()
            .query["@extra"]
            .clone();
        r.receive(json!({"@type":"updateAuthorizationState","@client_id":1,"authorization_state":{"@type":"authorizationStateClosed"}}),now,&mut absent,&mut execute);
        assert_eq!(r.engine.accounts["default"].client, 0);
        r.replace_closed_client("default", 2, now).unwrap();
        let late = r.receive(
            json!({"@type":"user","@client_id":1,"@extra":me,"id":7}),
            now,
            &mut absent,
            &mut execute,
        );
        assert!(late.replies.is_empty() && late.native.is_empty());
        assert_eq!(r.engine.accounts["default"].state.me_id, 0);
    }
    #[test]
    fn parameter_expiry_cannot_overwrite_later_auth_and_capacity_is_explicit() {
        let mut r = runtime();
        let now = Instant::now();
        let attempt = r.configure(
            "default",
            Ok(Some(json!({"@type":"setTdlibParameters"}))),
            now,
        );
        r.metadata.get_mut("default").unwrap().auth = json!({"state":"phone"});
        let expired = r.expire(now + STARTUP_TIMEOUT);
        assert!(expired.replies.is_empty());
        assert_eq!(r.metadata["default"].auth["state"], "phone");
        let late=r.receive(json!({"@type":"error","@client_id":1,"@extra":attempt.native[0].query["@extra"],"message":"secret"}),now+STARTUP_TIMEOUT,&mut |_|Ok(None),&mut |_|None);
        assert!(late.replies.is_empty());
        for _ in 0..crate::wire::TD_PENDING_MAX {
            r.query(
                "default",
                json!({"@type":"getOption","name":"version"}),
                Internal::Ignore,
                now,
                &mut Effects::default(),
            );
        }
        let full = r.configure(
            "default",
            Ok(Some(json!({"@type":"setTdlibParameters"}))),
            now,
        );
        assert!(full.native.is_empty());
        assert_eq!(
            r.metadata["default"].auth["reason"],
            "Too many pending Telegram requests; wait for the service"
        );
    }
    #[test]
    fn shutdown_never_resumes_story_or_user_continuations() {
        let mut r = runtime();
        let now = Instant::now();
        let mut absent = |_: &str| Ok(None);
        let mut execute = |_: &Value| None;
        let ready=r.receive(json!({"@type":"updateAuthorizationState","@client_id":1,"authorization_state":{"@type":"authorizationStateReady"}}),now,&mut absent,&mut execute);
        let story = ready
            .native
            .iter()
            .find(|n| n.query["@type"] == "loadActiveStories")
            .unwrap()
            .query["@extra"]
            .clone();
        r.stopping = true;
        let done = r.receive(
            json!({"@type":"ok","@client_id":1,"@extra":story}),
            now,
            &mut absent,
            &mut execute,
        );
        assert!(done.native.is_empty() && done.replies.is_empty());
        let closed=r.receive(json!({"@type":"updateAuthorizationState","@client_id":1,"authorization_state":{"@type":"authorizationStateClosed"}}),now,&mut absent,&mut execute);
        assert!(closed.native.is_empty());
        assert_eq!(r.engine.accounts["default"].client, 0);
    }
    #[test]
    fn parameter_attempts_have_deadlines_and_rejections_hide_secrets() {
        let mut r = runtime();
        let now = Instant::now();
        let first = r.configure(
            "default",
            Ok(Some(
                json!({"@type":"setTdlibParameters","api_hash":"synthetic secret"}),
            )),
            now,
        );
        let second = r.configure(
            "default",
            Ok(Some(json!({"@type":"setTdlibParameters"}))),
            now,
        );
        let mut absent = |_: &str| Ok(None);
        let mut execute = |_: &Value| None;
        let stale=r.receive(json!({"@type":"error","@client_id":1,"@extra":first.native[0].query["@extra"],"code":401,"message":"synthetic secret"}),now,&mut absent,&mut execute);
        assert!(stale.replies.is_empty());
        let rejected=r.receive(json!({"@type":"error","@client_id":1,"@extra":second.native[0].query["@extra"],"code":401,"message":"synthetic secret"}),now,&mut absent,&mut execute);
        assert_eq!(rejected.replies.len(), 1);
        assert!(
            !rejected.replies[0]
                .1
                .to_string()
                .contains("synthetic secret")
        );
        let pending = r.configure(
            "default",
            Ok(Some(json!({"@type":"setTdlibParameters"}))),
            now,
        );
        r.expire(now + STARTUP_TIMEOUT);
        let late = r.receive(
            json!({"@type":"ok","@client_id":1,"@extra":pending.native[0].query["@extra"]}),
            now + STARTUP_TIMEOUT,
            &mut absent,
            &mut execute,
        );
        assert!(late.replies.is_empty());
        assert!(!r.metadata["default"].bridge_ready);
    }
}
