//! Existing local settings schema and global key normalization, without I/O.
use crate::wire::is_integer;
use serde_json::{Value, json};
const GLOBALS: [&str; 3] = ["global.quickReply", "global.panel", "global.openWindow"];
pub fn empty() -> Value {
    json!({"shortcuts":{},"globalShortcuts":{},"playbackRate":1,"autoDownload":{"photos":true,"gifs":true,"videos":0,"files":0},"reactionsSeen":true,"emoji":{"tone":0,"recents":{}},"sounds":{"style":"drop","variants":{}},"quiet":false,"showStories":true})
}
fn modifier(s: &str) -> Option<&'static str> {
    match s {
        "SUPER" | "META" | "WIN" => Some("SUPER"),
        "CTRL" | "CONTROL" => Some("CTRL"),
        "ALT" => Some("ALT"),
        "SHIFT" => Some("SHIFT"),
        _ => None,
    }
}
pub fn normalize_combo(value: &Value) -> Option<String> {
    let s = value
        .as_str()
        .filter(|s| !s.is_empty() && s.chars().count() <= 60)?;
    let tokens: Vec<_> = s.split('+').map(str::trim).collect();
    if tokens.len() < 2 || tokens.iter().any(|s| s.is_empty()) {
        return None;
    }
    let mut mods = Vec::new();
    for m in &tokens[..tokens.len() - 1] {
        let m = modifier(&m.to_uppercase())?;
        if mods.contains(&m) {
            return None;
        }
        mods.push(m);
    }
    if !mods.iter().any(|m| matches!(*m, "SUPER" | "CTRL" | "ALT")) {
        return None;
    }
    let key = tokens[tokens.len() - 1].to_uppercase();
    let key = match key.as_str() {
        "ESC" => "ESCAPE",
        "ENTER" => "RETURN",
        "DEL" => "DELETE",
        "PGUP" | "PAGEUP" => "PAGE_UP",
        "PGDOWN" | "PAGEDOWN" => "PAGE_DOWN",
        "," => "COMMA",
        "." => "PERIOD",
        "/" => "SLASH",
        ";" => "SEMICOLON",
        "'" => "APOSTROPHE",
        "-" => "MINUS",
        "=" => "EQUAL",
        "[" => "BRACKETLEFT",
        "]" => "BRACKETRIGHT",
        "\\" => "BACKSLASH",
        "`" => "GRAVE",
        s => s,
    };
    let known = [
        "SPACE",
        "RETURN",
        "TAB",
        "ESCAPE",
        "BACKSPACE",
        "DELETE",
        "HOME",
        "END",
        "UP",
        "DOWN",
        "LEFT",
        "RIGHT",
        "PAGE_UP",
        "PAGE_DOWN",
        "COMMA",
        "PERIOD",
        "SLASH",
        "SEMICOLON",
        "APOSTROPHE",
        "MINUS",
        "EQUAL",
        "BRACKETLEFT",
        "BRACKETRIGHT",
        "BACKSLASH",
        "GRAVE",
    ];
    let function = key
        .strip_prefix('F')
        .filter(|s| !s.starts_with('0'))
        .and_then(|s| s.parse::<u8>().ok())
        .is_some_and(|n| (1..=24).contains(&n));
    if !(key.len() == 1
        && key
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        || function
        || known.contains(&key))
    {
        return None;
    }
    let mut out: Vec<_> = ["SUPER", "CTRL", "ALT", "SHIFT"]
        .into_iter()
        .filter(|m| mods.contains(m))
        .collect();
    out.push(key);
    Some(out.join(" + "))
}
fn valid_action(action: &str) -> bool {
    let Some((a, b)) = action.split_once('.') else {
        return false;
    };
    let part = |s: &str, max: usize| {
        !s.is_empty()
            && s.len() <= max
            && s.as_bytes()[0].is_ascii_lowercase()
            && s.bytes().all(|c| c.is_ascii_alphabetic())
    };
    part(a, 21) && part(b, 41) && a != "global"
}
// Compatibility representation is part of validation errors. Values are bounded
// JSON; this implements its string/number/container spellings, not execution.
fn py_string(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::from(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c)
            }
            c if c < ' ' || c == '\u{7f}' => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}
