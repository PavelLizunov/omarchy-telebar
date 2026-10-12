use serde_json::{Value, json};

pub fn integer(value: &Value, default: i128) -> i128 {
    match value {
        Value::Number(n) if crate::wire::is_integer(value) => {
            n.to_string().parse().unwrap_or(default)
        }
        Value::String(s) if !s.is_empty() && s.len() <= 20 => {
            let digits = s.strip_prefix('-').unwrap_or(s);
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
                s.parse().unwrap_or(default)
            } else {
                default
            }
        }
        _ => default,
    }
}

pub(crate) fn number(n: i128) -> Value {
    // i128 is serialized through the arbitrary-precision parser so neither
    // TDLib string int64 nor malformed-but-characterized 20-digit input wraps.
    serde_json::from_str(&n.to_string()).unwrap_or(Value::Null)
}

pub fn text(value: &Value, limit: usize) -> String {
    value
        .as_str()
        .map(|s| s.chars().take(limit).collect())
        .unwrap_or_default()
}

pub(crate) fn tag(value: &Value) -> &str {
    value["@type"].as_str().unwrap_or("")
}
pub(crate) fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.get(key).unwrap_or(&Value::Null)
}
pub(crate) fn typed<'a>(value: &'a Value, name: &str) -> &'a Value {
    if value.is_object() && tag(value) == name {
        value
    } else {
        &Value::Null
    }
}
pub(crate) fn array(value: &Value, limit: usize) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten().take(limit)
}
fn nonnegative(value: &Value) -> Value {
    number(integer(value, 0).max(0))
}
pub(crate) fn yes(value: &Value) -> bool {
    value == &Value::Bool(true)
}

pub fn formatted(value: &Value) -> Value {
    let body = text(&value["text"], 16384);
    let size = i128::try_from(body.encode_utf16().count()).unwrap_or(i128::MAX);
    let mut entities = Vec::new();
    for entity in array(&value["entities"], 512) {
        let kind = &entity["type"];
        let name = match tag(kind) {
            "textEntityTypeBold" => "bold",
            "textEntityTypeItalic" => "italic",
            "textEntityTypeUnderline" => "underline",
            "textEntityTypeStrikethrough" => "strikethrough",
            "textEntityTypeSpoiler" => "spoiler",
            "textEntityTypeCode" => "code",
            "textEntityTypePre" => "pre",
            "textEntityTypePreCode" => "preCode",
            "textEntityTypeBlockQuote" | "textEntityTypeExpandableBlockQuote" => "blockQuote",
            "textEntityTypeTextUrl" => "textUrl",
            "textEntityTypeUrl" => "url",
            "textEntityTypeMention" => "mention",
            "textEntityTypeMentionName" => "mentionName",
            "textEntityTypeHashtag" => "hashtag",
            "textEntityTypeCashtag" => "cashtag",
            "textEntityTypeBotCommand" => "botCommand",
            "textEntityTypeEmailAddress" => "email",
            "textEntityTypePhoneNumber" => "phone",
            "textEntityTypeCustomEmoji" => "customEmoji",
            _ => continue,
        };
        let offset = integer(&entity["offset"], -1);
        let length = integer(&entity["length"], -1);
        if offset < 0 || length <= 0 || offset.checked_add(length).is_none_or(|end| end > size) {
            continue;
        }
        let mut item = json!({"type":name,"offset":number(offset),"length":number(length)});
        match name {
            "textUrl" => item["url"] = json!(text(&kind["url"], 2048)),
            "mentionName" => item["userId"] = number(integer(&kind["user_id"], 0)),
            "preCode" => item["language"] = json!(text(&kind["language"], 64)),
            "customEmoji" => {
                item["customEmojiId"] = json!(integer(&kind["custom_emoji_id"], 0).to_string())
            }
            _ => {}
        }
        entities.push(item);
    }
    json!([body, entities])
}

pub fn local_path(value: &Value, roots: &[String]) -> String {
    let Some(path) = value.as_str() else {
        return String::new();
    };
    if !path.starts_with('/')
        || path.chars().count() > 4096
        || path.chars().any(|c| c < ' ' || c == '\u{7f}')
        || path
            .split('/')
            .skip(1)
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return String::new();
    }
    if roots.iter().any(|root| {
        root.starts_with('/') && path.starts_with(&format!("{}/", root.trim_end_matches('/')))
    }) {
        path.to_owned()
    } else {
        String::new()
    }
}

