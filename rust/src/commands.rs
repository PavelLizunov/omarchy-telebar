//! Command validation and TDLib continuations, independent of sockets and profiles.
use crate::{
    projection::{array, integer, number, text, typed, yes},
    state::State,
};
use serde_json::{Value, json};

pub type Result<T> = std::result::Result<T, String>;
const INT64_MAX: i128 = i64::MAX as i128;
const INT32_MAX: i128 = i32::MAX as i128;

pub fn arg_int(
    args: &Value,
    name: &str,
    low: Option<i128>,
    high: Option<i128>,
    default: Option<i128>,
) -> Result<i128> {
    let value = match args.get(name) {
        None => default.ok_or_else(|| format!("{name} must be an integer"))?,
        Some(Value::Number(n)) => {
            let s = n.to_string();
            if s.contains(['.', 'e', 'E']) {
                return Err(format!("{name} must be an integer"));
            }
            s.parse::<i128>()
                .map_err(|_| format!("{name} is out of range"))?
        }
        _ => return Err(format!("{name} must be an integer")),
    };
    if !(-INT64_MAX..=INT64_MAX).contains(&value)
        || low.is_some_and(|l| value < l)
        || high.is_some_and(|h| value > h)
    {
        return Err(format!("{name} is out of range"));
    }
    Ok(value)
}
pub fn arg_str<'a>(args: &'a Value, name: &str, max: usize, min: usize) -> Result<&'a str> {
    let value = args[name]
        .as_str()
        .filter(|s| (min..=max).contains(&s.chars().count()));
    value.ok_or_else(|| format!("{name} must be text of {min}-{max} characters"))
}
pub fn arg_list(args: &Value) -> Result<(String, Value)> {
    let key = args.get("list").unwrap_or(&Value::Null);
    let key = if args.get("list").is_none() {
        "main"
    } else {
        key.as_str().unwrap_or("")
    };
    match key {
        "main" => Ok((key.into(), json!({"@type":"chatListMain"}))),
        "archive" => Ok((key.into(), json!({"@type":"chatListArchive"}))),
        _ => {
            let raw = key.strip_prefix("folder:").unwrap_or("");
            let id = raw.parse::<i32>().ok();
            if raw.is_empty()
                || raw.len() > 10
                || raw.starts_with('0')
                || !raw.bytes().all(|c| c.is_ascii_digit())
                || id.is_none_or(|v| v <= 0)
            {
                return Err("list is main, archive or folder:<id>".into());
            }
            Ok((
                key.into(),
                json!({"@type":"chatListFolder","chat_folder_id":id}),
            ))
        }
    }
}
pub fn arg_ids(args: &Value, name: &str) -> Result<Value> {
    let Some(ids) = args[name]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 100)
    else {
        return Err(format!("{name} must be a list of 1-100 message ids"));
    };
    if ids.iter().any(|v| {
        !matches!(v, Value::Number(_))
            || !v.to_string().bytes().all(|c| c.is_ascii_digit())
            || v == &json!(0)
    }) {
        return Err(format!("{name} must contain message ids"));
    }
    Ok(json!(ids))
}
pub fn topic(args: &Value) -> Result<Value> {
    if !args["threadId"].is_null() {
        if !args["topicId"].is_null() {
            return Err("a topic or a thread, not both".into());
        }
        return Ok(
            json!({"@type":"messageTopicThread","message_thread_id":number(arg_int(args,"threadId",Some(1),None,None)?)}),
        );
    }
    if args["topicId"].is_null() {
        return Ok(Value::Null);
    }
    Ok(
        json!({"@type":"messageTopicForum","forum_topic_id":number(arg_int(args,"topicId",Some(1),Some(INT32_MAX),None)?)}),
    )
}
pub fn reply(args: &Value) -> Result<Value> {
    if args["replyToMessageId"].is_null() {
        return Ok(Value::Null);
    }
    Ok(
        json!({"@type":"inputMessageReplyToMessage","message_id":number(arg_int(args,"replyToMessageId",Some(1),None,None)?),"quote":null,"checklist_task_id":0,"poll_option_id":""}),
    )
}
fn required_bool(args: &Value, name: &str, error: &str) -> Result<bool> {
    args[name].as_bool().ok_or_else(|| error.into())
}
fn optional_text<'a>(args: &'a Value, name: &str, max: usize, error: &str) -> Result<&'a str> {
    match args.get(name) {
        None => Ok(""),
        Some(v) => v
            .as_str()
            .filter(|s| s.chars().count() <= max)
            .ok_or_else(|| error.into()),
    }
}

