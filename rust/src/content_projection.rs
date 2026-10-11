use crate::projection::{
    array, file_view, formatted, integer, media_for, minithumbnail, number, photo_size, tag, text,
    typed, yes,
};
use serde_json::{Value, json};

pub fn content(value: &Value, roots: &[String]) -> Value {
    let kind_name = tag(value);
    if kind_name == "messageRichMessage" {
        return crate::rich_projection::rich_message(&value["message"], roots);
    }
    if kind_name == "messageText" {
        let ft = formatted(&value["text"]);
        let mut out = json!({"kind":"text","text":ft[0],"entities":ft[1]});
        let preview = link_preview(&value["link_preview"], roots);
        if !preview.is_null() {
            out["linkPreview"] = preview;
        }
        return out;
    }
    let kind = match kind_name {
        "messagePhoto" => "photo",
        "messageSticker" => "sticker",
        "messageAnimation" => "gif",
        "messageVoiceNote" => "voice",
        "messageVideoNote" => "videoNote",
        "messageVideo" => "video",
        "messageDocument" => "file",
        "messageAudio" => "audio",
        "messageAnimatedEmoji" | "messageDice" => "emoji",
        "messageContact" => "contact",
        "messageLocation" | "messageVenue" => "location",
        "messagePoll" => "poll",
        name if [
            "messageChat",
            "messagePin",
            "messageBasicGroup",
            "messageSupergroup",
            "messageContactRegistered",
            "messageCall",
            "messageVideoChat",
            "messageScreenshotTaken",
            "messageForumTopic",
            "messageCustomServiceAction",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix)) =>
        {
            "service"
        }
        _ => "unsupported",
    };
    let ft = formatted(&value["caption"]);
    let mut out = json!({"kind":kind,"text":ft[0],"entities":ft[1]});
    match kind {
        "sticker" => out["emoji"] = json!(text(&value["sticker"]["emoji"], 16)),
        "file" => out["fileName"] = json!(text(&value["document"]["file_name"], 256)),
        "emoji" => {
            let mut body = text(&value["emoji"], 16);
            let result = integer(&value["value"], 0);
            if kind_name == "messageDice" && result > 0 {
                body.push_str(&format!(" {result}"));
            }
            out["text"] = json!(body);
        }
        "service" | "unsupported" => out["type"] = json!(text(&value["@type"], 64)),
        "poll" => {
            let poll = poll_view(&value["poll"]);
            if !poll.is_null() {
                out["text"] = poll["question"].clone();
                out["poll"] = poll;
            }
        }
        "location" => {
            let location = location_view(value);
            if !location.is_null() {
                out["text"] = location["title"].clone();
                out["location"] = location;
            }
        }
        "contact" => {
            let contact = contact_view(&value["contact"]);
            if !contact.is_null() {
                out["text"] = contact["name"].clone();
                out["contact"] = contact;
            }
        }
        _ => {}
    }
    if yes(&value["has_spoiler"]) {
        out["spoiler"] = json!(true);
    }
    let media = media_for(kind, value, roots);
    if !media.is_null() {
        out["media"] = media;
    }
    out
}

pub fn link_preview(value: &Value, roots: &[String]) -> Value {
    let p = typed(value, "linkPreview");
    let url = text(&p["url"], 2048);
    if url.is_empty() {
        return Value::Null;
    }
    let description = formatted(&p["description"]);
    let mut out = json!({"url":url,"displayUrl":text(&p["display_url"],2048),"siteName":text(&p["site_name"],256),"title":text(&p["title"],256),"description":text(&description[0],600),"photo":null,"above":yes(&p["show_above_text"])});
    let kind = &p["type"];
    if matches!(tag(kind), "linkPreviewTypeArticle" | "linkPreviewTypePhoto") {
        let photo = typed(&kind["photo"], "photo");
        let size = photo_size(&photo["sizes"], false);
        if !size.is_null() {
            out["photo"] = json!({"file":file_view(&size[2]["photo"],roots),"width":size[0],"height":size[1],"mini":minithumbnail(&photo["minithumbnail"])});
        }
    }
    out
}

