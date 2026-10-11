use crate::projection::{array, field, formatted, integer, number, tag, text, typed, yes};
use serde_json::{Value, json};

pub const FOLDER_FLAGS: [(&str, &str); 8] = [
    ("includeContacts", "include_contacts"),
    ("includeNonContacts", "include_non_contacts"),
    ("includeGroups", "include_groups"),
    ("includeChannels", "include_channels"),
    ("includeBots", "include_bots"),
    ("excludeMuted", "exclude_muted"),
    ("excludeRead", "exclude_read"),
    ("excludeArchived", "exclude_archived"),
];
const NOTIFICATION_FLAGS: [&str; 13] = [
    "use_default_mute_for",
    "use_default_sound",
    "use_default_show_preview",
    "show_preview",
    "use_default_mute_stories",
    "mute_stories",
    "use_default_story_sound",
    "use_default_show_story_poster",
    "show_story_poster",
    "use_default_disable_pinned_message_notifications",
    "disable_pinned_message_notifications",
    "use_default_disable_mention_notifications",
    "disable_mention_notifications",
];

pub fn status_view(value: &Value) -> Value {
    match tag(value) {
        "userStatusOnline" => {
            json!({"state":"online","expires":number(integer(&value["expires"],0))})
        }
        "userStatusOffline" => {
            json!({"state":"offline","wasOnline":number(integer(&value["was_online"],0))})
        }
        kind => {
            json!({"state":match kind {"userStatusRecently"=>"recently","userStatusLastWeek"=>"lastWeek","userStatusLastMonth"=>"lastMonth",_=>""}})
        }
    }
}

pub fn can_change_info(value: &Value) -> bool {
    tag(value) == "chatMemberStatusCreator"
        || (tag(value) == "chatMemberStatusAdministrator"
            && yes(&value["rights"]["can_change_info"]))
}

pub fn topic_id(value: &Value) -> Value {
    number(integer(&typed(value, "messageTopicForum")["forum_topic_id"], 0).max(0))
}
pub fn thread_id(value: &Value) -> Value {
    number(integer(&typed(value, "messageTopicThread")["message_thread_id"], 0).max(0))
}
pub fn scheduled_at(value: &Value) -> Value {
    number(match tag(value) {
        "messageSchedulingStateSendAtDate" | "messageSchedulingStateSendWhenVideoProcessed" => {
            integer(&value["send_date"], 0).max(1)
        }
        "messageSchedulingStateSendWhenOnline" => -1,
        _ => 0,
    })
}

pub fn formatted_view(value: &Value) -> Value {
    let parsed = formatted(value);
    json!({"text":parsed[0],"entities":parsed[1]})
}
pub fn draft_text(value: &Value) -> Value {
    let d = typed(value, "draftMessage");
    let c = typed(&d["content"], "draftMessageContentText");
    formatted(&c["text"])[0].clone()
}