#[derive(Debug)]
pub enum Action {
    Playback(crate::playback::Start),
    Reply(Value),
    Work(crate::jobs::Work),
    Recorded {
        token: String,
        content: Value,
        options: Value,
    },
    CredentialsSaved,
    StartupPrepared(crate::startup::Prepared),
    DiscardRecorded(String),
    PrepareRecorded {
        recording: Option<crate::recorded_send::Recording>,
        preparation: Box<crate::recording_jobs::Preparation>,
    },
    Batch(crate::batch::Batch),
    Native {
        query: Value,
        answer: Answer,
    },
    AccountNative {
        account: String,
        query: Value,
        answer: Answer,
    },
}
#[derive(Debug)]
pub enum Answer {
    Playback(crate::playback::Selection),
    Empty,
    Recorded(String),
    Export(crate::export::Gate),
    Cache(crate::sticker_cache::CacheAnswer),
    ChatDetail(crate::chat_commands::ChatAnswer),
    Constant(Value),
    Continue(Continuation),
    Chat(i128),
    ChatId,
    Profile(Value),
    FolderId,
    Folder(i128),
    Scope,
    Count,
    Join(i128),
    Message,
    Markdown,
    LinkDetail(crate::links::LinkAnswer),
    WrappedMessage,
    MessageId,
    Messages(Value),
    SearchChats {
        query: String,
        server: bool,
    },
    SearchMessages {
        query: String,
        chat: Option<i128>,
    },
    OldestMessage,
    Archive(Value),
    Link,
    LinkPreview,
    Vote,
    Seconds,
    Field {
        wire: &'static str,
        native: &'static str,
    },
    File,
    Rich,
    Proxies,
    Contacts,
    Blocked(i128),
    Callback,
    Stickers(Option<String>),
    StickerSets,
    Reactions {
        all: bool,
    },
    Viewers,
    Properties,
    Poll,
    Proxy,
    Privacy,
    Sessions,
    Password,
    Topics(i128),
    Story,
    Translate(String),
    Gifs,
    GifSearch,
    Thread {
        chat: i128,
        message: i128,
    },
}
#[derive(Debug)]
pub struct Completion {
    pub result: Value,
    pub followups: Vec<Value>,
    pub next: Option<Box<Action>>,
}
impl Answer {
    pub fn complete_with(
        &self,
        event: &Value,
        state: &State,
        execute: &mut dyn FnMut(&Value) -> Option<Value>,
    ) -> Result<Completion> {
        if !matches!(self, Self::Markdown) {
            return self.complete(event, state);
        }
        let content = &event["content"];
        let value = typed(
            if content["@type"] == "messageText" {
                &content["text"]
            } else {
                &content["caption"]
            },
            "formattedText",
        );
        let text = value["text"].as_str().unwrap_or("");
        let written = if text.is_empty() {
            None
        } else {
            execute(&json!({"@type":"getMarkdownText","text":value}))
        };
        let text = written
            .as_ref()
            .and_then(|v| v["text"].as_str())
            .unwrap_or(text)
            .chars()
            .take(8192)
            .collect::<String>();
        Ok(Completion {
            result: json!({"text":text}),
            followups: Vec::new(),
            next: None,
        })
    }
    pub fn failed(&self, event: &Value) -> Result<Value> {
        if let Self::LinkDetail(answer) = self
            && let Some(result) = answer.failed()
        {
            return Ok(result);
        }
        if matches!(self, Self::Continue(Continuation::GifBot { .. })) {
            return Err("GIF search is not available".into());
        }
        Err(text(&event["message"], 200))
    }
    pub fn complete(&self, event: &Value, state: &State) -> Result<Completion> {
        if let Self::Playback(selection) = self {
            return Ok(Completion {
                result: Value::Null,
                followups: Vec::new(),
                next: Some(Box::new(selection.complete(event, state)?)),
            });
        }
        if let Self::Cache(answer) = self {
            return Ok(Completion {
                result: Value::Null,
                followups: Vec::new(),
                next: Some(Box::new(answer.complete(event, state)?)),
            });
        }
        if let Self::Export(gate) = self {
            return Ok(Completion {
                result: Value::Null,
                followups: Vec::new(),
                next: Some(Box::new(gate.complete(event, state)?)),
            });
        }
        if let Self::LinkDetail(answer) = self {
            let action = answer.complete(event, state)?;
            return Ok(Completion {
                result: Value::Null,
                followups: Vec::new(),
                next: Some(Box::new(action)),
            });
        }
        let messages = || {
            array(&event["messages"], 100)
                .map(|m| state.message(m))
                .filter(|m| !m.is_null())
                .collect::<Vec<_>>()
        };
        let mut followups = Vec::new();
        let result = match self {
            Self::Continue(next) => {
                return Ok(Completion {
                    result: Value::Null,
                    followups: Vec::new(),
                    next: Some(Box::new(next.resume(event, state)?)),
                });
            }
            Self::ChatDetail(answer) => answer.complete(event, state)?,
            Self::LinkDetail(_) => unreachable!("link completion handled above"),
            Self::Export(_) => unreachable!("export completion handled above"),
            Self::Cache(_) => unreachable!("cache completion handled above"),
            Self::Playback(_) => unreachable!("playback completion handled above"),
            Self::Recorded(_) => state.message(event),
            Self::Empty => json!({}),
            Self::Constant(v) => v.clone(),
            Self::Chat(id) => json!({"chat":state.chat_view(*id)}),
            Self::ChatId => json!({"chatId":number(integer(&event["id"],0))}),
            Self::Profile(user) => crate::settings_commands::profile(user, event, state),
            Self::Folder(id) => crate::settings_projection::folder_view(event, *id),
            Self::Scope => crate::settings_projection::scope_view(event),
            Self::Count => number(integer(&event["count"], 0).max(0)),
            Self::FolderId => json!({"id":number(integer(&typed(event,"chatFolderInfo")["id"],0))}),
            Self::Join(fallback) => {
                let id = integer(&event["chat_id"], 0);
                json!({"chatId":number(if id==0{*fallback}else{id}),"state":match event["@type"].as_str().unwrap_or(""){"chatJoinResultSuccess"=>"joined","chatJoinResultRequestSent"=>"requested","chatJoinResultGuardBotApprovalRequired"=>"guardBot","chatJoinResultDeclined"=>"declined",_=>""}})
            }
            Self::Message => state.message(event),
            Self::Markdown => {
                let c = &event["content"];
                json!({"text":text(&typed(if c["@type"]=="messageText"{&c["text"]}else{&c["caption"]},"formattedText")["text"],8192)})
            }
            Self::WrappedMessage => json!({"message":state.message(event)}),
            Self::MessageId => {
                json!({"messageId":number(integer(&typed(event,"message")["id"],0))})
            }
            Self::Messages(base) => {
                let mut out = base.clone();
                out["messages"] = json!(messages());
                out
            }
            Self::SearchChats { query, server } => {
                json!({"query":query,"server":server,"chats":array(&event["chat_ids"],100).map(|i|state.chat_view(integer(i,0))).filter(|c|!c.is_null()).collect::<Vec<_>>()})
            }
            Self::SearchMessages { query, chat } => {
                let mut out = json!({"query":query,"chatId":number(chat.unwrap_or(0)),"messages":messages(),"total":number(integer(&event["total_count"],0))});
                if chat.is_some() {
                    out["nextFromMessageId"] = number(integer(&event["next_from_message_id"], 0));
                } else {
                    out["nextOffset"] = json!(text(&event["next_offset"], 512));
                }
                out
            }
            Self::OldestMessage => {
                json!({"messageId":number(array(&event["messages"],100).map(|m|integer(&typed(m,"message")["id"],0)).filter(|i|*i!=0).min().unwrap_or(0))})
            }
            Self::Archive(list) => {
                followups.push(json!({"@type":"loadChats","chat_list":list,"limit":100}));
                json!({})
            }
            Self::Link => {
                json!({"link":text(&event["link"],2048),"public":yes(&event["is_public"])})
            }
            Self::LinkPreview => {
                json!({"preview":crate::content_projection::link_preview(event,&state.roots)})
            }
            Self::Vote => json!({"count":array(&event["messages"],100).count()}),
            Self::Seconds => json!({"seconds":event["seconds"].as_f64().unwrap_or(0.0).max(0.0)}),
            Self::Field { wire, native } => {
                let mut v = json!({});
                v[wire] = number(integer(&event[native], 0).max(0));
                v
            }
            Self::File => {
                let f = crate::projection::file_view(event, &state.roots);
                if f.is_null() { json!({}) } else { f }
            }
            Self::Rich => crate::rich_projection::rich_message(event, &state.roots),
            Self::Proxies => {
                json!({"proxies":array(&event["proxies"],100).map(crate::settings_projection::proxy_view).filter(|v|!v.is_null()).collect::<Vec<_>>(),"connection":state.connection})
            }
            Self::Contacts => {
                json!({"contacts":array(&event["user_ids"],5000).filter_map(|i|state.users.get(&integer(i,0))).map(|u|json!({"userId":u["id"],"name":u["name"],"username":u["username"],"bot":u["bot"],"status":u["status"]})).collect::<Vec<_>>()})
            }
            Self::Blocked(offset) => {
                json!({"total":number(integer(&event["total_count"],0).max(0)),"offset":number(*offset),"senders":array(&event["senders"],100).filter_map(|s|{let(sender,name)=state.sender(s);if integer(&sender["id"],0)==0{return None;}let mut out=sender;out["name"]=json!(if name.is_empty(){if out["type"]=="user"{"Deleted account"}else{"Chat"}}else{&name});Some(out)}).collect::<Vec<_>>()})
            }
            Self::Callback => {
                json!({"text":text(&event["text"],200),"alert":yes(&event["show_alert"]),"url":text(&event["url"],2048)})
            }
            Self::Stickers(set) => {
                let mut v = json!({"stickers":array(&event["stickers"],300).map(|s|crate::projection::media_for("sticker",&json!({"sticker":s}),&state.roots)).filter(|v|!v.is_null()).collect::<Vec<_>>()});
                if let Some(id) = set {
                    v["id"] = json!(id);
                    v["title"] = json!(text(&event["title"], 128));
                    v["installed"] = json!(yes(&event["is_installed"]));
                }
                v
            }
            Self::StickerSets => {
                json!({"sets":array(&event["sets"],200).filter_map(|s|{let s=typed(s,"stickerSetInfo");if s.is_null(){return None;}let cover=array(&s["covers"],300).map(|c|crate::projection::media_for("sticker",&json!({"sticker":c}),&state.roots)).find(|v|!v.is_null()).unwrap_or(Value::Null);Some(json!({"id":integer(&s["id"],0).to_string(),"title":text(&s["title"],128),"size":number(integer(&s["size"],0).max(0)),"cover":cover}))}).collect::<Vec<_>>()})
            }
            Self::Reactions { all } => {
                let mut emoji = Vec::new();
                for group in ["top_reactions", "recent_reactions", "popular_reactions"] {
                    for r in array(&event[group], 100) {
                        let r = typed(r, "availableReaction");
                        let value = if r["type"]["@type"] == "reactionTypeEmoji" {
                            text(&r["type"]["emoji"], 16)
                        } else {
                            String::new()
                        };
                        if !value.is_empty() && !emoji.contains(&value) && !yes(&r["needs_premium"])
                        {
                            emoji.push(value);
                        }
                    }
                }
                if !all {
                    emoji.truncate(48);
                }
                json!({"emoji":emoji})
            }
            Self::Viewers => {
                json!({"viewers":array(&event["viewers"],100).filter_map(|v|{let v=typed(v,"messageViewer");let id=integer(&v["user_id"],0);if id<=0{return None;}let name=state.user_name(id);Some(json!({"userId":number(id),"name":if name.is_empty(){"Someone"}else{&name},"date":number(integer(&v["view_date"],0).max(0))}))}).collect::<Vec<_>>()})
            }
            Self::Properties => {
                let mut v = json!({});
                for (key, name) in [
                    ("can_be_deleted_for_all_users", "canDeleteForAll"),
                    ("can_be_deleted_only_for_self", "canDeleteForMe"),
                    ("can_be_edited", "canEdit"),
                    ("can_be_forwarded", "canForward"),
                    ("can_be_pinned", "canPin"),
                    ("can_be_copied", "canCopy"),
                    ("can_be_replied", "canReply"),
                    ("can_get_link", "canGetLink"),
                    ("can_be_saved", "canSave"),
                    ("can_get_message_thread", "canGetThread"),
                    ("can_get_viewers", "canGetViewers"),
                ] {
                    v[name] = json!(yes(&event[key]));
                }
                v
            }
            Self::Poll => json!({"poll":crate::content_projection::poll_view(event)}),
            Self::Proxy => json!({"proxy":crate::settings_projection::proxy_view(event)}),
            Self::Privacy => crate::settings_projection::privacy_view(event),
            Self::Password => crate::settings_projection::password_view(event),
            Self::Sessions => {
                let mut sessions: Vec<_> = array(&event["sessions"], 100)
                    .map(crate::settings_projection::session_view)
                    .filter(|v| !v.is_null())
                    .collect();
                sessions.sort_by_key(|v| (!yes(&v["current"]), -integer(&v["lastActive"], 0)));
                json!({"sessions":sessions,"inactiveDays":number(integer(&event["inactive_session_ttl_days"],0).max(0))})
            }
            Self::Topics(chat) => {
                json!({"chatId":number(*chat),"topics":array(&event["topics"],100).map(|t|state.topic_view(t)).filter(|t|!t.is_null()).collect::<Vec<_>>(),"total":number(integer(&event["total_count"],0).max(0)),"next":{"offsetDate":number(integer(&event["next_offset_date"],0)),"offsetMessageId":number(integer(&event["next_offset_message_id"],0)),"offsetTopicId":number(integer(&event["next_offset_forum_topic_id"],0))}})
            }
            Self::Story => json!({"story":state.story_view(event)}),
            Self::Translate(to) => {
                let mut v = crate::settings_projection::formatted_view(event);
                v["to"] = json!(to);
                v
            }
            Self::GifSearch => {
                json!({"queryId":integer(&event["inline_query_id"],0).to_string(),"nextOffset":text(&event["next_offset"],512),"results":array(&event["results"],100).filter_map(|r|{let r=typed(r,"inlineQueryResultAnimation");let id=text(&r["id"],64);let gif=crate::projection::media_for("gif",&json!({"animation":r["animation"]}),&state.roots);if id.is_empty()||gif.is_null(){None}else{Some(json!({"id":id,"gif":gif}))}}).collect::<Vec<_>>()})
            }
            Self::Gifs => {
                json!({"gifs":array(&event["animations"],300).map(|a|crate::projection::media_for("gif",&json!({"animation":a}),&state.roots)).filter(|v|!v.is_null()).collect::<Vec<_>>()})
            }
            Self::Thread { chat, message } => {
                let cid = integer(&event["chat_id"], 0);
                let tid = integer(&event["message_thread_id"], 0);
                if cid == 0 || tid <= 0 {
                    return Err("that message has no comments or replies".into());
                }
                json!({"chatId":number(cid),"threadId":number(tid),"fromChatId":number(*chat),"fromMessageId":number(*message),"messages":array(&event["messages"],10).map(|m|state.message(m)).filter(|m|!m.is_null()).collect::<Vec<_>>(),"replies":crate::content_projection::replies_view(&event["reply_info"]),"unread":number(integer(&event["unread_message_count"],0).max(0)),"draft":crate::settings_projection::draft_text(&event["draft_message"]),"chat":state.chat_view(cid)})
            }
        };
        Ok(Completion {
            result,
            followups,
            next: None,
        })
    }
}
fn native(query: Value, answer: Answer) -> Action {
    Action::Native { query, answer }
}

