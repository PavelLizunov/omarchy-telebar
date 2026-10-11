use crate::{
    content_projection as body,
    projection::{
        array, file_view, formatted, integer, list_key, media_for, minithumbnail, number, tag,
        text, thumbnail, typed, yes,
    },
    settings_projection as setting,
};
use indexmap::IndexMap;
use serde_json::{Value, json};

#[derive(Default)]
pub struct State {
    pub roots: Vec<String>,
    pub retained: crate::recorded_send::Retained,
    pub chats: IndexMap<i128, Value>,
    pub users: IndexMap<i128, Value>,
    pub basic_groups: IndexMap<i128, Value>,
    pub supergroups: IndexMap<i128, Value>,
    pub secrets: IndexMap<i128, Value>,
    pub calls: IndexMap<i128, Value>,
    pub stories: IndexMap<i128, Value>,
    pub me_id: i128,
    pub connection: String,
    pub folders: Vec<Value>,
    pub main_position: i128,
    file_marks: IndexMap<i128, Value>,
    scopes: IndexMap<String, bool>,
}
impl State {
    pub fn new(roots: Vec<String>) -> Self {
        Self {
            roots,
            ..Self::default()
        }
    }
    pub fn user_name(&self, id: i128) -> String {
        self.users
            .get(&id)
            .map(|v| text(&v["name"], 256))
            .unwrap_or_default()
    }
    pub fn sender(&self, value: &Value) -> (Value, String) {
        match tag(value) {
            "messageSenderUser" => {
                let id = integer(&value["user_id"], 0);
                (json!({"type":"user","id":number(id)}), self.user_name(id))
            }
            "messageSenderChat" => {
                let id = integer(&value["chat_id"], 0);
                (
                    json!({"type":"chat","id":number(id)}),
                    self.chats
                        .get(&id)
                        .map(|c| text(&c["title"], 256))
                        .unwrap_or_default(),
                )
            }
            _ => (json!({"type":"unknown","id":0}), String::new()),
        }
    }
    pub fn message(&self, value: &Value) -> Value {
        let m = typed(value, "message");
        if m.is_null() {
            return Value::Null;
        }
        let (sender, name) = self.sender(&m["sender_id"]);
        let sending = match tag(&m["sending_state"]) {
            "messageSendingStatePending" => json!("pending"),
            "messageSendingStateFailed" => json!("failed"),
            _ => Value::Null,
        };
        let mut content = body::content(&m["content"], &self.roots);
        if content["kind"] == "service" {
            content["text"] =
                json!(self.service_text(&m["content"], &sender, &name, integer(&m["chat_id"], 0)));
        }
        let interaction = body::reactions_view(&m["interaction_info"]);
        let album = integer(&m["media_album_id"], 0);
        let reply = typed(&m["reply_to"], "messageReplyToMessage");
        json!({"id":number(integer(&m["id"],0)),"chatId":number(integer(&m["chat_id"],0)),"date":number(integer(&m["date"],0)),"editDate":number(integer(&m["edit_date"],0)),"outgoing":yes(&m["is_outgoing"]),"pinned":yes(&m["is_pinned"]),"sender":sender,"senderName":name,"sending":sending,"replyTo":if reply.is_null(){Value::Null}else{json!({"chatId":number(integer(&reply["chat_id"],0)),"messageId":number(integer(&reply["message_id"],0))})},"content":content,"forward":self.forward_view(&m["forward_info"]),"albumId":if album==0{String::new()}else{album.to_string()},"reactions":interaction[0],"canSeeReactions":yes(&m["interaction_info"]["reactions"]["can_get_added_reactions"]),"views":interaction[1],"markup":body::reply_markup(&m["reply_markup"]),"topicId":setting::topic_id(&m["topic_id"]),"threadId":setting::thread_id(&m["topic_id"]),"replies":body::replies_view(&m["interaction_info"]["reply_info"]),"sendAt":setting::scheduled_at(&m["scheduling_state"])})
    }
    fn forward_view(&self, value: &Value) -> Value {
        let f = typed(value, "messageForwardInfo");
        if f.is_null() {
            return Value::Null;
        }
        let origin = &f["origin"];
        let mut name = match tag(origin) {
            "messageOriginUser" => self.user_name(integer(&origin["sender_user_id"], 0)),
            "messageOriginHiddenUser" => text(&origin["sender_name"], 128),
            "messageOriginChat" | "messageOriginChannel" => {
                let field = if tag(origin) == "messageOriginChat" {
                    "sender_chat_id"
                } else {
                    "chat_id"
                };
                let name = self
                    .chats
                    .get(&integer(&origin[field], 0))
                    .map(|c| text(&c["title"], 256))
                    .unwrap_or_default();
                let signature = text(&origin["author_signature"], 128);
                if signature.is_empty() {
                    name
                } else if name.is_empty() {
                    signature
                } else {
                    format!("{name} ({signature})")
                }
            }
            _ => String::new(),
        };
        if name.is_empty() {
            name = "Unknown".into();
        }
        json!({"name":name,"date":number(integer(&f["date"],0))})
    }
    fn service_text(&self, c: &Value, sender: &Value, actor: &str, chat_id: i128) -> String {
        let who = if actor.is_empty() { "Someone" } else { actor };
        let actor_id = if sender["type"] == "user" {
            integer(&sender["id"], 0)
        } else {
            0
        };
        let place = if self
            .chats
            .get(&chat_id)
            .is_some_and(|c| c["kind"] == "channel")
        {
            "channel"
        } else {
            "group"
        };
        let user = |id| {
            let name = self.user_name(id);
            if name.is_empty() {
                "someone".into()
            } else {
                name
            }
        };
        match tag(c) {
            "messageChatAddMembers" => {
                let ids: Vec<_> = array(&c["member_user_ids"], 20)
                    .map(|v| integer(v, 0))
                    .collect();
                if ids == [actor_id] {
                    format!("{who} joined the {place}")
                } else if ids.is_empty() {
                    format!("{who} added members")
                } else {
                    format!(
                        "{who} added {}",
                        ids.into_iter().map(user).collect::<Vec<_>>().join(", ")
                    )
                }
            }
            "messageChatJoinByLink" => format!("{who} joined the {place} with an invite link"),
            "messageChatJoinByRequest" => format!("{who} joined the {place}"),
            "messageChatDeleteMember" => {
                let id = integer(&c["user_id"], 0);
                if id == actor_id {
                    format!("{who} left the {place}")
                } else {
                    format!("{who} removed {}", user(id))
                }
            }
            "messageChatChangeTitle" => {
                format!("{who} renamed the {place} to “{}”", text(&c["title"], 256))
            }
            "messageChatChangePhoto" => format!("{who} changed the {place} photo"),
            "messageChatDeletePhoto" => format!("{who} removed the {place} photo"),
            "messagePinMessage" => format!("{who} pinned a message"),
            "messageBasicGroupChatCreate" | "messageSupergroupChatCreate" => {
                format!("{who} created the {place} “{}”", text(&c["title"], 256))
            }
            "messageChatUpgradeTo" | "messageChatUpgradeFrom" => {
                "The group was upgraded to a supergroup".into()
            }
            "messageScreenshotTaken" => format!("{who} took a screenshot"),
            "messageContactRegistered" => format!("{who} joined Telegram"),
            "messageCall" => {
                let seconds = integer(&c["duration"], 0).max(0);
                let kind = if yes(&c["is_video"]) {
                    "Video call"
                } else {
                    "Call"
                };
                if seconds > 0 {
                    format!("{kind}, {}", setting::clock_duration(seconds))
                } else {
                    format!("Missed {}", kind.to_lowercase())
                }
            }
            "messageVideoChatStarted" => format!("{who} started a video chat"),
            "messageVideoChatEnded" => format!(
                "Video chat ended, {}",
                setting::clock_duration(integer(&c["duration"], 0).max(0))
            ),
            "messageChatSetMessageAutoDeleteTime" => {
                let seconds = integer(&c["message_auto_delete_time"], 0).max(0);
                if seconds > 0 {
                    format!(
                        "{who} set messages to disappear after {}",
                        setting::human_period(seconds)
                    )
                } else {
                    format!("{who} turned off disappearing messages")
                }
            }
            "messageForumTopicCreated" => format!("Topic “{}” was created", text(&c["name"], 256)),
            "messageForumTopicEdited" => {
                let name = text(&c["name"], 256);
                if name.is_empty() {
                    format!("{who} edited the topic")
                } else {
                    format!("{who} renamed the topic to “{name}”")
                }
            }
            "messageCustomServiceAction" => text(&c["text"], 200),
            "messageChatSetTheme" => format!("{who} changed the chat theme"),
            "messageChatSetBackground" => format!("{who} changed the chat background"),
            "messageChatBoost" => format!("{who} boosted the {place}"),
            _ => String::new(),
        }
    }
    pub fn preview(&self, value: &Value) -> Value {
        let m = self.message(value);
        if m.is_null() {
            return Value::Null;
        }
        json!({"id":m["id"],"date":m["date"],"outgoing":m["outgoing"],"senderName":m["senderName"],"text":body::preview_text(&m["content"])})
    }
    fn photo(&self, value: &Value) -> Value {
        let p = typed(value, "chatPhotoInfo");
        let small = file_view(&p["small"], &self.roots);
        if small.is_null() {
            Value::Null
        } else {
            json!({"file":small,"mini":minithumbnail(&p["minithumbnail"])})
        }
    }
    fn position(chat: &mut Value, value: &Value) {
        let p = typed(value, "chatPosition");
        let key = list_key(&p["list"]);
        let Some(key) = key.as_str() else {
            return;
        };
        let order = integer(&p["order"], 0);
        if let Some(positions) = chat["positions"].as_object_mut() {
            if order == 0 {
                positions.remove(key);
            } else {
                positions.insert(
                    key.into(),
                    json!({"order":number(order),"pinned":yes(&p["is_pinned"])}),
                );
            }
        }
    }
    fn new_chat(&mut self, value: &Value) -> i128 {
        let c = typed(value, "chat");
        let id = integer(&c["id"], 0);
        if id == 0 {
            return 0;
        }
        let kind_obj = &c["type"];
        let kind = match tag(kind_obj) {
            "chatTypePrivate" => "private",
            "chatTypeBasicGroup" => "group",
            "chatTypeSecret" => "secret",
            "chatTypeSupergroup" => {
                if yes(&kind_obj["is_channel"]) {
                    "channel"
                } else {
                    "group"
                }
            }
            _ => "unknown",
        };
        let mut chat = json!({"id":number(id),"title":text(&c["title"],256),"kind":kind,"userId":number(integer(&kind_obj["user_id"],0)),"basicGroupId":number(integer(&kind_obj["basic_group_id"],0)),"supergroupId":number(integer(&kind_obj["supergroup_id"],0)),"secretChatId":number(integer(&kind_obj["secret_chat_id"],0)),"hasScheduled":yes(&c["has_scheduled_messages"]),"markedUnread":yes(&c["is_marked_as_unread"]),"unread":number(integer(&c["unread_count"],0).max(0)),"mentions":number(integer(&c["unread_mention_count"],0).max(0)),"reactions":number(integer(&c["unread_reaction_count"],0).max(0)),"autoDelete":number(integer(&c["message_auto_delete_time"],0).max(0)),"silent":yes(&c["default_disable_notification"]),"canChangeInfo":yes(&c["permissions"]["can_change_info"]),"muteFor":number(integer(&c["notification_settings"]["mute_for"],0).max(0)),"positions":{},"lastReadInbox":number(integer(&c["last_read_inbox_message_id"],0)),"lastReadOutbox":number(integer(&c["last_read_outbox_message_id"],0)),"lastMessage":null,"photo":self.photo(&c["photo"]),"draft":setting::draft_text(&c["draft_message"]),"notification":setting::notification_settings(&c["notification_settings"])});
        for p in array(&c["positions"], 16) {
            Self::position(&mut chat, p);
        }
        self.chats.insert(id, chat);
        let preview = self.preview(&c["last_message"]);
        if let Some(chat) = self.chats.get_mut(&id) {
            chat["lastMessage"] = preview;
        }
        id
    }
    pub fn chat_view(&self, id: i128) -> Value {
        let Some(chat) = self.chats.get(&id) else {
            return Value::Null;
        };
        let main = &chat["positions"]["main"];
        let gid = integer(&chat["supergroupId"], 0);
        let basic = integer(&chat["basicGroupId"], 0);
        let group = if gid != 0 {
            self.supergroups.get(&gid)
        } else if basic != 0 {
            self.basic_groups.get(&basic)
        } else {
            None
        }
        .unwrap_or(&Value::Null);
        let uid = integer(&chat["userId"], 0);
        let user = self.users.get(&uid).unwrap_or(&Value::Null);
        let secret = self.secrets.get(&integer(&chat["secretChatId"], 0));
        let kind = chat["kind"].as_str().unwrap_or("");
        let can_delete = if matches!(kind, "private" | "secret") {
            uid != 0 && uid != self.me_id && uid != 777000
        } else {
            yes(&group["canChangeInfo"])
                || kind == "group" && group["status"] == "member" && yes(&chat["canChangeInfo"])
        };
        let muted = if !yes(&chat["notification"]["use_default_mute_for"]) {
            json!(integer(&chat["muteFor"], 0) > 0)
        } else {
            let scope = if kind == "channel" {
                "channels"
            } else if kind == "group" {
                "groups"
            } else {
                "private"
            };
            self.scopes
                .get(scope)
                .map(|v| json!(v))
                .unwrap_or(Value::Null)
        };
        let mut positions = json!({});
        let mut lists = Vec::new();
        if let Some(ps) = chat["positions"].as_object() {
            for (key, p) in ps {
                lists.push(key.clone());
                positions[key] =
                    json!({"order":integer(&p["order"],0).to_string(),"pinned":yes(&p["pinned"])});
            }
        }
        lists.sort();
        let mut out = json!({"id":chat["id"],"title":chat["title"],"photo":chat["photo"],"kind":chat["kind"],"userId":chat["userId"],"unread":chat["unread"],"mentions":chat["mentions"],"unreadReactions":chat["reactions"],"autoDelete":chat["autoDelete"],"silent":chat["silent"],"canSetAutoDelete":can_delete,"muted":muted,"order":integer(&main["order"],0).to_string(),"pinned":yes(&main["pinned"]),"archived":chat["positions"].get("archive").is_some(),"lists":lists,"positions":positions,"lastReadInbox":chat["lastReadInbox"],"lastReadOutbox":chat["lastReadOutbox"],"lastMessage":chat["lastMessage"],"draft":chat["draft"],"markedUnread":chat["markedUnread"],"status":user["status"],"bot":yes(&user["bot"]),"memberCount":group.get("memberCount").unwrap_or(&json!(0)),"myStatus":group.get("status").unwrap_or(&json!("")),"username":if gid!=0{group.get("username")}else{user.get("username")}.unwrap_or(&json!("")),"forum":yes(&group["forum"]),"supergroup":gid!=0,"joinToWrite":kind!="group" || group.get("joinToSend").is_none_or(yes),"hasScheduled":chat["hasScheduled"],"secret":null});
        if let Some(s) = secret {
            out["secret"] = json!({"state":s["state"],"outbound":s["outbound"]});
        }
        out
    }
    pub fn chat_list(&self, key: &str, limit: usize) -> Vec<Value> {
        let mut rows: Vec<_> = self
            .chats
            .iter()
            .filter_map(|(id, c)| {
                c["positions"]
                    .get(key)
                    .map(|p| (integer(&p["order"], 0), *id))
            })
            .collect();
        rows.sort_unstable_by(|a, b| b.cmp(a));
        rows.into_iter()
            .take(limit)
            .map(|(_, id)| self.chat_view(id))
            .collect()
    }
    pub fn all_chats(&self, limit: usize) -> Vec<Value> {
        let mut rows: Vec<_> = self
            .chats
            .iter()
            .filter(|(_, c)| c["positions"].as_object().is_some_and(|p| !p.is_empty()))
            .map(|(id, c)| (integer(&c["positions"]["main"]["order"], 0), *id))
            .collect();
        rows.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        rows.into_iter()
            .take(limit)
            .map(|(_, id)| self.chat_view(id))
            .collect()
    }
    fn chat_event(&self, id: i128) -> Vec<Value> {
        let view = self.chat_view(id);
        if view.is_null() {
            Vec::new()
        } else {
            vec![json!({"event":"chat","chat":view})]
        }
    }
    fn group_events(&self, key: &str, id: i128) -> Vec<Value> {
        self.chats
            .iter()
            .filter(|(_, c)| integer(&c[key], 0) == id)
            .flat_map(|(cid, _)| self.chat_event(*cid))
            .collect()
    }
    pub fn apply(&mut self, u: &Value) -> Vec<Value> {
        let id = integer(&u["chat_id"], 0);
        match tag(u) {
            "updateNewChat" => {
                let id = self.new_chat(&u["chat"]);
                self.chat_event(id)
            }
            "updateUser" => {
                let user = typed(&u["user"], "user");
                let uid = integer(&user["id"], 0);
                if uid == 0 {
                    return Vec::new();
                }
                let username = array(&user["usernames"]["active_usernames"], 4)
                    .next()
                    .map(|v| text(v, 128))
                    .unwrap_or_default();
                let mut name = [
                    text(&user["first_name"], 128),
                    text(&user["last_name"], 128),
                ]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
                if name.is_empty() {
                    name = username.clone();
                }
                let view = json!({"id":number(uid),"name":name,"username":username,"bot":tag(&user["type"])=="userTypeBot","status":setting::status_view(&user["status"]),"phone":text(&user["phone_number"],32)});
                self.users.shift_remove(&uid);
                self.users.insert(uid, view.clone());
                if self.users.len() > 50000 {
                    let protected: Vec<_> = self
                        .chats
                        .values()
                        .map(|c| integer(&c["userId"], 0))
                        .chain([self.me_id])
                        .collect();
                    let old: Vec<_> = self.users.keys().copied().collect();
                    for uid in old {
                        if self.users.len() <= 45000 {
                            break;
                        }
                        if !protected.contains(&uid) {
                            self.users.shift_remove(&uid);
                        }
                    }
                }
                let mut public = view;
                if let Some(map) = public.as_object_mut() {
                    map.remove("phone");
                }
                vec![json!({"event":"user","user":public})]
            }
            "updateUserStatus" => {
                let uid = integer(&u["user_id"], 0);
                if uid == 0 {
                    return Vec::new();
                }
                let status = setting::status_view(&u["status"]);
                if let Some(user) = self.users.get_mut(&uid) {
                    user["status"] = status.clone();
                }
                vec![json!({"event":"userStatus","userId":number(uid),"status":status})]
            }
            "updateBasicGroup" | "updateSupergroup" => {
                let supergroup = tag(u) == "updateSupergroup";
                let g = typed(
                    &u[if supergroup {
                        "supergroup"
                    } else {
                        "basic_group"
                    }],
                    if supergroup {
                        "supergroup"
                    } else {
                        "basicGroup"
                    },
                );
                let gid = integer(&g["id"], 0);
                if gid <= 0 {
                    return Vec::new();
                }
                let mut view = json!({"memberCount":number(integer(&g["member_count"],0).max(0)),"status":member_status(tag(&g["status"]),""),"canChangeInfo":setting::can_change_info(&g["status"])});
                if supergroup {
                    view["username"] = json!(
                        array(&g["usernames"]["active_usernames"], 4)
                            .next()
                            .map(|v| text(v, 128))
                            .unwrap_or_default()
                    );
                    view["forum"] = json!(yes(&g["is_forum"]));
                    view["joinToSend"] = json!(g["join_to_send_messages"] != false);
                }
                let groups = if supergroup {
                    &mut self.supergroups
                } else {
                    &mut self.basic_groups
                };
                groups.shift_remove(&gid);
                groups.insert(gid, view);
                while groups.len() > 20000 {
                    groups.shift_remove_index(0);
                }
                self.group_events(
                    if supergroup {
                        "supergroupId"
                    } else {
                        "basicGroupId"
                    },
                    gid,
                )
            }
            "updateSecretChat" => {
                let s = typed(&u["secret_chat"], "secretChat");
                let sid = integer(&s["id"], 0);
                if sid <= 0 {
                    return Vec::new();
                }
                let state = match tag(&s["state"]) {
                    "secretChatStateReady" => "ready",
                    "secretChatStateClosed" => "closed",
                    _ => "pending",
                };
                self.secrets.shift_remove(&sid);
                self.secrets.insert(sid,json!({"state":state,"outbound":yes(&s["is_outbound"]),"userId":number(integer(&s["user_id"],0)),"keyHash":setting::key_hash_view(&s["key_hash"])}));
                while self.secrets.len() > 20000 {
                    self.secrets.shift_remove_index(0);
                }
                self.group_events("secretChatId", sid)
            }
            "updateConnectionState" => {
                let state = match tag(&u["state"]) {
                    "connectionStateWaitingForNetwork" => "network",
                    "connectionStateConnectingToProxy" => "proxy",
                    "connectionStateConnecting" => "connecting",
                    "connectionStateUpdating" => "updating",
                    "connectionStateReady" => "ready",
                    _ => "",
                };
                if state == self.connection {
                    return Vec::new();
                }
                self.connection = state.into();
                vec![json!({"event":"connection","state":state})]
            }
            "updateScopeNotificationSettings" => {
                let scope = match tag(&u["scope"]) {
                    "notificationSettingsScopePrivateChats" => "private",
                    "notificationSettingsScopeGroupChats" => "groups",
                    "notificationSettingsScopeChannelChats" => "channels",
                    _ => return Vec::new(),
                };
                self.scopes.insert(
                    scope.into(),
                    integer(&u["notification_settings"]["mute_for"], 0).max(0) > 0,
                );
                self.chats
                    .keys()
                    .flat_map(|id| self.chat_event(*id))
                    .collect()
            }
            "updateChatFolders" => {
                self.folders=array(&u["chat_folders"],64).filter_map(|v|{let f=typed(v,"chatFolderInfo");let id=integer(&f["id"],0);if id<=0{return None;}let ft=formatted(&f["name"]["text"]);let name=text(&ft[0],128).split_whitespace().collect::<Vec<_>>().join(" ");Some(json!({"id":number(id),"name":if name.is_empty(){"Folder"}else{&name},"icon":text(&f["icon"]["name"],32)}))}).collect();
                self.main_position = integer(&u["main_chat_list_position"], 0).max(0);
                vec![
                    json!({"event":"folders","folders":self.folders,"mainPosition":number(self.main_position)}),
                ]
            }
            "updateChatTitle"
            | "updateChatPhoto"
            | "updateChatLastMessage"
            | "updateChatPosition"
            | "updateChatDraftMessage"
            | "updateChatReadInbox"
            | "updateChatReadOutbox"
            | "updateChatUnreadMentionCount"
            | "updateChatUnreadReactionCount"
            | "updateMessageUnreadReactions"
            | "updateChatDefaultDisableNotification"
            | "updateChatMessageAutoDeleteTime"
            | "updateChatPermissions"
            | "updateChatNotificationSettings"
            | "updateChatIsMarkedAsUnread"
            | "updateChatHasScheduledMessages" => {
                let last = if tag(u) == "updateChatLastMessage" {
                    self.preview(&u["last_message"])
                } else {
                    Value::Null
                };
                let photo = if tag(u) == "updateChatPhoto" {
                    self.photo(&u["photo"])
                } else {
                    Value::Null
                };
                let Some(chat) = self.chats.get_mut(&id) else {
                    return Vec::new();
                };
                match tag(u) {
                    "updateChatTitle" => chat["title"] = json!(text(&u["title"], 256)),
                    "updateChatPhoto" => chat["photo"] = photo,
                    "updateChatLastMessage" | "updateChatDraftMessage" => {
                        if tag(u) == "updateChatLastMessage" {
                            chat["lastMessage"] = last;
                        } else {
                            chat["draft"] = setting::draft_text(&u["draft_message"]);
                        }
                        chat["positions"] = json!({});
                        for p in array(&u["positions"], 16) {
                            Self::position(chat, p);
                        }
                    }
                    "updateChatPosition" => Self::position(chat, &u["position"]),
                    "updateChatReadInbox" => {
                        chat["unread"] = number(integer(&u["unread_count"], 0).max(0));
                        chat["lastReadInbox"] =
                            number(integer(&u["last_read_inbox_message_id"], 0));
                    }
                    "updateChatReadOutbox" => {
                        chat["lastReadOutbox"] =
                            number(integer(&u["last_read_outbox_message_id"], 0))
                    }
                    "updateChatUnreadMentionCount" => {
                        chat["mentions"] = number(integer(&u["unread_mention_count"], 0).max(0))
                    }
                    "updateChatUnreadReactionCount" | "updateMessageUnreadReactions" => {
                        chat["reactions"] = number(integer(&u["unread_reaction_count"], 0).max(0))
                    }
                    "updateChatDefaultDisableNotification" => {
                        chat["silent"] = json!(yes(&u["default_disable_notification"]))
                    }
                    "updateChatMessageAutoDeleteTime" => {
                        chat["autoDelete"] =
                            number(integer(&u["message_auto_delete_time"], 0).max(0))
                    }
                    "updateChatPermissions" => {
                        chat["canChangeInfo"] = json!(yes(&u["permissions"]["can_change_info"]))
                    }
                    "updateChatNotificationSettings" => {
                        chat["muteFor"] =
                            number(integer(&u["notification_settings"]["mute_for"], 0).max(0));
                        chat["notification"] =
                            setting::notification_settings(&u["notification_settings"]);
                    }
                    "updateChatIsMarkedAsUnread" => {
                        chat["markedUnread"] = json!(yes(&u["is_marked_as_unread"]))
                    }
                    "updateChatHasScheduledMessages" => {
                        chat["hasScheduled"] = json!(yes(&u["has_scheduled_messages"]))
                    }
                    _ => {}
                }
                self.chat_event(id)
            }
            _ => self.other_update(u),
        }
    }
    fn other_update(&mut self, u: &Value) -> Vec<Value> {
        let id = integer(&u["chat_id"], 0);
        let mid = integer(&u["message_id"], 0);
        match tag(u) {
            "updateNewMessage" | "updateMessageSendSucceeded" | "updateMessageSendFailed" => {
                let m = self.message(&u["message"]);
                if m.is_null() {
                    return Vec::new();
                }
                let event = match tag(u) {
                    "updateNewMessage" => "message",
                    "updateMessageSendSucceeded" => "messageSent",
                    _ => "messageFailed",
                };
                let mut out = json!({"event":event,"message":m});
                if event != "message" {
                    out["oldMessageId"] = number(integer(&u["old_message_id"], 0));
                }
                if event == "messageFailed" {
                    out["error"] = json!(text(&u["error"]["message"], 200));
                }
                vec![out]
            }
            "updateMessageContent" => vec![
                json!({"event":"messageContent","chatId":number(id),"messageId":number(mid),"content":body::content(&u["new_content"],&self.roots)}),
            ],
            "updateMessageEdited" => vec![
                json!({"event":"messageEdited","chatId":number(id),"messageId":number(mid),"editDate":number(integer(&u["edit_date"],0))}),
            ],
            "updateDeleteMessages" => {
                if !yes(&u["is_permanent"]) {
                    return Vec::new();
                }
                vec![
                    json!({"event":"messagesDeleted","chatId":number(id),"messageIds":array(&u["message_ids"],1000).map(|v|integer(v,0)).filter(|i|*i!=0).map(number).collect::<Vec<_>>()}),
                ]
            }
            "updateMessageInteractionInfo" => {
                if id == 0 || mid == 0 {
                    return Vec::new();
                }
                let r = body::reactions_view(&u["interaction_info"]);
                vec![
                    json!({"event":"messageInteraction","chatId":number(id),"messageId":number(mid),"reactions":r[0],"views":r[1],"replies":body::replies_view(&u["interaction_info"]["reply_info"])}),
                ]
            }
            "updateMessageIsPinned" => {
                if id == 0 || mid == 0 {
                    return Vec::new();
                }
                vec![
                    json!({"event":"messagePinned","chatId":number(id),"messageId":number(mid),"pinned":yes(&u["is_pinned"])}),
                ]
            }
            "updatePoll" => {
                let p = body::poll_view(&u["poll"]);
                if p.is_null() {
                    Vec::new()
                } else {
                    vec![json!({"event":"poll","poll":p})]
                }
            }
            "updateForumTopicInfo" => {
                let i = typed(&u["info"], "forumTopicInfo");
                let id = integer(&i["chat_id"], 0);
                let tid = integer(&i["forum_topic_id"], 0);
                if id == 0 || tid <= 0 {
                    return Vec::new();
                }
                vec![
                    json!({"event":"topicInfo","chatId":number(id),"topicId":number(tid),"name":text(&i["name"],256),"closed":yes(&i["is_closed"]),"hidden":yes(&i["is_hidden"])}),
                ]
            }
            "updateForumTopic" => {
                let tid = integer(&u["forum_topic_id"], 0);
                if id == 0 || tid <= 0 {
                    return Vec::new();
                }
                vec![
                    json!({"event":"topicUpdate","chatId":number(id),"topicId":number(tid),"pinned":yes(&u["is_pinned"]),"mentions":number(integer(&u["unread_mention_count"],0).max(0)),"lastReadInbox":number(integer(&u["last_read_inbox_message_id"],0))}),
                ]
            }
            "updateFile" => {
                let view = file_view(&u["file"], &self.roots);
                if view.is_null() {
                    return Vec::new();
                }
                let id = integer(&view["id"], 0);
                let step = (integer(&view["size"], 0) / 20).max(65536);
                if let Some(last) = self.file_marks.get(&id)
                    && view["path"] == ""
                    && view["active"] == last["active"]
                    && integer(&view["downloaded"], 0) - integer(&last["downloaded"], 0) < step
                {
                    return Vec::new();
                }
                if self.file_marks.len() >= 4096 {
                    self.file_marks.clear();
                }
                self.file_marks.insert(id, view.clone());
                vec![json!({"event":"file","file":view})]
            }
            "updateCall" => {
                let c = typed(&u["call"], "call");
                let id = integer(&c["id"], 0);
                if id <= 0 {
                    return Vec::new();
                }
                let uid = integer(&c["user_id"], 0);
                let state = match tag(&c["state"]) {
                    "callStateExchangingKeys" => "connecting",
                    "callStateReady" => "ready",
                    "callStateHangingUp" => "ending",
                    "callStateDiscarded" => "ended",
                    "callStateError" => "failed",
                    _ => "pending",
                };
                let view = json!({"id":number(id),"userId":number(uid),"name":self.user_name(uid),"outgoing":yes(&c["is_outgoing"]),"video":yes(&c["is_video"]),"state":state});
                if matches!(state, "ended" | "failed") {
                    self.calls.shift_remove(&id);
                } else {
                    self.calls.insert(id, view.clone());
                    while self.calls.len() > 16 {
                        self.calls.shift_remove_index(0);
                    }
                }
                vec![json!({"event":"call","call":view})]
            }
            "updateChatActiveStories" => {
                let a = typed(&u["active_stories"], "chatActiveStories");
                let id = integer(&a["chat_id"], 0);
                if id == 0 {
                    return Vec::new();
                }
                let stories:Vec<_>=array(&a["stories"],100).filter_map(|v|{let s=typed(v,"storyInfo");let sid=integer(&s["story_id"],0);(sid>0&&sid<=i128::from(i32::MAX)).then(||json!({"id":number(sid),"date":number(integer(&s["date"],0)),"closeFriends":yes(&s["is_for_close_friends"]),"live":yes(&s["is_live"])}))}).collect();
                let view = json!({"chatId":number(id),"list":match tag(&a["list"]){"storyListMain"=>"main","storyListArchive"=>"archive",_=>""},"order":integer(&a["order"],0).max(0).to_string(),"maxReadId":number(integer(&a["max_read_story_id"],0).max(0)),"title":self.chats.get(&id).map(|c|text(&c["title"],256)).unwrap_or_default(),"stories":stories});
                self.stories.shift_remove(&id);
                if !stories.is_empty() {
                    self.stories.insert(id, view.clone());
                    while self.stories.len() > 500 {
                        self.stories.shift_remove_index(0);
                    }
                }
                vec![json!({"event":"stories","stories":view})]
            }
            "updateChatAction" => {
                let action = chat_action(tag(&u["action"]));
                if id == 0 || action.is_empty() {
                    return Vec::new();
                }
                let (sender, name) = self.sender(&u["sender_id"]);
                vec![
                    json!({"event":"chatAction","chatId":number(id),"senderId":sender["id"],"senderName":name,"action":action}),
                ]
            }
            _ => Vec::new(),
        }
    }
    pub fn member_view(&self, value: &Value) -> Value {
        let m = typed(value, "chatMember");
        let (sender, name) = self.sender(&m["member_id"]);
        let id = integer(&sender["id"], 0);
        if id == 0 {
            return Value::Null;
        }
        let user = if sender["type"] == "user" {
            self.users.get(&id)
        } else {
            None
        }
        .unwrap_or(&Value::Null);
        json!({"type":sender["type"],"id":sender["id"],"name":name,"status":member_status(tag(&m["status"]),"member"),"tag":text(&m["tag"],64),"bot":yes(&user["bot"]),"userStatus":user["status"]})
    }
    pub fn topic_view(&self, value: &Value) -> Value {
        let t = typed(value, "forumTopic");
        let i = typed(&t["info"], "forumTopicInfo");
        let id = integer(&i["forum_topic_id"], 0);
        if id <= 0 {
            return Value::Null;
        }
        json!({"id":number(id),"chatId":number(integer(&i["chat_id"],0)),"name":text(&i["name"],256),"color":number(integer(&typed(&i["icon"],"forumTopicIcon")["color"],0)&0xffffff),"general":yes(&i["is_general"]),"closed":yes(&i["is_closed"]),"hidden":yes(&i["is_hidden"]),"pinned":yes(&t["is_pinned"]),"unread":number(integer(&t["unread_count"],0).max(0)),"mentions":number(integer(&t["unread_mention_count"],0).max(0)),"order":integer(&t["order"],0).max(0).to_string(),"lastMessage":self.preview(&t["last_message"]),"draft":setting::draft_text(&t["draft_message"])})
    }
    pub fn story_view(&self, value: &Value) -> Value {
        let s = typed(value, "story");
        let id = integer(&s["id"], 0);
        let cid = integer(&s["poster_chat_id"], 0);
        if id <= 0 || id > i128::from(i32::MAX) || cid == 0 {
            return Value::Null;
        }
        let c = &s["content"];
        let (mut kind, mut media) = ("unsupported", Value::Null);
        match tag(c) {
            "storyContentPhoto" => {
                media = media_for("photo", &json!({"photo":c["photo"]}), &self.roots);
                if !media.is_null() {
                    kind = "photo";
                }
            }
            "storyContentVideo" => {
                let v = typed(&c["video"], "storyVideo");
                let file = file_view(&v["video"], &self.roots);
                if !file.is_null() {
                    let d = v["duration"]
                        .as_f64()
                        .filter(|d| *d >= 0.0 && *d <= 86400.0)
                        .unwrap_or(0.0);
                    media = json!({"file":file,"width":number(integer(&v["width"],0).max(0)),"height":number(integer(&v["height"],0).max(0)),"duration":d as u32,"thumb":thumbnail(&v["thumbnail"],&self.roots),"mini":minithumbnail(&v["minithumbnail"])});
                    kind = "video";
                }
            }
            "storyContentLive" => kind = "live",
            _ => {}
        }
        let ft = formatted(&s["caption"]);
        json!({"id":number(id),"chatId":number(cid),"date":number(integer(&s["date"],0)),"kind":kind,"media":media,"caption":{"text":ft[0],"entities":ft[1]},"title":self.chats.get(&cid).map(|c|text(&c["title"],256)).unwrap_or_default(),"protected":!yes(&s["can_be_forwarded"]),"views":number(integer(&s["interaction_info"]["view_count"],0).max(0))})
    }
}
fn member_status(kind: &str, default: &str) -> String {
    match kind {
        "chatMemberStatusCreator" => "owner",
        "chatMemberStatusAdministrator" => "admin",
        "chatMemberStatusMember" => "member",
        "chatMemberStatusRestricted" => "restricted",
        "chatMemberStatusLeft" => "left",
        "chatMemberStatusBanned" => "banned",
        _ => default,
    }
    .to_owned()
}
fn chat_action(kind: &str) -> &'static str {
    match kind {
        "chatActionTyping" => "typing",
        "chatActionRecordingVideo" => "recordingVideo",
        "chatActionUploadingVideo" => "uploadingVideo",
        "chatActionRecordingVoiceNote" => "recordingVoice",
        "chatActionUploadingVoiceNote" => "uploadingVoice",
        "chatActionUploadingPhoto" => "uploadingPhoto",
        "chatActionUploadingDocument" => "uploadingFile",
        "chatActionChoosingSticker" => "choosingSticker",
        "chatActionChoosingLocation" => "choosingLocation",
        "chatActionChoosingContact" => "choosingContact",
        "chatActionStartPlayingGame" => "playingGame",
        "chatActionRecordingVideoNote" => "recordingVideoNote",
        "chatActionUploadingVideoNote" => "uploadingVideoNote",
        "chatActionWatchingAnimations" => "watchingAnimations",
        "chatActionCancel" => "cancel",
        _ => "",
    }
}

pub fn project(value: &Value, roots: &[String]) -> Value {
    let mut state = State::new(roots.to_vec());
    state.me_id = integer(&value["meId"], 0);
    let events: Vec<_> = array(&value["updates"], 50000)
        .map(|u| json!(state.apply(u)))
        .collect();
    let mut queries = Vec::new();
    for q in array(&value["queries"], 1000) {
        queries.push(match q["operation"].as_str().unwrap_or("") {
            "message" => state.message(&q["value"]),
            "preview" => state.preview(&q["value"]),
            "topic_view" => state.topic_view(&q["value"]),
            "member_view" => state.member_view(&q["value"]),
            "story_view" => state.story_view(&q["value"]),
            "chat_view" => state.chat_view(integer(&q["value"], 0)),
            "chat_list" => json!(state.chat_list(q["list"].as_str().unwrap_or("main"), 500)),
            "all_chats" => json!(state.all_chats(500)),
            _ => Value::Null,
        });
    }
    json!({"events":events,"queries":queries,"chats":state.chat_list("main",500),"archive":state.chat_list("archive",500),"stories":state.stories.values().collect::<Vec<_>>()})
}
