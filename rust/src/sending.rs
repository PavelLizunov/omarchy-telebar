//! Send/edit preparation. Native Markdown execution is injected by the single
//! TDLib owner; no message is transmitted here and uncertain sends are not retried.
use crate::{
    commands::{self, Action, Answer, Result, arg_int, arg_str, reply, topic},
    projection::{array, number, yes},
    state::State,
};
use serde_json::{Value, json};

pub fn options(args: &Value, state: &State, now: f64) -> Result<Value> {
    let cid = args["chatId"].as_i64().map(i128::from);
    let default = cid
        .and_then(|id| state.chats.get(&id))
        .is_some_and(|c| yes(&c["silent"]));
    let silent = args
        .get("silent")
        .map_or(Some(default), Value::as_bool)
        .ok_or("silent is true or false")?;
    let at = arg_int(args, "sendAt", Some(-1), None, Some(0))?;
    if at > 0 && !(now + 10.0..=now + 366.0 * 86400.0).contains(&(at as f64)) {
        return Err("a message can be scheduled from now to a year ahead".into());
    }
    if args.get("silent").is_none() && !silent && at == 0 {
        return Ok(Value::Null);
    }
    let scheduling = if at > 0 {
        json!({"@type":"messageSchedulingStateSendAtDate","send_date":number(at),"repeat_period":0})
    } else if at == -1 {
        json!({"@type":"messageSchedulingStateSendWhenOnline"})
    } else {
        Value::Null
    };
    Ok(
        json!({"@type":"messageSendOptions","suggested_post_info":null,"disable_notification":silent,"from_background":false,"protect_content":false,"allow_paid_broadcast":false,"paid_message_star_count":0,"update_order_of_installed_sticker_sets":false,"scheduling_state":scheduling,"effect_id":0,"sending_id":0,"only_preview":false}),
    )
}
pub fn typed_text(
    text: &str,
    limit: usize,
    what: &str,
    execute: &mut dyn FnMut(&Value) -> Option<Value>,
) -> Result<Value> {
    let plain = json!({"@type":"formattedText","text":text,"entities":[]});
    let parsed = if text.trim().is_empty() {
        None
    } else {
        execute(&json!({"@type":"parseMarkdown","text":plain}))
    };
    let mut parsed = parsed
        .filter(|v| {
            v["@type"] == "formattedText" && v["text"].is_string() && v["entities"].is_array()
        })
        .unwrap_or(plain);
    if parsed["text"]
        .as_str()
        .is_some_and(|s| s.chars().count() > limit)
    {
        return Err(format!("{what} can be at most {limit} characters"));
    }
    let mut entities = Vec::new();
    for e in array(&parsed["entities"], usize::MAX) {
        let mut e = e.clone();
        if e["type"]["@type"] == "textEntityTypeTextUrl"
            && let Some(raw) = e["type"]["url"]
                .as_str()
                .and_then(|s| s.strip_prefix("tg://user?id="))
            && !raw.starts_with('0')
            && !raw.is_empty()
            && raw.len() <= 19
            && raw.bytes().all(|c| c.is_ascii_digit())
            && let Ok(id) = raw.parse::<i128>()
        {
            e["type"] = json!({"@type":"textEntityTypeMentionName","user_id":number(id)});
        }
        entities.push(e);
    }
    parsed["entities"] = json!(entities);
    Ok(parsed)
}
fn preview(args: &Value) -> Result<Value> {
    let mode = args.get("linkPreview").and_then(Value::as_str).unwrap_or(
        if args.get("linkPreview").is_none() {
            "below"
        } else {
            ""
        },
    );
    match mode {
        "below" => Ok(Value::Null),
        "above" | "none" => Ok(
            json!({"@type":"linkPreviewOptions","is_disabled":mode=="none","url":"","force_small_media":false,"force_large_media":false,"show_above_text":mode=="above"}),
        ),
        _ => Err("linkPreview is below, above or none".into()),
    }
}
fn send(args: &Value, state: &State, now: f64, chat: Value, content: Value) -> Result<Action> {
    Ok(Action::Native {
        query: json!({"@type":"sendMessage","chat_id":chat,"topic_id":topic(args)?,"reply_to":reply(args)?,"options":options(args,state,now)?,"reply_markup":null,"input_message_content":content}),
        answer: Answer::Message,
    })
}