/// Unknown handlers remain explicitly unsupported; no generic TDLib passthrough.
pub fn plan(command: &str, args: &Value, state: &State) -> Result<Action> {
    let id = || arg_int(args, "chatId", None, None, None).map(number);
    let mid = || arg_int(args, "messageId", Some(1), None, None).map(number);
    let empty = Answer::Empty;
    Ok(match command {
        "chats.list" => {
            let (key, _) = arg_list(args)?;
            Action::Reply(
                json!({"chats":state.chat_list(&key,arg_int(args,"limit",Some(1),Some(500),Some(200))? as usize)}),
            )
        }
        "chats.load" => {
            let limit = arg_int(args, "limit", Some(1), Some(500), Some(100))?;
            let (key, list) = arg_list(args)?;
            native(
                json!({"@type":"loadChats","chat_list":list,"limit":number(limit)}),
                Answer::Constant(json!({"list":key,"done":false})),
            )
        }
        "chat.open" => {
            let cid = id()?;
            native(
                json!({"@type":"openChat","chat_id":cid}),
                Answer::Chat(integer(&cid, 0)),
            )
        }
        "chat.close" => native(json!({"@type":"closeChat","chat_id":id()?}), empty),
        "chat.history" | "topic.history" | "thread.history" => {
            let cid = id()?;
            let (mut query, mut base) = if command == "chat.history" {
                (
                    json!({"@type":"getChatHistory","chat_id":cid,"only_local":false}),
                    json!({"chatId":cid}),
                )
            } else if command == "topic.history" {
                let tid = number(arg_int(args, "topicId", Some(1), Some(INT32_MAX), None)?);
                (
                    json!({"@type":"getForumTopicHistory","chat_id":cid,"forum_topic_id":tid}),
                    json!({"chatId":cid,"topicId":tid}),
                )
            } else {
                let tid = number(arg_int(args, "threadId", Some(1), None, None)?);
                (
                    json!({"@type":"getMessageThreadHistory","chat_id":cid,"message_thread_id":tid}),
                    json!({"chatId":cid,"threadId":tid}),
                )
            };
            query["from_message_id"] =
                number(arg_int(args, "fromMessageId", Some(0), None, Some(0))?);
            query["offset"] = number(arg_int(args, "offset", Some(-99), Some(0), Some(0))?);
            query["limit"] = number(arg_int(args, "limit", Some(1), Some(100), Some(50))?);
            base["messages"] = json!([]);
            native(query, Answer::Messages(base))
        }
        "chat.read" => {
            let t = topic(args)?;
            let source = if t.is_null() {
                "messageSourceChatHistory"
            } else if t["@type"] == "messageTopicForum" {
                "messageSourceForumTopicHistory"
            } else {
                "messageSourceMessageThreadHistory"
            };
            native(
                json!({"@type":"viewMessages","chat_id":id()?,"message_ids":arg_ids(args,"messageIds")?,"source":{"@type":source},"force_read":true}),
                empty,
            )
        }
        "chat.scheduled" => {
            let cid = id()?;
            native(
                json!({"@type":"getChatScheduledMessages","chat_id":cid}),
                Answer::Messages(json!({"chatId":cid})),
            )
        }
        "chat.pinned" => native(
            json!({"@type":"getChatPinnedMessage","chat_id":id()?}),
            Answer::WrappedMessage,
        ),
        "chat.leave" => native(json!({"@type":"leaveChat","chat_id":id()?}), empty),
        "chat.clearHistory" => {
            let remove = args
                .get("removeFromList")
                .unwrap_or(&Value::Bool(false))
                .as_bool();
            let revoke = args.get("revoke").unwrap_or(&Value::Bool(false)).as_bool();
            if remove.is_none() || revoke.is_none() {
                return Err("removeFromList and revoke are true or false".into());
            }
            native(
                json!({"@type":"deleteChatHistory","chat_id":id()?,"remove_from_chat_list":remove,"revoke":revoke}),
                empty,
            )
        }
        "chat.markUnread" => {
            let unread = required_bool(args, "unread", "unread is true or false")?;
            native(
                json!({"@type":"toggleChatIsMarkedAsUnread","chat_id":id()?,"is_marked_as_unread":unread}),
                empty,
            )
        }
        "chat.pin" => {
            let (_, list) = arg_list(args)?;
            let pinned = required_bool(args, "pinned", "pinned must be true or false")?;
            native(
                json!({"@type":"toggleChatIsPinned","chat_list":list,"chat_id":id()?,"is_pinned":pinned}),
                empty,
            )
        }
        "chat.archive" => {
            let archive = required_bool(args, "archived", "archived must be true or false")?;
            let list = json!({"@type":if archive{"chatListArchive"}else{"chatListMain"}});
            native(
                json!({"@type":"addChatToList","chat_id":id()?,"chat_list":list}),
                Answer::Archive(list),
            )
        }
        "chat.setSilent" => {
            let silent = required_bool(args, "silent", "silent is true or false")?;
            native(
                json!({"@type":"toggleChatDefaultDisableNotification","chat_id":id()?,"default_disable_notification":silent}),
                Answer::Constant(json!({"silent":silent})),
            )
        }
        "chat.setAutoDelete" => {
            let cid = id()?;
            let seconds = arg_int(args, "seconds", Some(0), Some(365 * 86400), None)?;
            if seconds % 86400 != 0
                && !state
                    .chats
                    .get(&integer(&cid, 0))
                    .is_some_and(|c| c["kind"] == "secret")
            {
                return Err("messages disappear after whole days, but in a secret chat".into());
            }
            native(
                json!({"@type":"setChatMessageAutoDeleteTime","chat_id":cid,"message_auto_delete_time":number(seconds)}),
                Answer::Constant(json!({"seconds":number(seconds)})),
            )
        }
        "chat.mute" => {
            let cid = id()?;
            let chat = state.chats.get(&integer(&cid, 0)).ok_or("unknown chat")?;
            let seconds = arg_int(args, "muteFor", Some(-1), Some(INT32_MAX), None)?;
            let mut settings = chat["notification"].clone();
            settings["use_default_mute_for"] = json!(seconds < 0);
            settings["mute_for"] = number(seconds.max(0));
            native(
                json!({"@type":"setChatNotificationSettings","chat_id":cid,"notification_settings":settings}),
                empty,
            )
        }
        "chat.join" => {
            let cid = id()?;
            if !state
                .chats
                .get(&integer(&cid, 0))
                .is_some_and(|c| integer(&c["supergroupId"], 0) != 0)
            {
                return Err("only public groups and channels can be joined".into());
            }
            native(
                json!({"@type":"joinChat","chat_id":cid}),
                Answer::Join(integer(&cid, 0)),
            )
        }
        "chats.search" => {
            let query = arg_str(args, "query", 256, 1)?.to_owned();
            let limit = arg_int(args, "limit", Some(1), Some(100), Some(30))?;
            let server = yes(&args["server"]);
            let public = yes(&args["public"]);
            let mut q = json!({"@type":if public{"searchPublicChats"}else if server{"searchChatsOnServer"}else{"searchChats"},"query":query,"type_filter":null});
            if !public {
                q["limit"] = number(limit);
            }
            native(q, Answer::SearchChats { query, server })
        }
        "messages.search" => {
            let query = arg_str(args, "query", 256, 1)?.to_owned();
            let limit = arg_int(args, "limit", Some(1), Some(100), Some(30))?;
            let chat = if args["chatId"].is_null() {
                None
            } else {
                Some(arg_int(args, "chatId", None, None, None)?)
            };
            let q = if let Some(chat) = chat {
                json!({"@type":"searchChatMessages","chat_id":number(chat),"topic_id":null,"query":query,"sender_id":null,"from_message_id":number(arg_int(args,"fromMessageId",Some(0),None,Some(0))?),"offset":0,"limit":number(limit),"filter":null})
            } else {
                let offset = optional_text(
                    args,
                    "offset",
                    512,
                    "offset is the text a previous search returned",
                )?;
                json!({"@type":"searchMessages","chat_list":null,"query":query,"offset":offset,"limit":number(limit),"filter":null,"chat_type_filter":null,"min_date":0,"max_date":0})
            };
            native(q, Answer::SearchMessages { query, chat })
        }
        "chat.nextMention" | "chat.nextReaction" => native(
            json!({"@type":"searchChatMessages","chat_id":id()?,"topic_id":topic(args)?,"query":"","sender_id":null,"from_message_id":0,"offset":0,"limit":100,"filter":{"@type":if command=="chat.nextMention"{"searchMessagesFilterUnreadMention"}else{"searchMessagesFilterUnreadReaction"}}}),
            Answer::OldestMessage,
        ),
        "chat.readMentions" | "chat.readReactions" => {
            let t = topic(args)?;
            let reaction = command == "chat.readReactions";
            let mut q = json!({"chat_id":id()?});
            if t.is_null() {
                q["@type"] = json!(if reaction {
                    "readAllChatReactions"
                } else {
                    "readAllChatMentions"
                });
            } else if t["@type"] == "messageTopicForum" {
                q["@type"] = json!(if reaction {
                    "readAllForumTopicReactions"
                } else {
                    "readAllForumTopicMentions"
                });
                q["forum_topic_id"] = t["forum_topic_id"].clone();
            } else {
                return Err(format!(
                    "Telegram reads {} for a whole chat or topic, not for one thread",
                    if reaction { "reactions" } else { "mentions" }
                ));
            }
            native(q, empty)
        }
        "chat.draft" => {
            let text = optional_text(
                args,
                "text",
                4096,
                "a draft is text of at most 4096 characters",
            )?;
            let draft = if text.trim().is_empty() {
                Value::Null
            } else {
                json!({"@type":"draftMessage","reply_to":reply(args)?,"date":0,"effect_id":0,"suggested_post_info":null,"content":{"@type":"draftMessageContentText","link_preview_options":null,"text":{"@type":"formattedText","text":text,"entities":[]}}})
            };
            native(
                json!({"@type":"setChatDraftMessage","chat_id":id()?,"topic_id":topic(args)?,"draft_message":draft}),
                empty,
            )
        }
        "message.properties" => native(
            json!({"@type":"getMessageProperties","chat_id":id()?,"message_id":mid()?}),
            Answer::Properties,
        ),
        "message.link" => native(
            json!({"@type":"getMessageLink","chat_id":id()?,"message_id":mid()?,"media_timestamp":0,"checklist_task_id":0,"poll_option_id":"","for_album":false,"in_message_thread":false}),
            Answer::Link,
        ),
        "message.pin" => {
            let cid = id()?;
            let message = mid()?;
            let pinned = required_bool(args, "pinned", "pinned must be true or false")?;
            let q = if pinned {
                json!({"@type":"pinChatMessage","chat_id":cid,"message_id":message,"disable_notification":!yes(&args["notify"]),"only_for_self":yes(&args["forSelf"])})
            } else {
                json!({"@type":"unpinChatMessage","chat_id":cid,"message_id":message})
            };
            native(q, empty)
        }
        "message.viewers" => native(
            json!({"@type":"getMessageViewers","chat_id":id()?,"message_id":mid()?}),
            Answer::Viewers,
        ),
        "reactions.available" => native(
            json!({"@type":"getMessageAvailableReactions","chat_id":id()?,"message_id":mid()?,"row_size":8}),
            Answer::Reactions {
                all: yes(&args["all"]),
            },
        ),
        "reaction.set" => {
            let cid = id()?;
            let message = mid()?;
            let emoji = arg_str(args, "emoji", 16, 1)?;
            let chosen = required_bool(args, "chosen", "chosen must be true or false")?;
            let mut q = json!({"@type":if chosen{"addMessageReaction"}else{"removeMessageReaction"},"chat_id":cid,"message_id":message,"reaction_type":{"@type":"reactionTypeEmoji","emoji":emoji}});
            if chosen {
                q["is_big"] = json!(false);
                q["update_recent_reactions"] = json!(true);
            }
            native(q, empty)
        }
        "poll.vote" => {
            let options = args["optionIds"]
                .as_array()
                .filter(|a| a.len() <= 12)
                .ok_or("optionIds is a list of option numbers")?;
            if options.iter().any(|v| v.as_u64().is_none_or(|i| i >= 12)) {
                return Err("optionIds is a list of option numbers".into());
            }
            native(
                json!({"@type":"setPollAnswer","chat_id":id()?,"message_id":mid()?,"option_ids":options}),
                empty,
            )
        }
        "chat.messageByDate" => native(
            json!({"@type":"getChatMessageByDate","chat_id":id()?,"date":number(arg_int(args,"date",Some(0),Some(INT32_MAX),None)?)}),
            Answer::MessageId,
        ),
        "thread.open" => {
            let cid = id()?;
            let message = mid()?;
            native(
                json!({"@type":"getMessageThread","chat_id":cid,"message_id":message}),
                Answer::Thread {
                    chat: integer(&cid, 0),
                    message: integer(&message, 0),
                },
            )
        }
        "topics.list" => {
            let cid = id()?;
            let query = optional_text(args, "query", 256, "query is text")?;
            native(
                json!({"@type":"getForumTopics","chat_id":cid,"query":query,"offset_date":number(arg_int(args,"offsetDate",Some(0),Some(INT32_MAX),Some(0))?),"offset_message_id":number(arg_int(args,"offsetMessageId",Some(0),None,Some(0))?),"offset_forum_topic_id":number(arg_int(args,"offsetTopicId",Some(0),Some(INT32_MAX),Some(0))?),"limit":number(arg_int(args,"limit",Some(1),Some(100),Some(50))?)}),
                Answer::Topics(integer(&cid, 0)),
            )
        }
        "user.chat" => native(
            json!({"@type":"createPrivateChat","user_id":number(arg_int(args,"userId",Some(1),None,None)?),"force":false}),
            Answer::ChatId,
        ),
        "secret.create" => native(
            json!({"@type":"createNewSecretChat","user_id":number(arg_int(args,"userId",Some(1),None,None)?)}),
            Answer::ChatId,
        ),
        "secret.close" => {
            let cid = id()?;
            let sid = state
                .chats
                .get(&integer(&cid, 0))
                .map(|c| integer(&c["secretChatId"], 0))
                .filter(|i| *i != 0)
                .ok_or("that is not a secret chat")?;
            native(
                json!({"@type":"closeSecretChat","secret_chat_id":number(sid)}),
                empty,
            )
        }
        "call.decline" => {
            let call = arg_int(args, "callId", Some(1), Some(INT32_MAX), None)?;
            native(
                json!({"@type":"discardCall","call_id":number(call),"is_disconnected":false,"invite_link":"","duration":0,"is_video":state.calls.get(&call).is_some_and(|c|yes(&c["video"])),"connection_id":0}),
                empty,
            )
        }
        "story.get" | "story.open" | "story.close" => {
            let cid = id()?;
            let story = arg_int(args, "storyId", Some(1), Some(INT32_MAX), None)?;
            let mut q = json!({"@type":match command{"story.get"=>"getStory","story.open"=>"openStory",_=>"closeStory"},"story_poster_chat_id":cid,"story_id":number(story)});
            let answer = if command == "story.get" {
                q["only_local"] = json!(false);
                Answer::Story
            } else {
                empty
            };
            native(q, answer)
        }
        "gifs.saved" => native(json!({"@type":"getSavedAnimations"}), Answer::Gifs),
        "sessions.list" => native(json!({"@type":"getActiveSessions"}), Answer::Sessions),
        "sessions.terminateOthers" => native(json!({"@type":"terminateAllOtherSessions"}), empty),
        "folder.delete" => native(
            json!({"@type":"deleteChatFolder","chat_folder_id":number(arg_int(args,"id",Some(1),Some(INT32_MAX),None)?),"leave_chat_ids":[]}),
            empty,
        ),
        "auth.phone" => {
            let phone = arg_str(args, "phone", 32, 5)?;
            let digits = phone.strip_prefix('+').unwrap_or(phone);
            if !(5..=31).contains(&digits.len())
                || !digits.as_bytes()[0].is_ascii_digit()
                || !digits
                    .bytes()
                    .all(|c| c.is_ascii_digit() || b" ()-".contains(&c))
            {
                return Err("that does not look like a phone number".into());
            }
            let phone: String = phone.chars().filter(|c| !" ()-".contains(*c)).collect();
            native(
                json!({"@type":"setAuthenticationPhoneNumber","phone_number":phone,"settings":null}),
                empty,
            )
        }
        "auth.code" => {
            let code = arg_str(args, "code", 10, 3)?;
            if !code.bytes().all(|c| c.is_ascii_digit()) {
                return Err("the code is digits only".into());
            }
            native(
                json!({"@type":"checkAuthenticationCode","code":code}),
                empty,
            )
        }
        "auth.password" => native(
            json!({"@type":"checkAuthenticationPassword","password":arg_str(args,"password",512,1)?}),
            empty,
        ),
        "auth.qr" => native(
            json!({"@type":"requestQrCodeAuthentication","other_user_ids":[]}),
            empty,
        ),
        "auth.logout" => native(json!({"@type":"logOut"}), empty),
        "message.rich" => native(
            json!({"@type":"getFullRichMessage","chat_id":id()?,"message_id":mid()?}),
            Answer::Rich,
        ),
        "message.delete" => {
            let revoke = args
                .get("revoke")
                .unwrap_or(&Value::Bool(true))
                .as_bool()
                .ok_or("revoke must be true or false")?;
            native(
                json!({"@type":"deleteMessages","chat_id":id()?,"message_ids":arg_ids(args,"messageIds")?,"revoke":revoke}),
                empty,
            )
        }
        "file.download" => native(
            json!({"@type":"downloadFile","file_id":number(arg_int(args,"fileId",Some(1),Some(INT32_MAX),None)?),"priority":number(arg_int(args,"priority",Some(1),Some(32),Some(16))?),"offset":0,"limit":0,"synchronous":false}),
            Answer::File,
        ),
        "proxies.list" => native(json!({"@type":"getProxies"}), Answer::Proxies),
        "proxy.enable" | "proxy.remove" => native(
            json!({"@type":if command=="proxy.enable"{"enableProxy"}else{"removeProxy"},"proxy_id":number(arg_int(args,"id",Some(1),Some(INT32_MAX),None)?)}),
            empty,
        ),
        "proxy.disable" => native(json!({"@type":"disableProxy"}), empty),
        "password.get" => native(json!({"@type":"getPasswordState"}), Answer::Password),
        "password.checkEmailCode" => {
            let code = arg_str(args, "code", 32, 1)?.trim();
            if code.is_empty() {
                return Err("the code from the email is needed".into());
            }
            native(
                json!({"@type":"checkRecoveryEmailAddressCode","code":code}),
                Answer::Password,
            )
        }
        "password.resendEmailCode" => native(
            json!({"@type":"resendRecoveryEmailAddressCode"}),
            Answer::Password,
        ),
        "account.ttl" => native(
            json!({"@type":"getAccountTtl"}),
            Answer::Field {
                wire: "days",
                native: "days",
            },
        ),
        "account.setTtl" => {
            let days = arg_int(args, "days", Some(30), Some(730), None)?;
            native(
                json!({"@type":"setAccountTtl","ttl":{"@type":"accountTtl","days":number(days)}}),
                Answer::Constant(json!({"days":number(days)})),
            )
        }
        "autoDelete.default" => native(
            json!({"@type":"getDefaultMessageAutoDeleteTime"}),
            Answer::Field {
                wire: "seconds",
                native: "time",
            },
        ),
        "autoDelete.setDefault" => {
            let seconds = arg_int(args, "seconds", Some(0), Some(365 * 86400), None)?;
            if seconds % 86400 != 0 {
                return Err("messages disappear after whole days".into());
            }
            native(
                json!({"@type":"setDefaultMessageAutoDeleteTime","message_auto_delete_time":{"@type":"messageAutoDeleteTime","time":number(seconds)}}),
                Answer::Constant(json!({"seconds":number(seconds)})),
            )
        }
        "contacts.list" => native(json!({"@type":"getContacts"}), Answer::Contacts),
        "contacts.search" => native(
            json!({"@type":"searchContacts","query":arg_str(args,"query",256,0)?,"limit":number(arg_int(args,"limit",Some(1),Some(100),Some(50))?)}),
            Answer::Contacts,
        ),
        "blocked.list" => {
            let offset = arg_int(args, "offset", Some(0), Some(100000), Some(0))?;
            native(
                json!({"@type":"getBlockedMessageSenders","block_list":{"@type":"blockListMain"},"offset":number(offset),"limit":100}),
                Answer::Blocked(offset),
            )
        }
        "blocked.unblock" => {
            let id = arg_int(args, "id", None, None, None)?;
            let sender = match args["type"].as_str() {
                Some("user") if id > 0 => json!({"@type":"messageSenderUser","user_id":number(id)}),
                Some("chat") if id < 0 => json!({"@type":"messageSenderChat","chat_id":number(id)}),
                _ => return Err("a blocked sender is a user or a chat".into()),
            };
            native(
                json!({"@type":"setMessageSenderBlockList","sender_id":sender,"block_list":null}),
                empty,
            )
        }
        "profile.setName" => {
            let first = arg_str(args, "firstName", 64, 1)?.trim();
            let last = arg_str(args, "lastName", 64, 0)?.trim();
            if first.is_empty() {
                return Err("a first name is needed".into());
            }
            if first.chars().chain(last.chars()).any(|c| c < ' ') {
                return Err("a name is one line".into());
            }
            native(
                json!({"@type":"setName","first_name":first,"last_name":last}),
                empty,
            )
        }
        "profile.setBio" => {
            let bio = arg_str(args, "bio", 140, 0)?;
            if bio.chars().any(|c| c < ' ') {
                return Err("a bio is one line".into());
            }
            native(json!({"@type":"setBio","bio":bio.trim()}), empty)
        }
        "profile.setUsername" => {
            let name = arg_str(args, "username", 33, 0)?.trim();
            let name = name.strip_prefix('@').unwrap_or(name);
            if !name.is_empty() && !valid_username(name) {
                return Err(
                    "a username is 5-32 letters, digits and underscores, starting with a letter"
                        .into(),
                );
            }
            native(json!({"@type":"setUsername","username":name}), empty)
        }
        "username.chat" => {
            let name = arg_str(args, "username", 64, 1)?.trim_start_matches('@');
            if !valid_username(name) {
                return Err("that is not a username".into());
            }
            native(
                json!({"@type":"searchPublicChat","username":name}),
                Answer::ChatId,
            )
        }
        "bot.start" => {
            let cid = id()?;
            let user = state
                .chats
                .get(&integer(&cid, 0))
                .map(|c| integer(&c["userId"], 0))
                .filter(|i| *i != 0)
                .ok_or("that is not a chat with a bot")?;
            native(
                json!({"@type":"sendBotStartMessage","bot_user_id":number(user),"chat_id":cid,"parameter":arg_str(args,"parameter",64,0)?}),
                Answer::Message,
            )
        }
        "chat.joinLink" => native(
            json!({"@type":"joinChatByInviteLink","invite_link":arg_str(args,"link",2048,1)?}),
            Answer::Join(0),
        ),
        "session.terminate" => {
            let raw = args["id"].as_str().ok_or("id is a session id, as text")?;
            let digits = raw.strip_prefix('-').unwrap_or(raw);
            let value = raw.parse::<i64>().ok();
            if digits.is_empty()
                || digits.len() > 19
                || !digits.bytes().all(|c| c.is_ascii_digit())
                || value.is_none()
            {
                return Err("id is a session id, as text".into());
            }
            native(
                json!({"@type":"terminateSession","session_id":value}),
                empty,
            )
        }
        "stickers.recent" => native(
            json!({"@type":"getRecentStickers","is_attached":false}),
            Answer::Stickers(None),
        ),
        "stickers.favorites" => native(
            json!({"@type":"getFavoriteStickers"}),
            Answer::Stickers(None),
        ),
        "stickers.sets" => native(
            json!({"@type":"getInstalledStickerSets","sticker_type":{"@type":"stickerTypeRegular"}}),
            Answer::StickerSets,
        ),
        "stickers.set" | "stickers.install" => {
            let raw = args["setId"]
                .as_str()
                .ok_or("setId is a sticker set id, as text")?;
            let digits = raw.strip_prefix('-').unwrap_or(raw);
            if digits.is_empty() || digits.len() > 20 || !digits.bytes().all(|c| c.is_ascii_digit())
            {
                return Err("setId is a sticker set id, as text".into());
            }
            let sid = serde_json::from_str::<Value>(raw)
                .map_err(|_| "setId is a sticker set id, as text")?;
            if command == "stickers.set" {
                native(
                    json!({"@type":"getStickerSet","set_id":sid}),
                    Answer::Stickers(Some(raw.into())),
                )
            } else {
                let installed = required_bool(args, "installed", "installed is true or false")?;
                native(
                    json!({"@type":"changeStickerSet","set_id":sid,"is_installed":installed,"is_archived":false}),
                    empty,
                )
            }
        }
        "sticker.favorite" => {
            let favorite = required_bool(args, "favorite", "favorite is true or false")?;
            native(
                json!({"@type":if favorite{"addFavoriteSticker"}else{"removeFavoriteSticker"},"sticker":{"@type":"inputFileId","id":number(arg_int(args,"fileId",Some(1),Some(INT32_MAX),None)?)}}),
                empty,
            )
        }
        "button.callback" => {
            let data = args["data"]
                .as_str()
                .filter(|s| {
                    !s.is_empty()
                        && s.len() <= 128
                        && s.bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b"+/=".contains(&c))
                })
                .ok_or("that is not a button's data")?;
            native(
                json!({"@type":"getCallbackQueryAnswer","chat_id":id()?,"message_id":mid()?,"payload":{"@type":"callbackQueryPayloadData","data":data}}),
                Answer::Callback,
            )
        }
        "chat.action" => {
            let action = match args["action"].as_str() {
                Some("typing") => "chatActionTyping",
                Some("recordingVoice") => "chatActionRecordingVoiceNote",
                Some("recordingVideoNote") => "chatActionRecordingVideoNote",
                Some("choosingSticker") => "chatActionChoosingSticker",
                Some("uploadingPhoto") => "chatActionUploadingPhoto",
                Some("uploadingFile") => "chatActionUploadingDocument",
                Some("cancel") => "chatActionCancel",
                _ => return Err("that is not a chat action".into()),
            };
            native(
                json!({"@type":"sendChatAction","chat_id":id()?,"topic_id":topic(args)?,"business_connection_id":"","action":{"@type":action}}),
                empty,
            )
        }
        "notifications.set" => {
            let (scope, kind) = scope(args)?;
            let muted = args["muted"].clone();
            let preview = args["preview"].clone();
            if muted.is_null() && preview.is_null()
                || [&muted, &preview]
                    .iter()
                    .any(|v| !v.is_null() && !v.is_boolean())
            {
                return Err("muted or preview, true or false".into());
            }
            native(
                json!({"@type":"getScopeNotificationSettings","scope":{"@type":kind}}),
                Answer::Continue(Continuation::Notification {
                    scope,
                    kind,
                    muted,
                    preview,
                }),
            )
        }
        "privacy.set" => {
            let setting = args["setting"].as_str().unwrap_or("");
            let kind =
                privacy_kind(setting).ok_or("that is not a privacy setting Telebar shows")?;
            let base = args["base"].as_str().unwrap_or("");
            let choices = if setting == "findByPhone" {
                vec!["everybody", "contacts"]
            } else {
                vec!["everybody", "contacts", "nobody"]
            };
            if !choices.contains(&base) {
                return Err(format!("{setting} is one of {}", choices.join(", ")));
            }
            native(
                json!({"@type":"getUserPrivacySettingRules","setting":{"@type":kind}}),
                Answer::Continue(Continuation::Privacy {
                    setting: setting.into(),
                    kind: kind.into(),
                    base: base.into(),
                }),
            )
        }
        "proxy.addLink" => native(
            json!({"@type":"getInternalLinkType","link":arg_str(args,"link",2048,1)?}),
            Answer::Continue(Continuation::ProxyLink),
        ),
        "proxy.ping" => native(
            json!({"@type":"getProxies"}),
            Answer::Continue(Continuation::ProxyPing(arg_int(
                args,
                "id",
                Some(1),
                Some(INT32_MAX),
                None,
            )?)),
        ),
        "gifs.search" => {
            let chat = arg_int(args, "chatId", None, None, None)?;
            let query = arg_str(args, "query", 256, 0)?;
            let offset = optional_text(
                args,
                "offset",
                512,
                "offset is the text a previous search returned",
            )?;
            native(
                json!({"@type":"searchPublicChat","username":"gif"}),
                Answer::Continue(Continuation::GifBot {
                    chat,
                    query: query.into(),
                    offset: offset.into(),
                }),
            )
        }
        "link.open" => return crate::links::plan(args),
        "message.markdown" => native(
            json!({"@type":"getMessage","chat_id":id()?,"message_id":mid()?}),
            Answer::Markdown,
        ),
        "profile.get" | "profile.deletePhoto" => native(
            json!({"@type":"getMe"}),
            Answer::Continue(Continuation::Profile {
                delete: command == "profile.deletePhoto",
            }),
        ),
        "notifications.get" | "privacy.get" | "folders.get" | "chat.mediaCounts" => {
            return crate::batch::plan(command, args, state);
        }
        "folder.save" | "folders.reorder" | "proxy.add" | "password.set" => {
            return crate::settings_commands::plan(command, args, state);
        }
        _ => return crate::chat_commands::plan(command, args, state),
    })
}

