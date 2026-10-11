//! Notification text/identity owner. Desktop transport receives escaped text
//! over the session bus, never in a subprocess argument list.
use indexmap::IndexMap;
use serde_json::{Value, json};
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Key {
    pub account: String,
    pub chat: i128,
}
pub const ACTIONS: [&str; 10] = [
    "default",
    "Open",
    "reply",
    "Reply",
    "read",
    "Mark as read",
    "mute",
    "Mute for an hour",
    "react",
    "👍",
];
pub fn clean(value: &str, limit: usize) -> String {
    let replaced: String = value
        .chars()
        .map(|c| {
            if c <= '\u{1f}'
                || ('\u{7f}'..='\u{9f}').contains(&c)
                || matches!(c, '\u{2028}' | '\u{2029}')
            {
                ' '
            } else {
                c
            }
        })
        .collect();
    let text = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > limit {
        let mut text = text
            .chars()
            .take(limit.saturating_sub(1))
            .collect::<String>();
        text.push('…');
        text
    } else {
        text
    }
}
pub fn body(value: &str) -> String {
    clean(value, 300)
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
pub struct Notification {
    pub replaces: u32,
    pub title: String,
    pub body: String,
    pub actions: Vec<String>,
    pub image: String,
}
#[derive(Default)]
pub struct Notifier {
    by_chat: IndexMap<Key, u32>,
    by_id: IndexMap<u32, Key>,
    pub focused: Option<Key>,
}
impl Notifier {
    pub fn prepare(&self, key: &Key, title: &str, text: &str, image: &str) -> Option<Notification> {
        if key.chat == 0 || self.focused.as_ref() == Some(key) {
            return None;
        }
        let title = clean(title, 120);
        Some(Notification {
            replaces: *self.by_chat.get(key).unwrap_or(&0),
            title: if title.is_empty() {
                "Telebar".into()
            } else {
                title
            },
            body: body(text),
            actions: ACTIONS.iter().map(|s| (*s).into()).collect(),
            image: image.into(),
        })
    }
    pub fn shown(&mut self, key: Key, id: u32) -> bool {
        if id == 0 {
            return false;
        }
        if let Some(replaces) = self.by_chat.get(&key)
            && *replaces != id
        {
            self.by_id.shift_remove(replaces);
        }
        if self.by_id.len() >= 512 && !self.by_id.contains_key(&id) {
            self.by_chat.clear();
            self.by_id.clear();
        }
        self.by_chat.insert(key.clone(), id);
        self.by_id.insert(id, key);
        true
    }
    pub fn withdraw(&mut self, key: &Key) -> Option<u32> {
        let id = self.by_chat.shift_remove(key)?;
        self.by_id.shift_remove(&id);
        Some(id)
    }
    pub fn action(&self, id: u32, action: &str) -> Option<Key> {
        ACTIONS
            .iter()
            .step_by(2)
            .any(|a| *a == action)
            .then(|| self.by_id.get(&id).cloned())
            .flatten()
    }
    pub fn closed(&mut self, id: u32) {
        if let Some(key) = self.by_id.shift_remove(&id)
            && self.by_chat.get(&key) == Some(&id)
        {
            self.by_chat.shift_remove(&key);
        }
    }
}
pub fn project(value: &Value) -> Value {
    let text = value["text"].as_str().unwrap_or("");
    match value["kind"].as_str() {
        Some("body") => json!(body(text)),
        _ => json!(clean(
            text,
            value["limit"].as_u64().unwrap_or(120).min(16384) as usize
        )),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escaped_previews_and_stale_closed_ids_preserve_replacements() {
        assert_eq!(
            body("<img src='x'> &\ntext"),
            "&lt;img src='x'&gt; &amp; text"
        );
        let mut n = Notifier::default();
        let key = Key {
            account: "default".into(),
            chat: 42,
        };
        assert!(n.shown(key.clone(), 1));
        n.shown(key.clone(), 2);
        n.closed(1);
        assert_eq!(n.prepare(&key, "", "hello", "").unwrap().replaces, 2);
        assert!(n.action(2, "unrecognized").is_none());
        assert_eq!(n.action(2, "reply"), Some(key.clone()));
        n.focused = Some(key.clone());
        assert!(n.prepare(&key, "", "hello", "").is_none());
        assert_eq!(n.withdraw(&key), Some(2));
    }
}