pub fn key_hash_view(value: &Value) -> Value {
    let Some(s) = value.as_str().filter(|s| !s.is_empty() && s.len() <= 256) else {
        return json!("");
    };
    let Some(raw) = crate::projection::decode_base64(s) else {
        return json!("");
    };
    let hex = raw
        .iter()
        .take(32)
        .map(|v| format!("{v:02x}"))
        .collect::<String>();
    json!(
        hex.as_bytes()
            .chunks(8)
            .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

pub fn session_view(value: &Value) -> Value {
    let s = typed(value, "session");
    if s.is_null() {
        return Value::Null;
    }
    let device = text(&s["device_type"]["@type"], 64)
        .replace("sessionDeviceType", "")
        .to_lowercase();
    let devices = [
        "android", "apple", "brave", "chrome", "edge", "firefox", "ipad", "iphone", "linux", "mac",
        "opera", "safari", "ubuntu", "vivaldi", "windows", "xbox",
    ];
    let mut out = json!({"id":integer(&s["id"],0).to_string(),"current":yes(&s["is_current"]),"passwordPending":yes(&s["is_password_pending"]),"unconfirmed":yes(&s["is_unconfirmed"]),"official":yes(&s["is_official_application"]),"type":if devices.contains(&device.as_str()){&device}else{"unknown"}});
    for (wire, key, limit) in [
        ("app", "application_name", 128),
        ("appVersion", "application_version", 64),
        ("device", "device_model", 128),
        ("platform", "platform", 64),
        ("system", "system_version", 64),
        ("ip", "ip_address", 64),
        ("location", "location", 128),
    ] {
        out[wire] = json!(text(&s[key], limit));
    }
    out["loginDate"] = number(integer(&s["log_in_date"], 0).max(0));
    out["lastActive"] = number(integer(&s["last_active_date"], 0).max(0));
    out
}

fn privacy_base(kind: &str) -> Option<&'static str> {
    match kind {
        "userPrivacySettingRuleAllowAll" => Some("everybody"),
        "userPrivacySettingRuleAllowContacts" => Some("contacts"),
        "userPrivacySettingRuleRestrictAll" => Some("nobody"),
        _ => None,
    }
}
pub fn privacy_rule_list(value: &Value) -> Vec<&Value> {
    array(&typed(value, "userPrivacySettingRules")["rules"], 64)
        .filter(|r| tag(r).starts_with("userPrivacySettingRule"))
        .collect()
}
pub fn privacy_view(value: &Value) -> Value {
    let mut base = None;
    let mut allowed = 0;
    let mut restricted = 0;
    for rule in privacy_rule_list(value) {
        let kind = tag(rule);
        if let Some(b) = privacy_base(kind) {
            base = base.or(Some(b));
            continue;
        }
        let count =
            array(&rule["user_ids"], 10000).count() + array(&rule["chat_ids"], 10000).count();
        if kind.starts_with("userPrivacySettingRuleAllow") {
            allowed += count;
        } else {
            restricted += count;
        }
    }
    json!({"base":base.unwrap_or("nobody"),"allowed":allowed,"restricted":restricted})
}
pub fn privacy_rules(value: &Value, base: &str) -> Result<Value, &'static str> {
    let general = match base {
        "everybody" => "userPrivacySettingRuleAllowAll",
        "contacts" => "userPrivacySettingRuleAllowContacts",
        "nobody" => "userPrivacySettingRuleRestrictAll",
        _ => return Err("invalid privacy base"),
    };
    let mut kept: Vec<Value> = privacy_rule_list(value)
        .into_iter()
        .filter(|r| {
            let kind = tag(r);
            let allows = kind.starts_with("userPrivacySettingRuleAllow");
            privacy_base(kind).is_none()
                && !(allows && base == "everybody" || !allows && base == "nobody")
        })
        .cloned()
        .collect();
    kept.push(json!({"@type":general}));
    Ok(json!({"@type":"userPrivacySettingRules","rules":kept}))
}