fn privacy_kind(setting: &str) -> Option<&'static str> {
    Some(match setting {
        "status" => "userPrivacySettingShowStatus",
        "photo" => "userPrivacySettingShowProfilePhoto",
        "phone" => "userPrivacySettingShowPhoneNumber",
        "findByPhone" => "userPrivacySettingAllowFindingByPhoneNumber",
        "bio" => "userPrivacySettingShowBio",
        "birthdate" => "userPrivacySettingShowBirthdate",
        "forwards" => "userPrivacySettingShowLinkInForwardedMessages",
        "calls" => "userPrivacySettingAllowCalls",
        "invites" => "userPrivacySettingAllowChatInvites",
        _ => return None,
    })
}
fn scope(args: &Value) -> Result<(String, String)> {
    let scope = args["scope"].as_str().unwrap_or("");
    let kind = match scope {
        "private" => "notificationSettingsScopePrivateChats",
        "groups" => "notificationSettingsScopeGroupChats",
        "channels" => "notificationSettingsScopeChannelChats",
        _ => return Err("scope is private, groups or channels".into()),
    };
    Ok((scope.into(), kind.into()))
}

fn valid_username(name: &str) -> bool {
    (5..=32).contains(&name.len())
        && name.as_bytes()[0].is_ascii_alphabetic()
        && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

#[derive(Debug)]
pub enum Continuation {
    Profile {
        delete: bool,
    },
    GifBot {
        chat: i128,
        query: String,
        offset: String,
    },
    Notification {
        scope: String,
        kind: String,
        muted: Value,
        preview: Value,
    },
    Privacy {
        setting: String,
        kind: String,
        base: String,
    },
    ProxyLink,
    ProxyPing(i128),
}
impl Continuation {
    fn resume(&self, event: &Value, _state: &State) -> Result<Action> {
        Ok(match self {
            Self::GifBot {
                chat,
                query,
                offset,
            } => {
                let bot = integer(&event["type"]["user_id"], 0);
                if bot == 0 {
                    return Err("GIF search is not available".into());
                }
                native(
                    json!({"@type":"getInlineQueryResults","bot_user_id":number(bot),"chat_id":number(*chat),"user_location":null,"query":query,"offset":offset}),
                    Answer::GifSearch,
                )
            }
            Self::Profile { delete } => {
                if *delete {
                    let photo = integer(&typed(&event["profile_photo"], "profilePhoto")["id"], 0);
                    if photo == 0 {
                        return Err("you have no profile photo".into());
                    }
                    native(
                        json!({"@type":"deleteProfilePhoto","profile_photo_id":number(photo)}),
                        Answer::Empty,
                    )
                } else {
                    let uid = integer(&event["id"], 0);
                    if uid <= 0 {
                        return Err("Telegram did not say who you are".into());
                    }
                    native(
                        json!({"@type":"getUserFullInfo","user_id":number(uid)}),
                        Answer::Profile(event.clone()),
                    )
                }
            }
            Self::Notification {
                scope,
                kind,
                muted,
                preview,
            } => {
                let settings = crate::settings_projection::scope_settings(event, muted, preview);
                let mut result = crate::settings_projection::scope_view(&settings);
                result["scope"] = json!(scope);
                native(
                    json!({"@type":"setScopeNotificationSettings","scope":{"@type":kind},"notification_settings":settings}),
                    Answer::Constant(result),
                )
            }
            Self::Privacy {
                setting,
                kind,
                base,
            } => {
                let rules = crate::settings_projection::privacy_rules(event, base)
                    .map_err(str::to_owned)?;
                let mut result = crate::settings_projection::privacy_view(&rules);
                result["setting"] = json!(setting);
                native(
                    json!({"@type":"setUserPrivacySettingRules","setting":{"@type":kind},"rules":rules}),
                    Answer::Constant(result),
                )
            }
            Self::ProxyLink => {
                if event["@type"] != "internalLinkTypeProxy" {
                    return Err("That is not a proxy link".into());
                }
                native(
                    json!({"@type":"addProxy","proxy":typed(&event["proxy"],"proxy"),"enable":true,"comment":""}),
                    Answer::Proxy,
                )
            }
            Self::ProxyPing(id) => {
                let proxy = array(&event["proxies"], 100)
                    .map(|p| typed(p, "addedProxy"))
                    .find(|p| integer(&p["id"], 0) == *id)
                    .ok_or("There is no such proxy")?;
                native(
                    json!({"@type":"pingProxy","proxy":proxy["proxy"]}),
                    Answer::Seconds,
                )
            }
        })
    }
}

/// Development-only parity seam; does not execute native requests.
pub fn project(value: &Value, roots: &[String]) -> Value {
    let mut state = State::new(roots.to_vec());
    for u in array(&value["updates"], 50000) {
        state.apply(u);
    }
    let mut execute = |_: &Value| value.get("executeAnswer").cloned();
    match crate::sending::plan(
        value["command"].as_str().unwrap_or(""),
        &value["args"],
        &state,
        value["now"].as_f64().unwrap_or(0.0),
        value["language"].as_str().unwrap_or("en"),
        &mut execute,
    ) {
        Err(error) => json!({"error":error}),
        Ok(Action::Reply(result)) => json!({"result":result,"queries":[]}),
        Ok(Action::Playback(_))
        | Ok(Action::Work(_))
        | Ok(Action::Recorded { .. })
        | Ok(Action::PrepareRecorded { .. })
        | Ok(Action::DiscardRecorded(_))
        | Ok(Action::CredentialsSaved)
        | Ok(Action::StartupPrepared(_)) => {
            json!({"error":"fixture cannot execute helper jobs"})
        }
        Ok(Action::AccountNative { .. }) => {
            json!({"error":"fixture requires registered account routing"})
        }
        Ok(Action::Batch(batch)) => {
            let (queries, result) = crate::batch::fixture(batch, &value["answers"], &state);
            match result {
                Ok(result) => json!({"result":result,"queries":queries}),
                Err(error) => json!({"error":error,"queries":queries}),
            }
        }
        Ok(action @ Action::Native { .. }) => {
            let mut action = action;
            let mut queries = Vec::new();
            let answers = value["answers"].as_array();
            for step in 0..8 {
                match action {
                    Action::Reply(result) => return json!({"result":result,"queries":queries}),
                    Action::AccountNative { .. } => {
                        return json!({"error":"fixture requires registered account routing","queries":queries});
                    }
                    Action::Playback(_)
                    | Action::Work(_)
                    | Action::Recorded { .. }
                    | Action::PrepareRecorded { .. }
                    | Action::DiscardRecorded(_)
                    | Action::CredentialsSaved
                    | Action::StartupPrepared(_) => {
                        return json!({"error":"fixture cannot execute helper jobs","queries":queries});
                    }
                    Action::Batch(batch) => {
                        let (next, result) =
                            crate::batch::fixture(batch, &value["answers"], &state);
                        queries.extend(next);
                        return match result {
                            Ok(result) => json!({"result":result,"queries":queries}),
                            Err(error) => json!({"error":error,"queries":queries}),
                        };
                    }
                    Action::Native { query, answer } => {
                        queries.push(query);
                        let event = answers
                            .and_then(|a| a.get(step))
                            .unwrap_or(&value["answer"]);
                        if event["@type"] == "error" {
                            return match answer.failed(event) {
                                Ok(result) => json!({"result":result,"queries":queries}),
                                Err(error) => json!({"error":error,"queries":queries}),
                            };
                        }
                        match answer.complete_with(event, &state, &mut execute) {
                            Err(error) => return json!({"error":error,"queries":queries}),
                            Ok(done) => {
                                queries.extend(done.followups);
                                if let Some(next) = done.next {
                                    action = *next;
                                } else {
                                    return json!({"result":done.result,"queries":queries});
                                }
                            }
                        }
                    }
                }
            }
            json!({"error":"fixture continuation exceeded limit","queries":queries})
        }
    }
}
