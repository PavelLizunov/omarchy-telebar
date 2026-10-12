//! Ordered notification policy over already projected account state.
use crate::{
    authorization::Runtime,
    notifications::{Key, Notifier},
    projection::{integer, number},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct Policy {
    pub messages: HashMap<Key, i128>,
    pub pending: HashMap<Key, (u64, u64, i128, i128)>,
    pub serial: u64,
    pub shown_generations: HashMap<Key, u64>,
    last_sound: Option<Instant>,
    people: HashMap<i128, Instant>,
}
pub struct Show {
    pub key: Key,
    pub notification: crate::notifications::Notification,
    pub message: i128,
    pub seed: i128,
    pub generation: u64,
}
pub enum Decision {
    None,
    Close(Key),
    Show(Show),
}
impl Policy {
    pub fn group(
        &mut self,
        runtime: &Runtime,
        notifier: &Notifier,
        settings: &Value,
        account: &str,
        event: &Value,
    ) -> Decision {
        let chat = integer(&event["chat_id"], 0);
        let key = Key {
            account: account.into(),
            chat,
        };
        if chat == 0 {
            return Decision::None;
        }
        if settings["quiet"] == true {
            return Decision::Close(key);
        }
        let loud = event["added_notifications"]
            .as_array()
            .into_iter()
            .flatten()
            .take(100)
            .filter(|n| {
                n["@type"] == "notification"
                    && n["is_silent"] != true
                    && n["type"]["@type"] == "notificationTypeNewMessage"
            })
            .max_by_key(|n| integer(&n["id"], 0));
        let Some(loud) = loud else {
            return if integer(&event["total_count"], 0) == 0 {
                Decision::Close(key)
            } else {
                Decision::None
            };
        };
        let Some(owner) = runtime.engine.accounts.get(account) else {
            return Decision::None;
        };
        if notifier.focused.as_ref() == Some(&key) {
            return Decision::None;
        }
        let chat = owner.state.chats.get(&chat);
        let message = owner.state.message(&loud["type"]["message"]);
        let show_preview = loud["type"]["show_preview"] == true;
        let mut title = chat
            .and_then(|c| c["title"].as_str())
            .unwrap_or("")
            .to_owned();
        if runtime.engine.accounts.len() > 1 {
            let name = runtime
                .metadata
                .get(account)
                .map(|m| m.name.as_str())
                .unwrap_or(account);
            title = if title.is_empty() {
                name.into()
            } else {
                format!("{name} • {title}")
            };
        }
        let total = integer(&event["total_count"], 0);
        if total > 1 {
            title = format!("{title} ({total})");
        }
        let mut body = if message.is_null() || !show_preview {
            "New message".into()
        } else {
            crate::content_projection::preview_text(&message["content"])
                .as_str()
                .unwrap_or("")
                .into()
        };
        if show_preview
            && !message.is_null()
            && chat.is_some_and(|c| c["kind"] != "private")
            && let Some(sender) = message["senderName"].as_str().filter(|s| !s.is_empty())
        {
            body = format!("{sender}: {body}");
        }
        let Some(notification) = notifier.prepare(&key, &title, &body, "") else {
            return Decision::None;
        };
        let mid = integer(&message["id"], 0);
        let seed = if message["sender"]["type"] == "user" {
            integer(&message["sender"]["id"], 0)
        } else {
            key.chat
        };
        if mid != 0 {
            if self.messages.len() >= 512 && !self.messages.contains_key(&key) {
                self.messages.clear();
            }
            self.messages.insert(key.clone(), mid);
        }
        Decision::Show(Show {
            key,
            notification,
            message: mid,
            seed,
            generation: owner.generation,
        })
    }
    pub fn sound(
        &mut self,
        seed: i128,
        settings: &Value,
        home: Option<&crate::storage::Storage>,
        now: Instant,
    ) -> bool {
        if seed == 0
            || settings["quiet"] == true
            || self.last_sound.is_some_and(|last| {
                now.saturating_duration_since(last) < Duration::from_millis(1200)
            })
            || self
                .people
                .get(&seed)
                .is_some_and(|last| now.saturating_duration_since(*last) < Duration::from_secs(4))
        {
            return false;
        }
        if home
            .and_then(|s| {
                s.read(".local/state/omarchy/notifications.json", 64 * 1024)
                    .ok()
                    .flatten()
            })
            .and_then(|b| crate::wire::bounded_json(&b, 8, 1024, 16384).ok())
            .is_some_and(|v| v["dnd"] == true)
        {
            return false;
        }
        if self.people.len() >= 512 {
            self.people.clear();
        }
        self.last_sound = Some(now);
        self.people.insert(seed, now);
        true
    }
    pub fn button(&self, key: &Key, action: &str, runtime: &Runtime) -> Option<Value> {
        let owner = runtime.engine.accounts.get(&key.account)?;
        let mid = *self.messages.get(key).unwrap_or(&0);
        match action{
  "read" if mid!=0=>Some(json!({"@type":"viewMessages","chat_id":number(key.chat),"message_ids":[number(mid)],"source":{"@type":"messageSourceNotification"},"force_read":true})),
  "mute"=>owner.state.chats.get(&key.chat).map(|chat|{let mut settings=chat["notification"].clone();settings["use_default_mute_for"]=json!(false);settings["mute_for"]=json!(3600);json!({"@type":"setChatNotificationSettings","chat_id":number(key.chat),"notification_settings":settings})}),
  "react" if mid!=0=>Some(json!({"@type":"addMessageReaction","chat_id":number(key.chat),"message_id":number(mid),"reaction_type":{"@type":"reactionTypeEmoji","emoji":"👍"},"is_big":false,"update_recent_reactions":true})),_=>None}
    }
}
/// Tiny notification picture cache, writes only admitted JPEG bytes.
pub fn picture(
    home: &crate::storage::Storage,
    root: &std::path::Path,
    roots: &[String],
    candidates: &[Value],
) -> Option<PathBuf> {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    for candidate in candidates {
        if let Some(data) = candidate["data"].as_str().filter(|s| s.len() <= 128 * 1024)
            && let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(data)
            && raw.starts_with(&[255, 216])
        {
            let name = format!(
                ".cache/omagram/notify/{}.jpg",
                &format!("{:x}", Sha256::digest(&raw))[..32]
            );
            let entries = home.files(".cache/omagram/notify", 250).ok()?;
            if entries.len() >= 200 {
                for (entry, _) in entries {
                    home.remove_file(&format!(".cache/omagram/notify/{entry}"))
                        .ok()?;
                }
            }
            home.write(&name, &raw, 0o600).ok()?;
            return Some(root.join(name));
        }
        let path = crate::projection::local_path(&candidate["path"], roots);
        if let Some(owner) = roots
            .iter()
            .filter(|r| path.starts_with(&format!("{}/", r.trim_end_matches('/'))))
            .max_by_key(|r| r.len())
            && crate::storage::Storage::open(std::path::Path::new(owner))
                .and_then(|s| s.open_read(&path[owner.trim_end_matches('/').len() + 1..]))
                .is_ok()
        {
            return Some(path.into());
        }
    }
    None
}
