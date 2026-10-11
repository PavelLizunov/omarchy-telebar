//! Persisted settings command owner. Driver supplies the admitted request;
//! persistence precedes in-memory replacement and all observable side effects.
use crate::{
    commands::{Action, Result},
    preferences, preferences_commands,
    state::State,
    storage::Storage,
};
use serde_json::{Value, json};
use std::io;
pub struct Settings {
    pub value: Value,
    pub global_status: Value,
    pub globals_pending: bool,
    pub sounds_pending: Vec<i128>,
    pub events: Vec<Value>,
}
impl Settings {
    pub fn load(home: &Storage) -> io::Result<Self> {
        let raw = home
            .read(".config/omagram/settings.json", 64 * 1024)
            .ok()
            .flatten();
        let value = raw
            .and_then(|b| crate::wire::bounded_json(&b, 6, 5000, 256).ok())
            .unwrap_or_else(preferences::empty);
        let value = preferences::check(&value, false).map_err(io::Error::other)?;
        Ok(Self {
            value,
            global_status: json!({"global.quickReply":"off","global.panel":"off","global.openWindow":"off"}),
            globals_pending: false,
            sounds_pending: Vec::new(),
            events: Vec::new(),
        })
    }
    pub fn view(&self) -> Value {
        json!({"settings":self.value,"globalStatus":self.global_status})
    }
    pub fn command(
        &mut self,
        home: &Storage,
        command: &str,
        args: &Value,
        state: &State,
    ) -> Option<Result<Action>> {
        if !matches!(
            command,
            "settings.set"
                | "settings.playback"
                | "settings.chatListWidth"
                | "settings.reactions"
                | "settings.emoji"
                | "settings.quiet"
                | "settings.sounds"
                | "settings.autoDownload"
                | "sounds.another"
        ) {
            return None;
        }
        Some((|| {
            let change = preferences_commands::prepare(command, args, &self.value, state.me_id)?;
            home.write(
                ".config/omagram/settings.json",
                change.settings.to_string().as_bytes(),
                0o600,
            )
            .map_err(|_| "the settings could not be saved")?;
            self.value = change.settings;
            self.globals_pending |= change.apply_globals;
            if let Some(seed) = change.play {
                if self.sounds_pending.len() >= 2 {
                    self.sounds_pending.remove(0);
                }
                self.sounds_pending.push(seed);
            }
            let result = self.view();
            let mut event = result.clone();
            event["event"] = json!("settings");
            self.events.push(event);
            Ok(Action::Reply(result))
        })())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };
    #[test]
    fn failed_save_keeps_original_settings_and_emits_nothing() {
        let root = std::env::temp_dir().join(format!("telebar-settings-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let home = Storage::open(&root).unwrap();
        let mut settings = Settings::load(&home).unwrap();
        let state = State::default();
        let result = settings
            .command(&home, "settings.playback", &json!({"rate":1.5}), &state)
            .unwrap()
            .unwrap();
        assert!(matches!(result, Action::Reply(_)));
        assert_eq!(Settings::load(&home).unwrap().value["playbackRate"], 1.5);
        settings.events.clear();
        fs::remove_file(root.join(".config/omagram/settings.json")).unwrap();
        symlink("foreign", root.join(".config/omagram/settings.json")).unwrap();
        let original = settings.value.clone();
        let result = settings
            .command(&home, "settings.quiet", &json!({"quiet":true}), &state)
            .unwrap();
        assert_eq!(result.unwrap_err(), "the settings could not be saved");
        assert_eq!(settings.value, original);
        assert!(settings.events.is_empty() && settings.sounds_pending.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