pub fn file_view(value: &Value, roots: &[String]) -> Value {
    let f = typed(value, "file");
    let id = integer(&f["id"], 0);
    if id <= 0 {
        return Value::Null;
    }
    let local = &f["local"];
    json!({"id":number(id), "size":number(integer(&f["size"],0).max(integer(&f["expected_size"],0)).max(0)),
        "downloaded":nonnegative(&local["downloaded_size"]), "active":yes(&local["is_downloading_active"]),
        "path":if yes(&local["is_downloading_completed"]) { local_path(&local["path"], roots) } else { String::new() }})
}

pub fn minithumbnail(value: &Value) -> Value {
    let m = typed(value, "minithumbnail");
    let Some(data) = m["data"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 16384 && s.is_ascii())
    else {
        return Value::Null;
    };
    json!({"width":nonnegative(&m["width"]),"height":nonnegative(&m["height"]),"data":data})
}

pub fn thumbnail(value: &Value, roots: &[String]) -> Value {
    let t = typed(value, "thumbnail");
    let file = file_view(&t["file"], roots);
    if file.is_null() {
        return Value::Null;
    }
    let format = match tag(&t["format"]) {
        "thumbnailFormatJpeg" => "jpeg",
        "thumbnailFormatGif" => "gif",
        "thumbnailFormatMpeg4" => "mp4",
        "thumbnailFormatPng" => "png",
        "thumbnailFormatTgs" => "tgs",
        "thumbnailFormatWebm" => "webm",
        "thumbnailFormatWebp" => "webp",
        _ => "unknown",
    };
    json!({"format":format,"width":nonnegative(&t["width"]),"height":nonnegative(&t["height"]),"file":file})
}

fn dimensions(value: &Value) -> (i128, i128) {
    (
        integer(&value["width"], 0).clamp(0, 100000),
        integer(&value["height"], 0).clamp(0, 100000),
    )
}
fn duration(value: &Value) -> Value {
    number(integer(&value["duration"], 0).clamp(0, 7 * 24 * 3600))
}
fn photo_sizes(value: &Value) -> Vec<(i128, i128, &Value)> {
    array(value, 16)
        .filter_map(|v| {
            let (w, h) = dimensions(v);
            (tag(v) == "photoSize" && w > 0 && h > 0 && tag(&v["photo"]) == "file")
                .then_some((w, h, v))
        })
        .collect()
}

pub fn photo_size(value: &Value, preview: bool) -> Value {
    let sizes = photo_sizes(value);
    let selected = select_photo(&sizes, preview);
    selected
        .map(|(w, h, size)| json!([number(w), number(h), size]))
        .unwrap_or(Value::Null)
}

