//! Telegram link classification without opening an external application.
use crate::{
    commands::{Action, Answer, Result, arg_str},
    projection::{integer, number, text, typed},
    state::State,
};
use serde_json::{Value, json};
#[derive(Debug)]
pub enum LinkAnswer {
    Classify(String),
    Chat(Option<String>),
    Message,
    Invite(String),
}
pub fn telegram(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("tg://") {
        return true;
    }
    let host = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"));
    host.is_some_and(|v| {
        [
            "t.me/",
            "telegram.me/",
            "telegram.dog/",
            "www.t.me/",
            "www.telegram.me/",
            "www.telegram.dog/",
        ]
        .iter()
        .any(|prefix| v.starts_with(prefix))
    })
}
fn native(query: Value, answer: LinkAnswer) -> Action {
    Action::Native {
        query,
        answer: Answer::LinkDetail(answer),
    }
}
pub fn plan(args: &Value) -> Result<Action> {
    let url = arg_str(args, "url", 2048, 1)?;
    let lower = url.to_ascii_lowercase();
    if url.chars().any(|c| c <= ' ')
        || !["https://", "http://", "tg://"]
            .iter()
            .any(|p| lower.starts_with(p))
    {
        return Err("only web and Telegram links open".into());
    }
    if !telegram(url) {
        return Ok(Action::Reply(json!({"kind":"external","url":url})));
    }
    Ok(native(
        json!({"@type":"getInternalLinkType","link":url}),
        LinkAnswer::Classify(url.into()),
    ))
}
impl LinkAnswer {
    pub fn failed(&self) -> Option<Value> {
        if let Self::Classify(url) = self {
            Some(json!({"kind":"external","url":url}))
        } else {
            None
        }
    }
    pub fn complete(&self, event: &Value, _state: &State) -> Result<Action> {
        Ok(match self {
            Self::Classify(url) => match event["@type"].as_str() {
                Some("internalLinkTypePublicChat") => native(
                    json!({"@type":"searchPublicChat","username":text(&event["chat_username"],64)}),
                    Self::Chat(None),
                ),
                Some("internalLinkTypeBotStart") => native(
                    json!({"@type":"searchPublicChat","username":text(&event["bot_username"],64)}),
                    Self::Chat(Some(text(&event["start_parameter"], 64))),
                ),
                Some("internalLinkTypeMessage") => native(
                    json!({"@type":"getMessageLinkInfo","url":text(&event["url"],2048)}),
                    Self::Message,
                ),
                Some("internalLinkTypeProxy") => {
                    let proxy = typed(&event["proxy"], "proxy");
                    let kind = match proxy["type"]["@type"].as_str() {
                        Some("proxyTypeSocks5") => "socks5",
                        Some("proxyTypeHttp") => "http",
                        Some("proxyTypeMtproto") => "mtproto",
                        _ => "",
                    };
                    Action::Reply(
                        json!({"kind":"proxy","link":url,"server":text(&proxy["server"],253),"port":number(integer(&proxy["port"],0).max(0)),"type":kind}),
                    )
                }
                Some("internalLinkTypeChatInvite") => {
                    let link = text(&event["invite_link"], 2048);
                    native(
                        json!({"@type":"checkChatInviteLink","invite_link":link}),
                        Self::Invite(link),
                    )
                }
                _ => Action::Reply(json!({"kind":"external","url":url})),
            },
            Self::Chat(parameter) => {
                let mut result = json!({"kind":"chat","chatId":number(integer(&event["id"],0))});
                if let Some(parameter) = parameter {
                    result["botStart"] = json!(parameter);
                }
                Action::Reply(result)
            }
            Self::Message => Action::Reply(
                json!({"kind":"chat","chatId":number(integer(&event["chat_id"],0)),"messageId":number(integer(&typed(&event["message"],"message")["id"],0))}),
            ),
            Self::Invite(link) => Action::Reply(
                json!({"kind":"invite","link":link,"title":text(&event["title"],256),"members":number(integer(&event["member_count"],0).max(0)),"chatId":number(integer(&event["chat_id"],0))}),
            ),
        })
    }
}
