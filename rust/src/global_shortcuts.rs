//! Compatible Hyprland binding ownership. Fixed commands, bounded helpers and
//! explicit session context; no configuration edits or unrelated unbinding.
use crate::{preferences::normalize_combo, process};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
const ACTIONS: [(&str, &str); 3] = [
    ("global.quickReply", "Quick reply"),
    ("global.panel", "Bar panel"),
    ("global.openWindow", "Open Telebar"),
];
const PREFIX: &str = "Omagram: ";
const MODS: [(&str, i64); 4] = [("SUPER", 64), ("CTRL", 4), ("ALT", 8), ("SHIFT", 1)];
#[derive(Clone, Debug)]
pub struct Context {
    pub plugin: PathBuf,
    pub environment: BTreeMap<String, String>,
}
#[derive(Debug)]
pub struct Job {
    pub context: Context,
    pub settings: Value,
}
fn label(action: &str) -> Option<String> {
    ACTIONS
        .iter()
        .find(|(key, _)| *key == action)
        .map(|(_, name)| format!("{PREFIX}{name}"))
}
pub fn lua_string(value: &str) -> String {
    let mut out = String::from("\"");
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b" +_-./:{}'".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("\\{byte:03}"));
        }
    }
    out.push('"');
    out
}
fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&b))
    {
        value.into()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}
fn part(combo: &str) -> (i64, String) {
    let tokens: Vec<_> = combo.split('+').map(str::trim).collect();
    let mask = MODS
        .iter()
        .filter(|(name, _)| tokens[..tokens.len() - 1].contains(name))
        .map(|(_, bit)| bit)
        .sum();
    (mask, tokens[tokens.len() - 1].to_lowercase())
}
pub fn bind_statement(combo: &str, action: &str, context: &Context) -> Option<String> {
    let argv = match action {
        "global.quickReply" => vec![
            "/usr/bin/omarchy-shell".into(),
            "shell".into(),
            "toggle".into(),
            "io.github.pavellizunov.telebar".into(),
            "{}".into(),
        ],
        "global.panel" => vec![
            "/usr/bin/omarchy-shell".into(),
            "io.github.pavellizunov.telebar.panel".into(),
            "toggle".into(),
        ],
        "global.openWindow" => vec![context.plugin.join("bin/omagram").to_str()?.to_owned()],
        _ => return None,
    };
    let command = argv
        .iter()
        .map(|s| shell_quote(s))
        .collect::<Vec<_>>()
        .join(" ");
    Some(format!(
        "hl.bind({}, hl.dsp.exec_cmd({}), {{ description = {} }})",
        lua_string(combo),
        lua_string(&command),
        lua_string(&label(action)?)
    ))
}
pub(crate) fn statuses(desired: &Value, state: &str) -> Value {
    let mut out = json!({});
    for (action, _) in ACTIONS {
        out[action] = json!(if desired.get(action).is_some() {
            state
        } else {
            "off"
        });
    }
    out
}
/// Pure policy over a captured binds response. The injected evaluator is the
/// only effect seam; tests never discover a compositor or invoke hyprctl.
pub fn apply_with(
    desired: &Value,
    binds: &Value,
    context: &Context,
    eval: &mut dyn FnMut(&str) -> bool,
) -> Value {
    let mut ours = indexmap::IndexMap::new();
    let mut theirs = BTreeSet::new();
    for bind in binds.as_array().into_iter().flatten() {
        let (Some(key), Some(mask)) = (bind["key"].as_str(), bind["modmask"].as_i64()) else {
            continue;
        };
        let key = (mask, key.to_lowercase());
        let desc = bind["description"].as_str().unwrap_or("");
        if desc.starts_with(PREFIX) {
            ours.insert(key, desc.to_owned());
        } else {
            theirs.insert(key);
        }
    }
    // Settings insertion order is observable in helper invocation order.
    let mut wanted = indexmap::IndexMap::new();
    for (action, value) in desired.as_object().into_iter().flatten() {
        if let (Some(combo), Some(desc)) = (normalize_combo(value), label(action))
            && value.as_str() == Some(combo.as_str())
        {
            wanted.insert(part(&combo), (combo, action.clone(), desc));
        }
    }
    let mut status = statuses(&json!({}), "off");
    for (key, desc) in &mut ours {
        if wanted.get(key).is_none_or(|(_, _, label)| label != desc) && !theirs.contains(key) {
            let mut tokens = MODS
                .iter()
                .filter(|(_, bit)| key.0 & bit != 0)
                .map(|(name, _)| (*name).to_owned())
                .collect::<Vec<_>>();
            tokens.push(key.1.to_uppercase());
            eval(&format!("hl.unbind({})", lua_string(&tokens.join(" + "))));
            desc.clear();
        }
    }
    for (key, (combo, action, desc)) in wanted {
        status[&action] = json!(if theirs.contains(&key) {
            "taken"
        } else if ours.get(&key) == Some(&desc)
            || bind_statement(&combo, &action, context).is_some_and(|statement| eval(&statement))
        {
            "active"
        } else {
            "failed"
        });
    }
    status
}
impl Job {
    pub fn run(self, cancel: &AtomicBool) -> crate::commands::Result<crate::commands::Action> {
        if cancel.load(Ordering::Relaxed) {
            return Err("the connection was closed".into());
        }
        let desired = &self.settings["globalShortcuts"];
        let status = if !self
            .context
            .environment
            .contains_key("HYPRLAND_INSTANCE_SIGNATURE")
        {
            statuses(desired, "unavailable")
        } else {
            let deadline = Instant::now() + Duration::from_secs(32);
            let binds = process::run_cancel(
                "/usr/bin/hyprctl",
                &["binds", "-j"],
                &self.context.environment,
                &[],
                Duration::from_secs(4),
                4 * 1024 * 1024,
                cancel,
            )
            .ok()
            .filter(|o| o.ok())
            .and_then(|o| crate::wire::bounded_json(&o.stdout, 6, 200000, 4096).ok());
            if let Some(binds) = binds {
                apply_with(desired, &binds, &self.context, &mut |statement| {
                    let budget = deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_secs(4));
                    if budget.is_zero() {
                        return false;
                    }
                    process::run_cancel(
                        "/usr/bin/hyprctl",
                        &["eval", statement],
                        &self.context.environment,
                        &[],
                        budget,
                        4 * 1024 * 1024,
                        cancel,
                    )
                    .is_ok_and(|o| {
                        o.ok()
                            && !String::from_utf8_lossy(&o.stdout)
                                .trim()
                                .starts_with("error")
                    })
                })
            } else {
                statuses(desired, "failed")
            }
        };
        if cancel.load(Ordering::Relaxed) {
            return Err("the connection was closed".into());
        }
        Ok(crate::commands::Action::Reply(
            json!({"settings":self.settings,"globalStatus":status}),
        ))
    }
}