pub fn password_view(value: &Value) -> Value {
    let p = typed(value, "passwordState");
    let c = typed(
        &p["recovery_email_address_code_info"],
        "emailAddressAuthenticationCodeInfo",
    );
    json!({"hasPassword":yes(&p["has_password"]),"hint":text(&p["password_hint"],128),"hasRecoveryEmail":yes(&p["has_recovery_email_address"]),"emailCodePattern":text(&c["email_address_pattern"],128),"emailCodeLength":number(integer(&c["length"],0).max(0)),"resetDate":number(integer(&p["pending_reset_date"],0).max(0))})
}
pub fn proxy_view(value: &Value) -> Value {
    let p = typed(value, "addedProxy");
    let proxy = typed(&p["proxy"], "proxy");
    if proxy.is_null() {
        return Value::Null;
    }
    let kind = &proxy["type"];
    json!({"id":number(integer(&p["id"],0)),"server":text(&proxy["server"],253),"port":number(integer(&proxy["port"],0).max(0)),"type":match tag(kind){"proxyTypeSocks5"=>"socks5","proxyTypeHttp"=>"http","proxyTypeMtproto"=>"mtproto",_=>""},"username":text(&kind["username"],128),"enabled":yes(&p["is_enabled"]),"lastUsed":number(integer(&p["last_used_date"],0).max(0))})
}
pub fn folder_ids(value: &Value) -> Value {
    json!(
        array(value, 1000)
            .map(|v| integer(v, 0))
            .filter(|v| *v != 0)
            .map(number)
            .collect::<Vec<_>>()
    )
}
pub fn folder_view(value: &Value, id: i128) -> Value {
    let f = typed(value, "chatFolder");
    if f.is_null() || id <= 0 {
        return Value::Null;
    }
    let name = formatted(&f["name"]["text"]);
    let mut out = json!({"id":number(id),"name":text(&name[0],128),"icon":text(&f["icon"]["name"],32),"colorId":number(integer(&f["color_id"],-1)),"shareable":yes(&f["is_shareable"]),"pinned":folder_ids(&f["pinned_chat_ids"]),"included":folder_ids(&f["included_chat_ids"]),"excluded":folder_ids(&f["excluded_chat_ids"])});
    for (key, field) in FOLDER_FLAGS {
        out[key] = json!(yes(&f[field]));
    }
    out
}
pub fn scope_view(value: &Value) -> Value {
    let s = typed(value, "scopeNotificationSettings");
    json!({"muted":integer(&s["mute_for"],0)>0,"preview":yes(&s["show_preview"])})
}
pub fn scope_settings(value: &Value, muted: &Value, preview: &Value) -> Value {
    let s = typed(value, "scopeNotificationSettings");
    let mut out = json!({"@type":"scopeNotificationSettings","mute_for":number(integer(&s["mute_for"],0).max(0)),"sound_id":number(integer(&s["sound_id"],0)),"story_sound_id":number(integer(&s["story_sound_id"],0))});
    for field in [
        "show_preview",
        "use_default_mute_stories",
        "mute_stories",
        "show_story_poster",
        "disable_pinned_message_notifications",
        "disable_mention_notifications",
    ] {
        out[field] = json!(yes(&s[field]));
    }
    if !muted.is_null() {
        out["mute_for"] = json!(if crate::wire::truthy(muted) {
            i32::MAX
        } else {
            0
        });
    }
    if !preview.is_null() {
        out["show_preview"] = preview.clone();
    }
    out
}
pub fn notification_settings(value: &Value) -> Value {
    let mut out = json!({"@type":"chatNotificationSettings","mute_for":number(integer(&value["mute_for"],0).max(0))});
    for flag in NOTIFICATION_FLAGS {
        out[flag] = json!(yes(&value[flag]));
    }
    for key in ["sound_id", "story_sound_id"] {
        out[key] = json!(integer(&value[key], 0).to_string());
    }
    out
}
pub fn clock_duration(seconds: i128) -> String {
    let seconds = seconds.max(0);
    let hours = seconds / 3600;
    let minutes = seconds % 3600 / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}
pub fn human_period(seconds: i128) -> String {
    for (unit, size) in [("day", 86400), ("hour", 3600), ("minute", 60)] {
        if seconds >= size && seconds % size == 0 {
            let count = seconds / size;
            return format!("{count} {unit}{}", if count == 1 { "" } else { "s" });
        }
    }
    format!("{seconds} seconds")
}

pub fn project(operation: &str, value: &Value) -> Result<Value, &'static str> {
    Ok(match operation {
        "status_view" => status_view(value),
        "can_change_info" => json!(can_change_info(value)),
        "draft_text" => draft_text(value),
        "formatted_view" => formatted_view(value),
        "topic_id" => topic_id(value),
        "thread_id" => thread_id(value),
        "scheduled_at" => scheduled_at(value),
        "key_hash_view" => key_hash_view(value),
        "session_view" => session_view(value),
        "privacy_rule_list" => json!(privacy_rule_list(value)),
        "privacy_view" => privacy_view(value),
        "privacy_rules" => {
            privacy_rules(field(value, "rules"), value["base"].as_str().unwrap_or(""))?
        }
        "password_view" => password_view(value),
        "proxy_view" => proxy_view(value),
        "folder_ids" => folder_ids(value),
        "folder_view" => folder_view(field(value, "folder"), integer(&value["id"], 0)),
        "scope_view" => scope_view(value),
        "scope_settings" => {
            scope_settings(field(value, "settings"), &value["muted"], &value["preview"])
        }
        "notification_settings" => notification_settings(value),
        "clock_duration" => json!(clock_duration(integer(value, 0))),
        "human_period" => json!(human_period(integer(value, 0))),
        _ => return Err("projection is not implemented"),
    })
}