pub fn plan(
    command: &str,
    args: &Value,
    state: &State,
    now: f64,
    language: &str,
    execute: &mut dyn FnMut(&Value) -> Option<Value>,
) -> Result<Action> {
    let cid = || arg_int(args, "chatId", None, None, None).map(number);
    let mid = || arg_int(args, "messageId", Some(1), None, None).map(number);
    let native = |query, answer| Ok(Action::Native { query, answer });
    match command {
        "message.send" => {
            let chat = cid()?;
            let text = typed_text(arg_str(args, "text", 8192, 1)?, 4096, "a message", execute)?;
            send(
                args,
                state,
                now,
                chat,
                json!({"@type":"inputMessageText","text":text,"link_preview_options":preview(args)?,"clear_draft":true}),
            )
        }
        "message.edit" => {
            let chat = cid()?;
            let message = mid()?;
            let query = if yes(&args["caption"]) {
                json!({"@type":"editMessageCaption","chat_id":chat,"message_id":message,"reply_markup":null,"caption":typed_text(arg_str(args,"text",2048,0)?,1024,"a caption",execute)?,"show_caption_above_media":false})
            } else {
                json!({"@type":"editMessageText","chat_id":chat,"message_id":message,"reply_markup":null,"input_message_content":{"@type":"inputMessageText","text":typed_text(arg_str(args,"text",8192,1)?,4096,"a message",execute)?,"link_preview_options":null,"clear_draft":false}})
            };
            native(query, Answer::Message)
        }
        "message.linkPreview" => native(
            json!({"@type":"getLinkPreview","text":typed_text(arg_str(args,"text",8192,1)?,4096,"a message",execute)?,"link_preview_options":null}),
            Answer::LinkPreview,
        ),
        "message.reschedule" => {
            let opts = options(
                &json!({"sendAt":args.get("sendAt").unwrap_or(&json!(0))}),
                state,
                now,
            )?;
            native(
                json!({"@type":"editMessageSchedulingState","chat_id":cid()?,"message_id":mid()?,"scheduling_state":opts["scheduling_state"]}),
                Answer::Empty,
            )
        }
        "message.forward" => native(
            json!({"@type":"forwardMessages","chat_id":cid()?,"topic_id":topic(args)?,"from_chat_id":number(arg_int(args,"fromChatId",None,None,None)?),"message_ids":commands::arg_ids(args,"messageIds")?,"options":options(args,state,now)?,"send_copy":yes(&args["sendCopy"]),"remove_caption":false}),
            Answer::Vote,
        ),
        "message.sendSticker" => {
            let chat = cid()?;
            let emoji = match args.get("emoji") {
                None => "",
                Some(v) => v
                    .as_str()
                    .filter(|s| s.chars().count() <= 16)
                    .ok_or("emoji is a short text")?,
            };
            let content = json!({"@type":"inputMessageSticker","emoji":emoji,"sticker":{"@type":"inputSticker","thumbnail":null,"sticker":{"@type":"inputFileId","id":number(arg_int(args,"fileId",Some(1),Some(i32::MAX as i128),None)?)},"width":number(arg_int(args,"width",Some(0),Some(10000),Some(0))?),"height":number(arg_int(args,"height",Some(0),Some(10000),Some(0))?)}});
            send(args, state, now, chat, content)
        }
        "message.sendDice" => {
            let emoji = args["emoji"].as_str().unwrap_or("");
            if !["🎲", "🎯", "🏀", "⚽", "🎳", "🎰"].contains(&emoji) {
                return Err("dice are one of 🎲 🎯 🏀 ⚽ 🎳 🎰".into());
            }
            send(
                args,
                state,
                now,
                cid()?,
                json!({"@type":"inputMessageDice","emoji":emoji,"clear_draft":false}),
            )
        }
        "message.sendPoll" => {
            let question = arg_str(args, "question", 510, 1)?.trim();
            if !(1..=255).contains(&question.chars().count()) {
                return Err("a poll's question is 1-255 characters".into());
            }
            let raw = args["options"]
                .as_array()
                .filter(|a| (2..=12).contains(&a.len()))
                .ok_or("a poll has 2-12 answers of 1-100 characters")?;
            let mut answers = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for option in raw {
                let answer = option
                    .as_str()
                    .map(str::trim)
                    .filter(|s| (1..=100).contains(&s.chars().count()))
                    .ok_or("a poll has 2-12 answers of 1-100 characters")?;
                answers.push(answer);
            }
            for answer in &answers {
                if !seen.insert(answer.to_lowercase()) {
                    return Err("two answers are the same".into());
                }
            }
            let flag = |key, default| {
                args.get(key)
                    .map_or(Some(default), Value::as_bool)
                    .ok_or_else(|| format!("{key} is true or false"))
            };
            let anonymous = flag("anonymous", true)?;
            let multiple = flag("multiple", false)?;
            let quiz = flag("quiz", false)?;
            let kind = if quiz {
                if multiple {
                    return Err("a quiz has one right answer".into());
                }
                let explanation = if args["explanation"].is_null() {
                    ""
                } else {
                    arg_str(args, "explanation", 200, 0)?.trim()
                };
                json!({"@type":"inputPollTypeQuiz","correct_option_ids":[number(arg_int(args,"correct",Some(0),Some(answers.len() as i128-1),None)?)],"explanation":{"@type":"formattedText","text":explanation,"entities":[]},"explanation_media":null})
            } else {
                json!({"@type":"inputPollTypeRegular","allow_adding_options":false})
            };
            let content = json!({"@type":"inputMessagePoll","question":{"@type":"formattedText","text":question,"entities":[]},"options":answers.into_iter().map(|a|json!({"@type":"inputPollOption","text":{"@type":"formattedText","text":a,"entities":[]},"media":null})).collect::<Vec<_>>(),"description":null,"media":null,"is_anonymous":anonymous,"allows_multiple_answers":multiple,"allows_revoting":!quiz,"members_only":false,"country_codes":[],"shuffle_options":false,"hide_results_until_closes":false,"type":kind,"open_period":0,"close_date":0,"is_closed":false});
            send(args, state, now, cid()?, content)
        }
        "message.sendContact" => {
            let id = arg_int(args, "userId", Some(1), None, None)?;
            let user = state
                .users
                .get(&id)
                .filter(|u| !yes(&u["bot"]))
                .ok_or("a contact card is for a person Telebar knows")?;
            let name = user["name"].as_str().unwrap_or("");
            let (first, last) = name.split_once(' ').unwrap_or((name, ""));
            send(
                args,
                state,
                now,
                cid()?,
                json!({"@type":"inputMessageContact","contact":{"@type":"contact","phone_number":user["phone"],"first_name":first,"last_name":last,"vcard":"","user_id":number(id)}}),
            )
        }
        "message.sendLocation" => {
            let latitude = args["latitude"]
                .as_f64()
                .filter(|v| (-90.0..=90.0).contains(v));
            let longitude = args["longitude"]
                .as_f64()
                .filter(|v| (-180.0..=180.0).contains(v));
            if latitude.is_none() || longitude.is_none() {
                return Err(
                    "a location is a latitude from -90 to 90 and a longitude from -180 to 180"
                        .into(),
                );
            }
            send(
                args,
                state,
                now,
                cid()?,
                json!({"@type":"inputMessageLocation","location":{"@type":"location","latitude":latitude,"longitude":longitude,"horizontal_accuracy":0}}),
            )
        }
        "message.sendGif" => {
            let chat = cid()?;
            if !args["queryId"].is_null() {
                let raw = args["queryId"]
                    .as_str()
                    .ok_or("queryId and resultId come from gifs.search")?;
                let digits = raw.strip_prefix('-').unwrap_or(raw);
                let id = raw.parse::<i64>().ok();
                let result = args["resultId"]
                    .as_str()
                    .filter(|s| (1..=64).contains(&s.chars().count()));
                if digits.is_empty()
                    || digits.len() > 19
                    || !digits.bytes().all(|c| c.is_ascii_digit())
                    || id.is_none()
                    || result.is_none()
                {
                    return Err("queryId and resultId come from gifs.search".into());
                }
                return native(
                    json!({"@type":"sendInlineQueryResultMessage","chat_id":chat,"topic_id":topic(args)?,"reply_to":reply(args)?,"options":options(args,state,now)?,"query_id":id,"result_id":result,"hide_via_bot":true}),
                    Answer::Message,
                );
            }
            let animation = json!({"@type":"inputAnimation","animation":{"@type":"inputFileId","id":number(arg_int(args,"fileId",Some(1),Some(i32::MAX as i128),None)?)},"thumbnail":null,"added_sticker_file_ids":[],"duration":number(arg_int(args,"duration",Some(0),Some(86400),Some(0))?),"width":number(arg_int(args,"width",Some(0),Some(10000),Some(0))?),"height":number(arg_int(args,"height",Some(0),Some(10000),Some(0))?)});
            send(
                args,
                state,
                now,
                chat,
                json!({"@type":"inputMessageAnimation","animation":animation,"caption":null,"show_caption_above_media":false,"has_spoiler":false}),
            )
        }
        "message.translate" => {
            let to = if crate::wire::truthy(&args["to"]) {
                args["to"].as_str().unwrap_or("")
            } else {
                language.split('-').next().unwrap_or("en")
            };
            if !valid_language(to) {
                return Err("to is a language code, like en or pt-BR".into());
            }
            native(
                json!({"@type":"translateMessageText","chat_id":cid()?,"message_id":mid()?,"to_language_code":to,"tone":""}),
                Answer::Translate(to.into()),
            )
        }
        _ => commands::plan(command, args, state),
    }
}
fn valid_language(to: &str) -> bool {
    let mut parts = to.split('-');
    let first = parts.next().unwrap_or("");
    if !(2..=3).contains(&first.len()) || !first.bytes().all(|c| c.is_ascii_lowercase()) {
        return false;
    }
    let second = parts.next();
    parts.next().is_none()
        && second.is_none_or(|s| {
            (2..=8).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_alphanumeric())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_send_preserves_original_input_and_schedule_is_explicit() {
        let state = State::default();
        let args = json!({"chatId":42,"text":"original","sendAt":110});
        let original = args.clone();
        let mut execute = |_: &Value| None;
        assert!(plan("message.send", &args, &state, 100.1, "en", &mut execute).is_err());
        assert_eq!(args, original);
        let Action::Native { query, .. } =
            plan("message.send", &args, &state, 99.0, "en", &mut execute).unwrap()
        else {
            panic!("native")
        };
        assert_eq!(query["input_message_content"]["text"]["text"], "original");
        assert_eq!(query["options"]["scheduling_state"]["send_date"], 110);
        assert!(options(&json!({"silent":null}), &state, 100.0).is_err());
        assert_eq!(options(&json!({}), &state, 100.0).unwrap(), Value::Null);
        assert!(
            !options(&json!({"silent":false}), &state, 100.0)
                .unwrap()
                .is_null()
        );
    }
    #[test]
    fn markdown_entities_keep_utf16_mentions() {
        let mut execute = |_: &Value| {
            Some(
                json!({"@type":"formattedText","text":"😀 Person","entities":[{"@type":"textEntity","offset":3,"length":6,"type":{"@type":"textEntityTypeTextUrl","url":"tg://user?id=123"}}]}),
            )
        };
        let text = typed_text("fixture", 4096, "a message", &mut execute).unwrap();
        assert_eq!(text["entities"][0]["offset"], 3);
        assert_eq!(
            text["entities"][0]["type"],
            json!({"@type":"textEntityTypeMentionName","user_id":123})
        );
    }
}