pub fn poll_view(value: &Value) -> Value {
    let p = typed(value, "poll");
    if p.is_null() {
        return Value::Null;
    }
    let question = formatted(&p["question"]);
    let options:Vec<_>=array(&p["options"],12).enumerate().map(|(index,raw)|{
        let o=typed(raw,"pollOption");let ft=formatted(&o["text"]);
        json!({"index":index,"text":text(&ft[0],300),"voters":number(integer(&o["voter_count"],0).max(0)),"percent":number(integer(&o["vote_percentage"],0).clamp(0,100)),"chosen":yes(&o["is_chosen"])})
    }).collect();
    let kind = &p["type"];
    let quiz = tag(kind) == "pollTypeQuiz";
    let correct: Vec<_> = if quiz {
        array(&kind["correct_option_ids"], 12)
            .map(|v| number(integer(v, 0)))
            .collect()
    } else {
        Vec::new()
    };
    let voted = options.iter().any(|o| yes(&o["chosen"]));
    json!({"id":integer(&p["id"],0).to_string(),"question":text(&question[0],300),"options":options,"total":number(integer(&p["total_voter_count"],0).max(0)),"closed":yes(&p["is_closed"]),"anonymous":yes(&p["is_anonymous"]),"multiple":yes(&p["allows_multiple_answers"]),"canRevote":yes(&p["allows_revoting"]),"quiz":quiz,"correct":correct,"voted":voted})
}
fn float(value: &Value, low: f64, high: f64) -> Option<f64> {
    value.as_f64().filter(|v| *v >= low && *v <= high)
}
pub fn location_view(value: &Value) -> Value {
    let venue = typed(&value["venue"], "venue");
    let loc = typed(
        if venue.is_null() {
            &value["location"]
        } else {
            &venue["location"]
        },
        "location",
    );
    let (Some(lat), Some(lon)) = (
        float(&loc["latitude"], -90.0, 90.0),
        float(&loc["longitude"], -180.0, 180.0),
    ) else {
        return Value::Null;
    };
    json!({"lat":lat,"lon":lon,"title":text(&venue["title"],256),"address":text(&venue["address"],256)})
}
pub fn contact_view(value: &Value) -> Value {
    let p = typed(value, "contact");
    if p.is_null() {
        return Value::Null;
    }
    let name = [text(&p["first_name"], 128), text(&p["last_name"], 128)]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    json!({"name":name,"phone":text(&p["phone_number"],32),"userId":number(integer(&p["user_id"],0))})
}
fn inline_button(value: &Value) -> Value {
    let b = typed(value, "inlineKeyboardButton");
    if b.is_null() {
        return Value::Null;
    }
    let t = &b["type"];
    let kind = match tag(t) {
        "inlineKeyboardButtonTypeCallback" => "callback",
        "inlineKeyboardButtonTypeUrl" | "inlineKeyboardButtonTypeLoginUrl" => "url",
        "inlineKeyboardButtonTypeWebApp" => "webApp",
        "inlineKeyboardButtonTypeSwitchInline" => "switchInline",
        "inlineKeyboardButtonTypeUser" => "user",
        "inlineKeyboardButtonTypeCopyText" => "copy",
        "inlineKeyboardButtonTypeCallbackGame" => "game",
        "inlineKeyboardButtonTypeBuy" => "buy",
        "inlineKeyboardButtonTypeCallbackWithPassword" => "password",
        "inlineKeyboardButtonTypeDisabled" => "disabled",
        _ => "unsupported",
    };
    let mut out = json!({"text":text(&b["text"],64),"kind":kind});
    match kind {
        "callback" => {
            if let Some(data) = t["data"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 128 && s.is_ascii())
            {
                out["data"] = json!(data);
            } else {
                out["kind"] = json!("unsupported");
            }
        }
        "url" | "webApp" => out["url"] = json!(text(&t["url"], 2048)),
        "user" => out["userId"] = number(integer(&t["user_id"], 0)),
        "copy" => out["copyText"] = json!(text(&t["text"], 4096)),
        "switchInline" => out["query"] = json!(text(&t["query"], 256)),
        _ => {}
    }
    out
}
fn keyboard_button(value: &Value) -> Value {
    let b = typed(value, "keyboardButton");
    if b.is_null() {
        return Value::Null;
    }
    json!({"text":text(&b["text"],64),"kind":match tag(&b["type"]){"keyboardButtonTypeText"=>"text","keyboardButtonTypeRequestPhoneNumber"=>"requestPhone","keyboardButtonTypeRequestLocation"=>"requestLocation",_=>"unsupported"}})
}
pub fn reply_markup(value: &Value) -> Value {
    let kind = tag(value);
    if matches!(
        kind,
        "replyMarkupInlineKeyboard" | "replyMarkupShowKeyboard"
    ) {
        let rows: Vec<Vec<Value>> = array(&value["rows"], 12)
            .map(|r| {
                array(r, 8)
                    .map(if kind == "replyMarkupInlineKeyboard" {
                        inline_button
                    } else {
                        keyboard_button
                    })
                    .filter(|b| !b.is_null())
                    .collect()
            })
            .filter(|r: &Vec<Value>| !r.is_empty())
            .collect();
        if rows.is_empty() {
            return Value::Null;
        }
        return if kind == "replyMarkupInlineKeyboard" {
            json!({"type":"inline","rows":rows})
        } else {
            json!({"type":"keyboard","rows":rows,"oneTime":yes(&value["one_time"]),"resize":yes(&value["resize_keyboard"]),"persistent":yes(&value["is_persistent"]),"placeholder":text(&value["input_field_placeholder"],64)})
        };
    }
    match kind {
        "replyMarkupRemoveKeyboard" => json!({"type":"remove"}),
        "replyMarkupForceReply" => {
            json!({"type":"forceReply","placeholder":text(&value["input_field_placeholder"],64)})
        }
        _ => Value::Null,
    }
}
pub fn reactions_view(value: &Value) -> Value {
    let info = typed(value, "messageInteractionInfo");
    let mut out = Vec::new();
    for raw in array(&info["reactions"]["reactions"], 20) {
        let r = typed(raw, "messageReaction");
        let t = &r["type"];
        let count = integer(&r["total_count"], 0).max(0);
        if count == 0 {
            continue;
        }
        let chosen = yes(&r["is_chosen"]);
        match tag(t){"reactionTypeEmoji"=>{let emoji=text(&t["emoji"],16);if !emoji.is_empty(){out.push(json!({"emoji":emoji,"count":number(count),"chosen":chosen}));}},"reactionTypeCustomEmoji"=>out.push(json!({"emoji":"","customEmojiId":integer(&t["custom_emoji_id"],0).to_string(),"count":number(count),"chosen":chosen})),"reactionTypePaid"=>out.push(json!({"emoji":"⭐","paid":true,"count":number(count),"chosen":chosen})),_=>{},}
    }
    json!([out, number(integer(&info["view_count"], 0).max(0))])
}
pub fn replies_view(value: &Value) -> Value {
    let r = typed(value, "messageReplyInfo");
    if r.is_null() {
        return Value::Null;
    }
    json!({"count":number(integer(&r["reply_count"],0).max(0)),"unread":integer(&r["last_message_id"],0)>integer(&r["last_read_inbox_message_id"],0).max(0)})
}
pub fn preview_text(value: &Value) -> Value {
    let kind = value["kind"].as_str().unwrap_or("");
    let body = text(&value["text"], 16384);
    let label = preview_label(kind);
    let body = if matches!(kind, "text" | "emoji" | "rich") || kind == "service" && !body.is_empty()
    {
        body
    } else if kind == "sticker" {
        format!("{} Sticker", value["emoji"].as_str().unwrap_or(""))
            .trim()
            .to_owned()
    } else if kind == "file" && crate::wire::truthy(&value["fileName"]) && body.is_empty() {
        text(&value["fileName"], 256)
    } else if body.is_empty() {
        label.to_owned()
    } else {
        format!("{label}, {body}")
    };
    json!(
        body.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(200)
            .collect::<String>()
    )
}
pub fn preview_label(kind: &str) -> &'static str {
    match kind {
        "photo" => "Photo",
        "sticker" => "Sticker",
        "gif" => "GIF",
        "voice" => "Voice message",
        "videoNote" => "Video message",
        "video" => "Video",
        "file" => "File",
        "audio" => "Audio",
        "contact" => "Contact",
        "location" => "Location",
        "poll" => "Poll",
        "service" => "Service message",
        _ => "Message",
    }
}

pub fn project(operation: &str, value: &Value, roots: &[String]) -> Result<Value, &'static str> {
    Ok(match operation {
        "content" => content(value, roots),
        "link_preview" => link_preview(value, roots),
        "poll_view" => poll_view(value),
        "location_view" => location_view(value),
        "contact_view" => contact_view(value),
        "reply_markup" => reply_markup(value),
        "reactions_view" => reactions_view(value),
        "replies_view" => replies_view(value),
        "preview_text" => preview_text(value),
        "rich_message" => crate::rich_projection::rich_message(value, roots),
        _ => return crate::settings_projection::project(operation, value),
    })
}