fn py(value: &Value, repr: bool) -> String {
    match value {
        Value::Null => "None".into(),
        Value::Bool(v) => if *v { "True" } else { "False" }.into(),
        Value::String(s) => {
            if repr {
                py_string(s)
            } else {
                s.clone()
            }
        }
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(|v| py(v, true)).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(m) => format!(
            "{{{}}}",
            m.iter()
                .map(|(k, v)| format!("{}: {}", py_string(k), py(v, true)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        v => v.to_string(),
    }
}
fn repr(value: &Value, cap: usize) -> String {
    py_string(&py(value, false).chars().take(cap).collect::<String>())
}
fn bad(strict: bool, message: String) -> Result<(), String> {
    if strict { Err(message) } else { Ok(()) }
}
fn positive(value: &Value) -> bool {
    value.as_f64().is_some_and(|v| v > 0.0 && v < 1e15)
}
pub fn check(value: &Value, strict: bool) -> Result<Value, String> {
    let mut out = empty();
    let Some(value) = value.as_object() else {
        bad(strict, "settings must be an object".into())?;
        return Ok(out);
    };
    let shortcuts = value.get("shortcuts").cloned().unwrap_or(json!({}));
    let shortcuts = match shortcuts.as_object().filter(|m| m.len() <= 200) {
        Some(m) => Some(m),
        None => {
            bad(
                strict,
                "shortcuts must be an object of at most 200 actions".into(),
            )?;
            None
        }
    };
    if let Some(shortcuts) = shortcuts {
        for (action, keys) in shortcuts {
            if !valid_action(action) {
                bad(
                    strict,
                    format!("{} is not an action", repr(&json!(action), 60)),
                )?;
                continue;
            }
            let Some(keys) = keys.as_array().filter(|a| a.len() <= 6) else {
                bad(strict, format!("{action} takes a list of at most 6 keys"))?;
                continue;
            };
            let mut clean = Vec::new();
            let mut good = true;
            for key in keys {
                if !key.as_str().is_some_and(|s| {
                    !s.is_empty() && s.len() <= 40 && s.bytes().all(|c| (0x21..=0x7e).contains(&c))
                }) {
                    bad(strict, format!("{} is not a key sequence", repr(key, 60)))?;
                    good = false;
                    break;
                }
                if !clean.contains(key) {
                    clean.push(key.clone());
                }
            }
            if good {
                out["shortcuts"][action] = json!(clean);
            }
        }
    }
    let globals = value.get("globalShortcuts").cloned().unwrap_or(json!({}));
    let globals = match globals.as_object() {
        Some(m) => Some(m),
        None => {
            bad(strict, "globalShortcuts must be an object".into())?;
            None
        }
    };
    let mut seen = Vec::new();
    if let Some(globals) = globals {
        for (action, combo) in globals {
            if !GLOBALS.contains(&action.as_str()) {
                bad(
                    strict,
                    format!("{} is not a global shortcut", repr(&json!(action), 60)),
                )?;
                continue;
            }
            if combo.is_null() || combo == "" {
                continue;
            }
            let Some(normal) = normalize_combo(combo) else {
                bad(
                    strict,
                    format!(
                        "{} is not a key combination with a modifier",
                        repr(combo, 60)
                    ),
                )?;
                continue;
            };
            if seen.contains(&normal) {
                bad(strict, format!("{normal} is used twice"))?;
                continue;
            }
            seen.push(normal.clone());
            out["globalShortcuts"][action] = json!(normal);
        }
    }
    let rate = value.get("playbackRate").unwrap_or(&json!(1)).clone();
    if !rate.as_f64().is_some_and(|v| [1.0, 1.5, 2.0].contains(&v)) {
        bad(strict, "playbackRate is 1, 1.5 or 2".into())?;
    } else {
        out["playbackRate"] = rate;
    }
    let downloads = value.get("autoDownload").cloned().unwrap_or(json!({}));
    if !downloads.is_object() {
        bad(strict, "autoDownload must be an object".into())?;
    } else {
        for key in ["photos", "gifs", "videos", "files"] {
            if let Some(v) = downloads.get(key) {
                let boolean = matches!(key, "photos" | "gifs");
                if if boolean {
                    !v.is_boolean()
                } else {
                    !v.as_f64().is_some_and(|v| [0.0, 10.0, 50.0].contains(&v))
                } {
                    bad(
                        strict,
                        format!(
                            "autoDownload.{key} is {}",
                            if boolean {
                                "true or false"
                            } else {
                                "0, 10 or 50"
                            }
                        ),
                    )?;
                } else {
                    out["autoDownload"][key] = v.clone();
                }
            }
        }
    }
    for (key, default) in [
        ("showStories", true),
        ("reactionsSeen", true),
        ("quiet", false),
    ] {
        let v = value.get(key).cloned().unwrap_or(json!(default));
        if !v.is_boolean() {
            bad(strict, format!("{key} is true or false"))?;
        } else {
            out[key] = v;
        }
    }
    if let Some(width) = value.get("chatListWidth") {
        if !is_integer(width) || !width.as_i64().is_some_and(|v| (72..=1200).contains(&v)) {
            bad(
                strict,
                "chatListWidth is a whole number from 72 to 1200".into(),
            )?;
        } else {
            out["chatListWidth"] = width.clone();
        }
    }
    let emoji = value.get("emoji").cloned().unwrap_or(json!({}));
    if !emoji.is_object() {
        bad(strict, "emoji must be an object".into())?;
    }
    let tone = emoji.get("tone").cloned().unwrap_or(json!(0));
    if !is_integer(&tone) || !tone.as_i64().is_some_and(|v| (0..=5).contains(&v)) {
        bad(strict, "emoji.tone is a whole number from 0 to 5".into())?;
    } else {
        out["emoji"]["tone"] = tone;
    }
    let recents = emoji.get("recents").cloned().unwrap_or(json!({}));
    if let Some(recents) = recents.as_object().filter(|m| m.len() <= 80) {
        for (key, entry) in recents {
            let (kind, body) = key.split_once(':').unwrap_or((key, ""));
            if !["emoji", "symbols", "kaomoji"].contains(&kind)
                || body.is_empty()
                || body.chars().count() > 80
                || body.chars().any(|c| c < ' ')
                || !entry.is_object()
                || !positive(&entry["c"])
                || !positive(&entry["t"])
            {
                bad(
                    strict,
                    format!("{} is not a recent emoji", repr(&json!(key), 40)),
                )?;
                continue;
            }
            out["emoji"]["recents"][key] =
                json!({"c":entry["c"].as_f64(),"t":entry["t"].as_f64().unwrap_or(0.0) as u64});
        }
    } else {
        bad(
            strict,
            "emoji.recents is an object of at most 80 entries".into(),
        )?;
    }
    let sounds = value.get("sounds").cloned().unwrap_or(json!({}));
    if !sounds.is_object() {
        bad(strict, "sounds must be an object".into())?;
    }
    let style = sounds.get("style").cloned().unwrap_or(json!("drop"));
    if !style
        .as_str()
        .is_some_and(|s| ["pop", "drop", "knock", "off"].contains(&s))
    {
        bad(strict, "sounds.style is pop, drop, knock or off".into())?;
    } else {
        out["sounds"]["style"] = style;
    }
    let variants = sounds.get("variants").cloned().unwrap_or(json!({}));
    if let Some(variants) = variants.as_object().filter(|m| m.len() <= 500) {
        for (key, n) in variants {
            let digits = key.strip_prefix('-').unwrap_or(key);
            if digits.is_empty()
                || digits.len() > 19
                || !digits.bytes().all(|c| c.is_ascii_digit())
                || !is_integer(n)
                || !n.as_i64().is_some_and(|v| (1..100).contains(&v))
            {
                bad(
                    strict,
                    format!(
                        "{} is not a person with another sound",
                        repr(&json!(key), 24)
                    ),
                )?;
                continue;
            }
            out["sounds"]["variants"][key] = n.clone();
        }
    } else {
        bad(
            strict,
            "sounds.variants is an object of at most 500 people".into(),
        )?;
    }
    Ok(out)
}

pub fn project(value: &Value) -> Value {
    match value["operation"].as_str() {
        Some("combo") => json!(normalize_combo(&value["value"])),
        _ => match check(&value["value"], value["strict"] != false) {
            Ok(result) => json!({"result":result}),
            Err(error) => json!({"error":error}),
        },
    }
}