fn select_photo<'a>(
    sizes: &[(i128, i128, &'a Value)],
    preview: bool,
) -> Option<(i128, i128, &'a Value)> {
    let preferred = sizes.iter().filter(|(w, h, _)| {
        if preview {
            (*w).max(*h) >= 320
        } else {
            (*w).max(*h) <= 1280
        }
    });
    let choices: Vec<_> = preferred.copied().collect();
    let candidates = if choices.is_empty() { sizes } else { &choices };
    let minimize = if preview {
        !choices.is_empty()
    } else {
        choices.is_empty()
    };
    // Python max/min retain the first equal key, unlike Iterator::max_by_key.
    candidates.iter().copied().reduce(|a, b| {
        if if minimize {
            b.0 * b.1 < a.0 * a.1
        } else {
            b.0 * b.1 > a.0 * a.1
        } {
            b
        } else {
            a
        }
    })
}

pub fn media_for(kind: &str, content: &Value, roots: &[String]) -> Value {
    if kind == "photo" {
        let photo = &content["photo"];
        let sizes = photo_sizes(&photo["sizes"]);
        let Some((w, h, size)) = select_photo(&sizes, false) else {
            return Value::Null;
        };
        let Some((pw, ph, small)) = select_photo(&sizes, true) else {
            return Value::Null;
        };
        return json!({"file":file_view(&size["photo"],roots),"width":number(w),"height":number(h),
            "preview":{"file":file_view(&small["photo"],roots),"width":number(pw),"height":number(ph)},
            "mini":minithumbnail(&photo["minithumbnail"])});
    }
    let (key, typename, filekey) = match kind {
        "sticker" => ("sticker", "sticker", "sticker"),
        "gif" => ("animation", "animation", "animation"),
        "voice" => ("voice_note", "voiceNote", "voice"),
        "videoNote" => ("video_note", "videoNote", "video"),
        "video" => ("video", "video", "video"),
        "file" => ("document", "document", "document"),
        "audio" => ("audio", "audio", "audio"),
        _ => return Value::Null,
    };
    let value = typed(&content[key], typename);
    let file = file_view(&value[filekey], roots);
    if file.is_null() {
        return Value::Null;
    }
    let (w, h) = dimensions(value);
    let mut out = json!({"file":file});
    match kind {
        "sticker" => {
            out["format"] = json!(match tag(&value["format"]) {
                "stickerFormatWebp" => "webp",
                "stickerFormatTgs" => "tgs",
                "stickerFormatWebm" => "webm",
                _ => "unknown",
            });
            out["width"] = number(w);
            out["height"] = number(h);
            out["emoji"] = json!(text(&value["emoji"], 16));
            let set = integer(&value["set_id"], 0);
            out["setId"] = json!(if set != 0 {
                set.to_string()
            } else {
                String::new()
            });
        }
        "gif" | "video" => {
            out["width"] = number(w);
            out["height"] = number(h);
            out["duration"] = duration(value);
            if kind == "gif" {
                out["mime"] = json!(text(&value["mime_type"], 128));
            } else {
                out["fileName"] = json!(text(&value["file_name"], 256));
            }
        }
        "videoNote" => {
            out["duration"] = duration(value);
            out["length"] = number(integer(&value["length"], 0).clamp(0, 4096));
            out["viewed"] = json!(yes(&content["is_viewed"]));
        }
        "voice" => {
            out["duration"] = duration(value);
            out["waveform"] = waveform(&value["waveform"], 48);
            out["mime"] = json!(text(&value["mime_type"], 128));
            out["listened"] = json!(yes(&content["is_listened"]));
        }
        "file" => {
            out["fileName"] = json!(text(&value["file_name"], 256));
            out["mime"] = json!(text(&value["mime_type"], 128));
        }
        "audio" => {
            out["duration"] = duration(value);
            for (wire, key) in [
                ("title", "title"),
                ("performer", "performer"),
                ("fileName", "file_name"),
            ] {
                out[wire] = json!(text(&value[key], 256));
            }
            out["mime"] = json!(text(&value["mime_type"], 128));
        }
        _ => {}
    }
    if matches!(kind, "sticker" | "gif" | "videoNote" | "video" | "file") {
        out["thumb"] = thumbnail(&value["thumbnail"], roots);
    }
    if matches!(kind, "gif" | "videoNote" | "video") {
        out["mini"] = minithumbnail(&value["minithumbnail"]);
    }
    out
}

pub fn waveform(value: &Value, bars: usize) -> Value {
    let Some(s) = value.as_str().filter(|s| !s.is_empty() && s.len() <= 256) else {
        return json!([]);
    };
    let Some(raw) = decode_base64(s) else {
        return json!([]);
    };
    let samples: Vec<u8> = (0..raw.len() * 8 / 5)
        .map(|i| {
            let bit = i * 5;
            let index = bit / 8;
            let shift = bit % 8;
            let pair =
                u16::from(raw[index]) | (u16::from(raw.get(index + 1).copied().unwrap_or(0)) << 8);
            u8::try_from((pair >> shift) & 31).unwrap_or(0)
        })
        .collect();
    if samples.len() <= bars {
        return json!(samples);
    }
    if bars == 0 {
        return json!([]);
    }
    let step = samples.len() as f64 / bars as f64;
    let peaks: Vec<_> = (0..bars)
        .map(|i| {
            let start = (i as f64 * step) as usize;
            let end = ((i + 1) as f64 * step) as usize;
            samples[start..end.max(start + 1)]
                .iter()
                .copied()
                .max()
                .unwrap_or(0)
        })
        .collect();
    json!(peaks)
}

// This small decoder is for the bounded, legacy waveform only. Crypto and the
// bridge must use maintained native libraries, not this projection helper.
pub(crate) fn decode_base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut accumulator = 0u32;
    let mut bits = 0;
    let mut count = 0;
    let mut padding = 0;
    for b in s.bytes() {
        if b == b'=' {
            padding += 1;
            continue;
        }
        if padding > 0 {
            return None;
        }
        let n = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        count += 1;
        accumulator = (accumulator << 6) | u32::from(n);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((accumulator >> bits) & 255).ok()?);
        }
    }
    if (count % 4 == 0 && padding != 0)
        || count % 4 == 1
        || (count % 4 == 2 && padding != 2)
        || (count % 4 == 3 && padding != 1)
    {
        return None;
    }
    Some(out)
}

