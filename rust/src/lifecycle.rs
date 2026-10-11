use crate::wire::{IPC_PENDING_MAX, TD_PENDING_MAX};
use serde_json::Value;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Token(u64);

pub struct Pending {
    pub peer: u64,
    pub id: Value,
    pub account: String,
    pub generation: u64,
    pub started: Instant,
}

pub struct Requests {
    next: u64,
    ipc: HashMap<Token, Pending>,
    td: HashMap<u64, Token>,
    td_capacity: usize,
}

impl Default for Requests {
    fn default() -> Self {
        Self {
            next: 1,
            ipc: HashMap::new(),
            td: HashMap::new(),
            td_capacity: TD_PENDING_MAX,
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum Admission {
    Duplicate,
    Full,
    Exhausted,
}

impl Requests {
    pub fn duplicate(&self, peer: u64, id: &Value) -> bool {
        self.ipc.values().any(|p| p.peer == peer && p.id == *id)
    }
    pub fn admit(&mut self, request: Pending) -> Result<Token, Admission> {
        let peers = self.ipc.values().filter(|p| p.peer == request.peer);
        if peers.clone().any(|p| p.id == request.id) {
            return Err(Admission::Duplicate);
        }
        if peers.count() >= IPC_PENDING_MAX {
            return Err(Admission::Full);
        }
        let token = Token(self.next);
        self.next = self.next.checked_add(1).ok_or(Admission::Exhausted)?;
        self.ipc.insert(token, request);
        Ok(token)
    }

    pub fn reserve_external(&mut self, count: usize) {
        self.td_capacity = TD_PENDING_MAX.saturating_sub(count);
    }
    pub fn can_track(&self, token: Token, count: usize) -> bool {
        self.ipc.contains_key(&token) && count <= self.td_capacity.saturating_sub(self.td.len())
    }
    pub fn get_mut(&mut self, token: Token) -> Option<&mut Pending> {
        self.ipc.get_mut(&token)
    }
    pub fn get(&self, token: Token) -> Option<&Pending> {
        self.ipc.get(&token)
    }
    pub fn owns(&self, token: Token) -> bool {
        self.ipc.contains_key(&token)
    }
    pub fn track_td(&mut self, extra: u64, token: Token) -> bool {
        if !self.ipc.contains_key(&token)
            || self.td.len() >= self.td_capacity
            || self.td.contains_key(&extra)
        {
            return false;
        }
        self.td.insert(extra, token);
        true
    }

    pub fn current(&self, token: Token, account: &str, generation: u64) -> bool {
        self.ipc
            .get(&token)
            .is_some_and(|p| p.account == account && p.generation == generation)
    }

    pub fn terminal(&mut self, token: Token) -> Option<Pending> {
        self.td.retain(|_, owner| *owner != token);
        self.ipc.remove(&token)
    }

    pub fn td_response(&mut self, extra: u64, account: &str, generation: u64) -> Option<Token> {
        let token = *self.td.get(&extra)?;
        if !self.current(token, account, generation) {
            return None;
        }
        self.td.remove(&extra);
        Some(token)
    }

    pub fn expire(&mut self, now: Instant) -> Vec<Pending> {
        let tokens: Vec<_> = self
            .ipc
            .iter()
            .filter_map(|(t, p)| {
                (now.saturating_duration_since(p.started) >= REQUEST_TIMEOUT).then_some(*t)
            })
            .collect();
        tokens
            .into_iter()
            .filter_map(|t| self.terminal(t))
            .collect()
    }

    pub fn retire_peer(&mut self, peer: u64) -> Vec<Pending> {
        let tokens: Vec<_> = self
            .ipc
            .iter()
            .filter_map(|(t, p)| (p.peer == peer).then_some(*t))
            .collect();
        tokens
            .into_iter()
            .filter_map(|t| self.terminal(t))
            .collect()
    }

    pub fn retire_account(&mut self, account: &str) -> Vec<Pending> {
        let tokens: Vec<_> = self
            .ipc
            .iter()
            .filter_map(|(t, p)| (p.account == account).then_some(*t))
            .collect();
        tokens
            .into_iter()
            .filter_map(|t| self.terminal(t))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pending(now: Instant) -> Pending {
        Pending {
            peer: 1,
            id: json!(7),
            account: "default".into(),
            generation: 1,
            started: now,
        }
    }

    #[test]
    fn expired_callback_cannot_satisfy_reused_id() {
        let now = Instant::now();
        let mut requests = Requests::default();
        let old = requests.admit(pending(now)).unwrap();
        assert!(requests.track_td(12, old));
        assert_eq!(requests.expire(now + REQUEST_TIMEOUT).len(), 1);
        let new = requests.admit(pending(now + REQUEST_TIMEOUT)).unwrap();
        assert_ne!(old, new);
        assert!(requests.td_response(12, "default", 1).is_none());
        assert!(requests.terminal(old).is_none());
        assert!(requests.current(new, "default", 1));
        assert!(requests.terminal(new).is_some());
        assert!(requests.terminal(new).is_none());
    }

    #[test]
    fn wrong_account_does_not_consume_original_callback() {
        let mut requests = Requests::default();
        let token = requests.admit(pending(Instant::now())).unwrap();
        assert!(requests.track_td(1, token));
        assert!(requests.td_response(1, "other", 1).is_none());
        assert!(requests.td_response(1, "default", 2).is_none());
        assert_eq!(requests.td_response(1, "default", 1), Some(token));
        assert_eq!(
            requests.admit(pending(Instant::now())).err(),
            Some(Admission::Duplicate)
        );
        assert_eq!(requests.retire_peer(1).len(), 1);
    }
}
