//! Chat/member information and shared-media commands.
use crate::{
    commands::{Action, Answer, Result, arg_int, arg_str, topic},
    projection::{array, integer, number, text, typed, yes},
    state::State,
};
use serde_json::{Value, json};

pub const MEDIA_FILTERS: [(&str, &str); 6] = [
    ("photos", "searchMessagesFilterPhotoAndVideo"),
    ("files", "searchMessagesFilterDocument"),
    ("links", "searchMessagesFilterUrl"),
    ("voice", "searchMessagesFilterVoiceAndVideoNote"),
    ("music", "searchMessagesFilterAudio"),
    ("gifs", "searchMessagesFilterAnimation"),
];
#[derive(Debug)]
pub enum ChatAnswer {
    Info(i128),
    Members {
        chat: i128,
        basic: bool,
        query: String,
        offset: usize,
        limit: usize,
    },
    Mentions(i128),
    BotCommands {
        chat: i128,
        user: i128,
    },
    Media {
        chat: i128,
        kind: String,
    },
    GroupCreated,
    Reactions,
    CustomEmoji,
}
fn members(event: &Value, state: &State) -> Vec<Value> {
    array(&event["members"], 200)
        .map(|m| state.member_view(m))
        .filter(|m| !m.is_null())
        .collect()
}
impl ChatAnswer {
    pub fn complete(&self, event: &Value, state: &State) -> Result<Value> {
        Ok(match self {
            Self::Info(cid) => {
                let chat = state.chats.get(cid).ok_or("unknown chat")?;
                let mut out = json!({"chatId":number(*cid),"kind":chat["kind"]});
                let uid = integer(&chat["userId"], 0);
                let basic = integer(&chat["basicGroupId"], 0);
                let supergroup = integer(&chat["supergroupId"], 0);
                if uid != 0 {
                    let user = state.users.get(&uid).unwrap_or(&Value::Null);
                    let secret = state
                        .secrets
                        .get(&integer(&chat["secretChatId"], 0))
                        .unwrap_or(&Value::Null);
                    let fields = json!({"userId":number(uid),"username":user.get("username").unwrap_or(&json!("")),"phone":user.get("phone").unwrap_or(&json!("")),"bio":crate::settings_projection::formatted_view(&event["bio"]),"botDescription":text(&typed(&event["bot_info"],"botInfo")["description"],600),"commonGroups":number(integer(&event["group_in_common_count"],0).max(0)),"keyHash":secret.get("keyHash").unwrap_or(&json!(""))});
                    merge(&mut out, fields);
                } else {
                    let invite = text(
                        &typed(&event["invite_link"], "chatInviteLink")["invite_link"],
                        2048,
                    );
                    let description = text(&event["description"], 600);
                    if basic != 0 {
                        let members = members(event, state);
                        merge(
                            &mut out,
                            json!({"description":description,"memberCount":members.len(),"members":members,"canGetMembers":true,"inviteLink":invite}),
                        );
                    } else if supergroup != 0 {
                        let known = state.supergroups.get(&supergroup).unwrap_or(&Value::Null);
                        let count = integer(&event["member_count"], 0).max(0);
                        merge(
                            &mut out,
                            json!({"description":description,"memberCount":number(if count==0{integer(&known["memberCount"],0)}else{count}),"members":[],"canGetMembers":yes(&event["can_get_members"]),"inviteLink":invite,"username":known.get("username").unwrap_or(&json!("")),"linkedChatId":number(integer(&event["linked_chat_id"],0))}),
                        );
                    }
                }
                out
            }
            Self::Members {
                chat,
                basic,
                query,
                offset,
                limit,
            } => {
                let mut found = members(event, state);
                let total;
                if *basic {
                    let wanted = query.to_lowercase();
                    found.retain(|m| {
                        m["name"]
                            .as_str()
                            .is_some_and(|s| s.to_lowercase().contains(&wanted))
                    });
                    total = found.len() as i128;
                    found = found.into_iter().skip(*offset).take(*limit).collect();
                } else {
                    total = integer(&event["total_count"], 0).max(found.len() as i128);
                }
                json!({"chatId":number(*chat),"members":found,"total":number(total)})
            }
            Self::Mentions(chat) => {
                json!({"chatId":number(*chat),"people":members(event,state).into_iter().filter(|m|m["type"]=="user"&&integer(&m["id"],0)!=state.me_id).take(20).map(|m|{let u=state.users.get(&integer(&m["id"],0)).unwrap_or(&Value::Null);json!({"userId":m["id"],"name":m["name"],"username":u.get("username").unwrap_or(&json!("")),"bot":m["bot"]})}).collect::<Vec<_>>()})
            }
            Self::BotCommands { chat, user } => {
                let mut out = Vec::new();
                let mut listed = |id, commands: &Value| {
                    let u = state.users.get(&id).unwrap_or(&Value::Null);
                    for c in array(commands, 100) {
                        let c = typed(c, "botCommand");
                        if let Some(name) = c["command"].as_str().filter(|s| {
                            !s.is_empty()
                                && s.len() <= 32
                                && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
                        }) {
                            out.push(json!({"command":name,"description":text(&c["description"],600),"botId":number(id),"bot":u.get("username").unwrap_or(&json!(""))}));
                        }
                    }
                };
                if *user != 0 {
                    listed(*user, &typed(&event["bot_info"], "botInfo")["commands"]);
                } else {
                    for g in array(&event["bot_commands"], 100) {
                        let g = typed(g, "botCommands");
                        listed(integer(&g["bot_user_id"], 0), &g["commands"]);
                    }
                    out.truncate(100);
                }
                json!({"chatId":number(*chat),"commands":out})
            }
            Self::Media { chat, kind } => {
                json!({"chatId":number(*chat),"filter":kind,"messages":array(&event["messages"],100).map(|m|state.message(m)).filter(|m|!m.is_null()).collect::<Vec<_>>(),"total":number(integer(&event["total_count"],0).max(0)),"nextFromMessageId":number(integer(&event["next_from_message_id"],0))})
            }
            Self::GroupCreated => {
                json!({"chatId":number(integer(&event["chat_id"],0)),"notAdded":array(&event["failed_to_add_members"]["failed_to_add_members"],200).count()})
            }
            Self::Reactions => {
                json!({"total":number(integer(&event["total_count"],0).max(0)),"nextOffset":text(&event["next_offset"],512),"reactions":array(&event["reactions"],100).filter_map(|r|{let r=typed(r,"addedReaction");let(sender,name)=state.sender(&r["sender_id"]);if integer(&sender["id"],0)==0{return None;}Some(json!({"type":sender["type"],"id":sender["id"],"name":if name.is_empty(){"Someone"}else{&name},"emoji":if r["type"]["@type"]=="reactionTypeEmoji"{text(&r["type"]["emoji"],16)}else{String::new()},"date":number(integer(&r["date"],0).max(0)),"outgoing":yes(&r["is_outgoing"])}))}).collect::<Vec<_>>()})
            }
            Self::CustomEmoji => {
                json!({"emoji":array(&event["stickers"],200).filter_map(|s|{let s=typed(s,"sticker");let full=typed(&s["full_type"],"stickerFullTypeCustomEmoji");if full.is_null(){return None;}let mut v=crate::projection::media_for("sticker",&json!({"sticker":s}),&state.roots);if v.is_null(){return None;}v["id"]=json!(integer(&full["custom_emoji_id"],0).to_string());Some(v)}).collect::<Vec<_>>()})
            }
        })
    }
}
fn merge(target: &mut Value, value: Value) {
    if let (Some(out), Value::Object(values)) = (target.as_object_mut(), value) {
        out.extend(values);
    }
}
fn native(query: Value, answer: ChatAnswer) -> Action {
    Action::Native {
        query,
        answer: Answer::ChatDetail(answer),
    }
}
fn query<'a>(args: &'a Value, max: usize, error: &str) -> Result<&'a str> {
    match args.get("query") {
        None => Ok(""),
        Some(v) => v
            .as_str()
            .filter(|s| s.chars().count() <= max)
            .ok_or_else(|| error.into()),
    }
}
fn group_request(chat: &Value) -> Option<Value> {
    let uid = integer(&chat["userId"], 0);
    let basic = integer(&chat["basicGroupId"], 0);
    let supergroup = integer(&chat["supergroupId"], 0);
    if uid != 0 {
        Some(json!({"@type":"getUserFullInfo","user_id":number(uid)}))
    } else if basic != 0 {
        Some(json!({"@type":"getBasicGroupFullInfo","basic_group_id":number(basic)}))
    } else if supergroup != 0 {
        Some(json!({"@type":"getSupergroupFullInfo","supergroup_id":number(supergroup)}))
    } else {
        None
    }
}
pub fn plan(command: &str, args: &Value, state: &State) -> Result<Action> {
    Ok(match command {
        "chat.info" => {
            let cid = arg_int(args, "chatId", None, None, None)?;
            let chat = state.chats.get(&cid).ok_or("unknown chat")?;
            if let Some(q) = group_request(chat) {
                native(q, ChatAnswer::Info(cid))
            } else {
                Action::Reply(json!({"chatId":number(cid),"kind":chat["kind"]}))
            }
        }
        "chat.mentions" => {
            let cid = arg_int(args, "chatId", None, None, None)?;
            let q = query(args, 64, "query is text")?;
            let chat = state.chats.get(&cid);
            if chat.is_none_or(|c| {
                integer(&c["basicGroupId"], 0) == 0 && integer(&c["supergroupId"], 0) == 0
            }) {
                Action::Reply(json!({"chatId":number(cid),"people":[]}))
            } else {
                native(
                    json!({"@type":"searchChatMembers","chat_id":number(cid),"query":q,"limit":20,"filter":{"@type":"chatMembersFilterMention","topic_id":topic(args)?}}),
                    ChatAnswer::Mentions(cid),
                )
            }
        }
        "chat.commands" => {
            let cid = arg_int(args, "chatId", None, None, None)?;
            let chat = state.chats.get(&cid).ok_or("unknown chat")?;
            let uid = integer(&chat["userId"], 0);
            let basic = integer(&chat["basicGroupId"], 0);
            let supergroup = integer(&chat["supergroupId"], 0);
            let bot = uid != 0
                && integer(&chat["secretChatId"], 0) == 0
                && state.users.get(&uid).is_some_and(|u| yes(&u["bot"]));
            if bot || basic != 0 || supergroup != 0 {
                native(
                    group_request(chat).ok_or("unknown chat")?,
                    ChatAnswer::BotCommands {
                        chat: cid,
                        user: if bot { uid } else { 0 },
                    },
                )
            } else {
                Action::Reply(json!({"chatId":number(cid),"commands":[]}))
            }
        }
        "chat.members" => {
            let cid = arg_int(args, "chatId", None, None, None)?;
            let chat = state
                .chats
                .get(&cid)
                .filter(|c| {
                    integer(&c["basicGroupId"], 0) != 0 || integer(&c["supergroupId"], 0) != 0
                })
                .ok_or("that chat has no members to list")?;
            let query = query(args, 256, "query is text")?.to_owned();
            let offset = arg_int(args, "offset", Some(0), Some(100000), Some(0))?;
            let limit = arg_int(args, "limit", Some(1), Some(200), Some(50))?;
            let sid = integer(&chat["supergroupId"], 0);
            let basic = sid == 0;
            let q = if basic {
                json!({"@type":"getBasicGroupFullInfo","basic_group_id":chat["basicGroupId"]})
            } else {
                let filter = if query.is_empty() {
                    json!({"@type":"supergroupMembersFilterRecent"})
                } else {
                    json!({"@type":"supergroupMembersFilterSearch","query":query})
                };
                json!({"@type":"getSupergroupMembers","supergroup_id":number(sid),"filter":filter,"offset":number(offset),"limit":number(limit)})
            };
            native(
                q,
                ChatAnswer::Members {
                    chat: cid,
                    basic,
                    query,
                    offset: offset as usize,
                    limit: limit as usize,
                },
            )
        }
        "chat.media" => {
            let cid = arg_int(args, "chatId", None, None, None)?;
            let kind = args["filter"].as_str().unwrap_or("");
            let filter = MEDIA_FILTERS
                .iter()
                .find(|(key, _)| *key == kind)
                .map(|(_, v)| *v)
                .ok_or_else(|| {
                    format!(
                        "filter is one of {}",
                        MEDIA_FILTERS.map(|(k, _)| k).join(", ")
                    )
                })?;
            native(
                json!({"@type":"searchChatMessages","chat_id":number(cid),"topic_id":null,"query":"","sender_id":null,"from_message_id":number(arg_int(args,"fromMessageId",Some(0),None,Some(0))?),"offset":0,"limit":number(arg_int(args,"limit",Some(1),Some(100),Some(50))?),"filter":{"@type":filter}}),
                ChatAnswer::Media {
                    chat: cid,
                    kind: kind.into(),
                },
            )
        }
        "group.create" | "channel.create" => {
            let title = arg_str(args, "title", 128, 1)?.trim();
            if title.is_empty() {
                return Err("a new chat needs a name".into());
            }
            if command == "group.create" {
                let ids = args.get("userIds").cloned().unwrap_or(json!([]));
                let ids = ids
                    .as_array()
                    .filter(|a| a.len() <= 200)
                    .ok_or("userIds is a list of at most 200 people")?;
                if ids.iter().any(|v| v.as_i64().is_none_or(|i| i <= 0)) {
                    return Err("userIds is a list of at most 200 people".into());
                }
                native(
                    json!({"@type":"createNewBasicGroupChat","user_ids":ids,"title":title,"message_auto_delete_time":0}),
                    ChatAnswer::GroupCreated,
                )
            } else {
                let description = args
                    .get("description")
                    .map_or(Some(""), Value::as_str)
                    .filter(|s| s.chars().count() <= 255)
                    .ok_or("a description is text of at most 255 characters")?;
                Action::Native {
                    query: json!({"@type":"createNewSupergroupChat","title":title,"is_forum":false,"is_channel":args.get("channel").unwrap_or(&Value::Bool(true))==&Value::Bool(true),"description":description,"location":null,"message_auto_delete_time":0,"for_import":false}),
                    answer: Answer::ChatId,
                }
            }
        }
        "message.reactions" => {
            let offset = args
                .get("offset")
                .map_or(Some(""), Value::as_str)
                .filter(|s| s.chars().count() <= 512)
                .ok_or("offset is the text a previous answer gave")?;
            native(
                json!({"@type":"getMessageAddedReactions","chat_id":number(arg_int(args,"chatId",None,None,None)?),"message_id":number(arg_int(args,"messageId",Some(1),None,None)?),"reaction_type":null,"offset":offset,"limit":50}),
                ChatAnswer::Reactions,
            )
        }
        "customEmoji.get" => {
            let ids = args["ids"]
                .as_array()
                .filter(|a| (1..=200).contains(&a.len()))
                .ok_or("ids is a list of custom emoji ids, as text")?;
            let mut values = Vec::new();
            for id in ids {
                let raw = id
                    .as_str()
                    .ok_or("ids is a list of custom emoji ids, as text")?;
                let digits = raw.strip_prefix('-').unwrap_or(raw);
                if digits.is_empty()
                    || digits.len() > 19
                    || !digits.bytes().all(|c| c.is_ascii_digit())
                {
                    return Err("ids is a list of custom emoji ids, as text".into());
                }
                let n = raw
                    .parse::<i64>()
                    .map_err(|_| "ids is a list of custom emoji ids, as text")?;
                values.push(n);
            }
            native(
                json!({"@type":"getCustomEmojiStickers","custom_emoji_ids":values}),
                ChatAnswer::CustomEmoji,
            )
        }
        _ => return Err("migration command is not implemented".into()),
    })
}
