//! Pure settings mutation preparation. Persistence must succeed before the
//! owning runtime replaces settings, publishes events or applies side effects.
use crate::{commands::arg_int, preferences};
use serde_json::{Value, json};
pub struct Change {
    pub settings: Value,
    pub apply_globals: bool,
    pub play: Option<i128>,
}
pub fn prepare(command: &str, args: &Value, current: &Value, me: i128) -> Result<Change, String> {
    let mut new = current.clone();
    let mut play = None;
    match command {
        "settings.set" => {
            let incoming = &args["settings"];
            if let Some(fields) = incoming.as_object() {
                for (key, value) in fields {
                    new[key] = value.clone();
                }
            } else {
                new = incoming.clone();
            }
        }
        "settings.playback" => {
            let rate = &args["rate"];
            if !rate.as_f64().is_some_and(|v| [1.0, 1.5, 2.0].contains(&v)) {
                return Err("the playback rate is 1, 1.5 or 2".into());
            }
            new["playbackRate"] = rate.clone();
        }
        "settings.reactions" => {
            if !args["seen"].is_boolean() {
                return Err("seen is true or false".into());
            }
            new["reactionsSeen"] = args["seen"].clone();
        }
        "settings.quiet" => {
            if !args["quiet"].is_boolean() {
                return Err("quiet is true or false".into());
            }
            new["quiet"] = args["quiet"].clone();
        }
        "settings.chatListWidth" => new["chatListWidth"] = args["width"].clone(),
        "settings.autoDownload" => new["autoDownload"] = args["rules"].clone(),
        "settings.emoji" => new["emoji"] = json!({"tone":args["tone"],"recents":args["recents"]}),
        "settings.sounds" => {
            new["sounds"]["style"] = args["style"].clone();
            play = Some(me);
        }
        "sounds.another" => {
            let id = arg_int(args, "userId", Some(1), None, None)?;
            let key = id.to_string();
            let n = new["sounds"]["variants"][&key].as_u64().unwrap_or(0);
            new["sounds"]["variants"][key] = json!(n % 99 + 1);
            play = Some(id);
        }
        _ => return Err("migration command is not implemented".into()),
    }
    let settings = preferences::check(&new, true)?;
    Ok(Change {
        apply_globals: settings["globalShortcuts"] != current["globalShortcuts"],
        settings,
        play,
    })
}
pub fn project(value: &Value) -> Value {
    let current = value
        .get("current")
        .cloned()
        .unwrap_or_else(preferences::empty);
    match prepare(
        value["command"].as_str().unwrap_or(""),
        &value["args"],
        &current,
        value["meId"].as_i64().unwrap_or(0).into(),
    ) {
        Ok(change) => {
            json!({"settings":change.settings,"applyGlobals":change.apply_globals,"play":change.play})
        }
        Err(error) => json!({"error":error}),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation_never_consumes_current_settings() {
        let current = preferences::empty();
        let original = current.clone();
        assert!(prepare("settings.quiet", &json!({"quiet":null}), &current, 7).is_err());
        assert_eq!(current, original);
        let changed = prepare("settings.playback", &json!({"rate":1.5}), &current, 7).unwrap();
        assert_eq!(changed.settings["playbackRate"], 1.5);
        assert!(!changed.apply_globals);
    }
}