pub fn list_key(value: &Value) -> Value {
    match tag(value) {
        "chatListMain" => json!("main"),
        "chatListArchive" => json!("archive"),
        "chatListFolder" => json!(format!("folder:{}", integer(&value["chat_folder_id"], 0))),
        _ => Value::Null,
    }
}

pub fn auth_view(value: &Value) -> Value {
    let state = match tag(value) {
        "authorizationStateWaitTdlibParameters" => "starting",
        "authorizationStateWaitPhoneNumber" => "phone",
        "authorizationStateReady" => "ready",
        "authorizationStateLoggingOut" => "loggingOut",
        "authorizationStateClosing" => "closing",
        "authorizationStateClosed" => "closed",
        _ => "",
    };
    if !state.is_empty() {
        return json!({"state":state});
    }
    match tag(value) {
        "authorizationStateWaitCode" => {
            let info = &value["code_info"];
            let kind = &info["type"];
            let via = text(&kind["@type"], 64).replace("authenticationCodeType", "");
            json!({"state":"code","phone":text(&info["phone_number"],32),"via":if via.is_empty(){"unknown"}else{&via},"length":number(integer(&kind["length"],0))})
        }
        "authorizationStateWaitPassword" => {
            json!({"state":"password","hint":text(&value["password_hint"],128)})
        }
        "authorizationStateWaitOtherDeviceConfirmation" => {
            json!({"state":"qr","link":text(&value["link"],2048)})
        }
        _ => json!({"state":"unsupported","reason":text(&value["@type"],64)}),
    }
}

pub fn project(operation: &str, value: &Value, roots: &[String]) -> Result<Value, &'static str> {
    Ok(match operation {
        "state_sequence" => crate::state::project(value, roots),
        "proxy_crypto" => crate::proxy_crypto::project(value),
        "menu_helper" => match value["kind"].as_str() {
            Some("strip") => json!(crate::menu::strip_jsonc(
                value["raw"].as_str().unwrap_or("")
            )),
            Some("valid") => json!(crate::menu::valid(value["raw"].as_str().unwrap_or(""))),
            _ => json!(crate::menu::without_block(
                value["raw"].as_str().unwrap_or("")
            )),
        },
        "command" => crate::commands::project(value, roots),
        "preferences" => crate::preferences::project(value),
        "preferences_command" => crate::preferences_commands::project(value),
        "media_helper" => crate::media::project(value),
        "upload_helper" => crate::uploads::project(value),
        "sound_helper" => crate::sounds::project(value),
        "notification_helper" => crate::notifications::project(value),
        "export_gate" => crate::export::project(value, roots),
        "download_name" => crate::files::project(value),
        "clipboard_paths" => crate::clipboard::project(value),
        "accounts" => crate::accounts::project(value),
        "formatted" => formatted(value),
        "local_path" => json!(local_path(value, roots)),
        "file_view" => file_view(value, roots),
        "minithumbnail" => minithumbnail(value),
        "thumbnail" => thumbnail(value, roots),
        "best_photo_size" => photo_size(value, false),
        "preview_photo_size" => photo_size(value, true),
        "waveform" => waveform(value, 48),
        "media_for" => media_for(
            value["kind"].as_str().unwrap_or(""),
            field(value, "content"),
            roots,
        ),
        "auth_view" => auth_view(value),
        "list_key" => list_key(value),
        _ => return crate::content_projection::project(operation, value, roots),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_entities_use_utf16() {
        let value = json!({"text":"😀x","entities":[{"offset":0,"length":2,"type":{"@type":"textEntityTypeBold"}},{"offset":2,"length":2,"type":{"@type":"textEntityTypeItalic"}}]});
        assert_eq!(
            formatted(&value),
            json!(["😀x",[{"offset":0,"length":2,"type":"bold"}]])
        );
    }
    #[test]
    fn sent_video_paths_are_guarded_like_downloads() {
        let roots = vec!["/data/sent".to_owned()];
        for path in [
            "/data/sent/../database/db.sqlite",
            "/data/sent//a",
            "/data/sentx/a",
            "/data/sent/a\n",
        ] {
            assert_eq!(local_path(&json!(path), &roots), "");
        }
        assert_eq!(
            local_path(&json!("/data/sent/video.mp4"), &roots),
            "/data/sent/video.mp4"
        );
        let file = json!({"@type":"file","id":1,"size":-1,"expected_size":"42","local":{"is_downloading_completed":false,"path":"/data/sent/video.mp4"}});
        assert_eq!(file_view(&file, &roots)["path"], "");
        assert_eq!(file_view(&file, &roots)["size"], 42);
    }
}
