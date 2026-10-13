#!/usr/bin/python3
"""Development-only pure Python/Rust parity; use an isolated home/network worker."""
import base64
import importlib.util
import json
import pathlib
import random
import os
import socket
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
# Fail before importing runtime owners. Environment opt-in alone is not an
# isolation boundary: verify the readonly workspace and private network too.
if not os.statvfs(ROOT).f_flag & os.ST_RDONLY:
    raise SystemExit("Differential checks require the reviewed read-only inert workspace")
if any(name != "lo" for _, name in socket.if_nameindex()):
    raise SystemExit("Differential checks require an isolated network namespace")
status = pathlib.Path("/proc/self/status").read_text()
if any(line.startswith("CapEff:") and int(line.split()[1], 16) for line in status.splitlines()):
    raise SystemExit("Differential checks require no effective capabilities")
if os.environ.get("HOME") != "/home/worker" or pathlib.Path("/home/slovn").exists():
    raise SystemExit("Differential checks require an inert home with no working profiles")
sys.path.insert(0, str(ROOT / "bin"))
import omagram_projection_media as media
import omagram_projection_text as text
import omagram_projection_settings as settings
import omagram_projection_content as content
import omagram_projection_rich as rich
import omagram_state as state
# This test is invoked only in the reviewed inert namespace. The source derives
# its home from passwd, not HOME; redirect before importing application helpers.
import plugin_safety
plugin_safety.home_dir = lambda: "/tmp/home"
pathlib.Path("/tmp/home").mkdir(mode=0o700, exist_ok=True)
import omagram_backend as backend
import omagram_settings as prefs


def corpus():
    rng = random.Random(20261010)
    roots = ["/fixture/files", "/fixture/database/stories", "/fixture/sent", "/fixture/run/rec"]
    values = [None, False, True, 0, -1, 1.5, "", [], {}, "123", "-9", "18446744073709551615"]
    cases = []

    def add(operation, value, expected, case_roots=roots):
        cases.append({"operation": operation, "value": value, "roots": case_roots, "expected": expected})

    for value in values:
        for operation in ["formatted", "file_view", "minithumbnail", "thumbnail", "best_photo_size", "preview_photo_size", "waveform", "auth_view", "list_key"]:
            function = getattr(text if operation == "formatted" else media, operation)
            expected = function(value, roots) if operation in ["file_view", "thumbnail"] else function(value)
            add(operation, value, expected)
    for path in ["/fixture/sent/video.mp4", "/fixture/sent/../database/db.sqlite", "/fixture/sent//bad", "/fixture/sent/a\x7f", "/fixture/sent/😀.mp4", "/fixture/sentx/a", "file:///fixture/sent/a", "/fixture/sent", "/fixture/files/./a", "relative"]:
        add("local_path", path, media.local_path(path, roots))
    tags = list(text.ENTITY_TYPES) + ["unsupported"]
    for index in range(160):
        body = rng.choice(["", "hello", "😀é Ж", "x" * 16385, "🧑‍💻" * 6])
        entities = [{"offset": rng.randrange(-2, 15), "length": rng.randrange(-2, 15), "type": {"@type": rng.choice(tags), "url": "https://example.test", "user_id": "123", "language": "rust", "custom_emoji_id": "9223372036854775807"}} for _ in range(rng.randrange(8))]
        value = {"text": body, "entities": entities}
        add("formatted", value, text.formatted(value))
    for index in range(160):
        value = {"@type": "file", "id": rng.choice(values), "size": rng.choice(values), "expected_size": rng.choice(values), "local": {"is_downloading_completed": rng.choice(values), "is_downloading_active": rng.choice(values), "downloaded_size": rng.choice(values), "path": rng.choice(["/fixture/sent/video.mp4", "/fixture/files/😀", "/fixture/files/../no", ""] )}}
        add("file_view", value, media.file_view(value, roots))
    for length in [0, 1, 2, 3, 4, 5, 30, 31, 32, 60, 100, 190]:
        raw = bytes(rng.randrange(256) for _ in range(length))
        value = base64.b64encode(raw).decode()
        add("waveform", value, media.waveform(value))
    for value in ["!!!!", "Zg", "Zg=", "Zg==", "Zg===", "Zh==", "AAAA====", "AAAA\n", "=", "===="]:
        add("waveform", value, media.waveform(value))
    for index in range(70):
        sizes = [{"@type": "photoSize", "width": rng.choice([1, 100, 320, 1280, 1281, 100001]), "height": rng.choice([1, 100, 320, 1280, 1281]), "photo": {"@type": "file", "id": j+1}} for j in range(rng.randrange(18))]
        for operation in ["best_photo_size", "preview_photo_size"]:
            add(operation, sizes, getattr(media, operation)(sizes))
    for kind, key, tag, filekey in [("sticker", "sticker", "sticker", "sticker"), ("gif", "animation", "animation", "animation"), ("voice", "voice_note", "voiceNote", "voice"), ("videoNote", "video_note", "videoNote", "video"), ("video", "video", "video", "video"), ("file", "document", "document", "document"), ("audio", "audio", "audio", "audio")]:
        for completed in [True, False, 1, None]:
            value = {key: {"@type": tag, filekey: {"@type": "file", "id": 7, "expected_size": 43, "local": {"path": "/fixture/sent/outgoing.mp4", "is_downloading_completed": completed}}, "width": 1920, "height": 1080, "duration": 8, "set_id": "9223372036854775807", "waveform": "AAAA", "mime_type": "video/mp4", "length": 480, "emoji": "😀", "title": "fixture", "performer": "fixture", "file_name": "fixture.mp4"}, "is_viewed": True, "is_listened": False}
            add("media_for", {"kind": kind, "content": value}, media.media_for(kind, value, roots))
    for kind in media.WAITING_STATES:
        value = {"@type": kind}
        add("auth_view", value, media.auth_view(value))
    for value in [{"@type": "authorizationStateWaitCode", "code_info": {"phone_number": "synthetic", "type": {"@type": "authenticationCodeTypeTelegramMessage", "length": 5}}}, {"@type": "authorizationStateWaitPassword", "password_hint": "synthetic"}, {"@type": "authorizationStateWaitOtherDeviceConfirmation", "link": "tg://login?token=synthetic"}]:
        add("auth_view", value, media.auth_view(value))
    for value in values:
        for operation in ["status_view", "can_change_info", "draft_text", "formatted_view", "topic_id", "thread_id", "scheduled_at", "key_hash_view", "session_view", "privacy_rule_list", "privacy_view", "password_view", "proxy_view", "folder_ids", "scope_view", "notification_settings"]:
            add(operation, value, getattr(settings, operation)(value))
    for seconds in [0, 1, -1, 59, 60, 61, 3600, 3601, 86400, 172800, 9223372036854775807]:
        for operation in ["clock_duration", "human_period"]:
            add(operation, seconds, getattr(settings, operation)(seconds))
    for length in [1, 2, 3, 8, 31, 32, 33, 64]:
        value = base64.b64encode(bytes(rng.randrange(256) for _ in range(length))).decode()
        add("key_hash_view", value, settings.key_hash_view(value))
    for index in range(80):
        value = {"@type": "scopeNotificationSettings", "mute_for": rng.choice(values), "sound_id": "9223372036854775807", "story_sound_id": "-9223372036854775808", "show_preview": rng.choice(values), "mute_stories": rng.choice(values)}
        add("scope_view", value, settings.scope_view(value))
        add("notification_settings", value, settings.notification_settings(value))
        muted, preview = rng.choice([None, False, True]), rng.choice([None, False, True])
        add("scope_settings", {"settings": value, "muted": muted, "preview": preview}, settings.scope_settings(value, muted, preview))
    for value in [
        {"@type": "userStatusOnline", "expires": 10},
        {"@type": "userStatusOffline", "was_online": "123"},
        {"@type": "userStatusRecently"},
        {"@type": "userStatusLastWeek"},
        {"@type": "userStatusLastMonth"},
    ]:
        add("status_view", value, settings.status_view(value))
    for value in [{"@type": "chatMemberStatusCreator"}, {"@type": "chatMemberStatusAdministrator", "rights": {"can_change_info": True}}, {"@type": "chatMemberStatusAdministrator", "rights": {"can_change_info": 1}}]:
        add("can_change_info", value, settings.can_change_info(value))
    for tag, key in [("messageTopicForum", "forum_topic_id"), ("messageTopicThread", "message_thread_id")]:
        for id in [0, 1, -1, "18446744073709551615", False]:
            value = {"@type": tag, key: id}
            for operation in ["topic_id", "thread_id"]:
                add(operation, value, getattr(settings, operation)(value))
    for tag in ["messageSchedulingStateSendAtDate", "messageSchedulingStateSendWhenVideoProcessed", "messageSchedulingStateSendWhenOnline"]:
        for date in [0, -1, 123]:
            value = {"@type": tag, "send_date": date}
            add("scheduled_at", value, settings.scheduled_at(value))
    for base in ["everybody", "contacts", "nobody"]:
        value = {"@type": "userPrivacySettingRules", "rules": [{"@type": "userPrivacySettingRuleAllowAll"}, {"@type": "userPrivacySettingRuleAllowContacts"}, {"@type": "userPrivacySettingRuleAllowUsers", "user_ids": [1, 2]}, {"@type": "userPrivacySettingRuleRestrictChatMembers", "chat_ids": [3]}, {"@type": "userPrivacySettingRuleNewFuture"}]}
        add("privacy_view", value, settings.privacy_view(value))
        add("privacy_rules", {"rules": value, "base": base}, settings.privacy_rules(value, base))
    for value in values:
        for operation in ["content", "link_preview", "poll_view", "contact_view", "reply_markup", "reactions_view", "replies_view"]:
            function = getattr(content, operation)
            add(operation, value, function(value, roots) if operation in ["content", "link_preview"] else function(value))
        add("rich_message", value, rich.rich_message(value, roots))
    for name, kind in content.CONTENT_KINDS.items():
        value = {"@type": name, "caption": {"text": "😀 caption", "entities": []}, "emoji": "🎲", "value": 6, "has_spoiler": True}
        add("content", value, content.content(value, roots))
    for operation, value in [
        ("contact_view", {"@type": "contact", "first_name": "😀", "last_name": "fixture", "phone_number": "synthetic", "user_id": "123"}),
        ("location_view", {"@type": "messageLocation", "location": {"@type": "location", "latitude": 89.1, "longitude": -179.9}}),
        ("location_view", {"@type": "messageLocation", "location": {"@type": "location", "latitude": True, "longitude": -179.9}}),
        ("replies_view", {"@type": "messageReplyInfo", "reply_count": 3, "last_message_id": 10, "last_read_inbox_message_id": 9}),
        ("poll_view", {"@type": "poll", "id": "9223372036854775807", "question": {"text": "fixture"}, "options": [{"@type": "pollOption", "text": {"text": "one"}, "voter_count": 2, "vote_percentage": 140, "is_chosen": True}, None], "type": {"@type": "pollTypeQuiz", "correct_option_ids": [0]}, "is_closed": True}),
        ("reactions_view", {"@type": "messageInteractionInfo", "reactions": {"reactions": [{"@type": "messageReaction", "total_count": 3, "is_chosen": True, "type": {"@type": "reactionTypePaid"}}, {"@type": "messageReaction", "total_count": 2, "type": {"@type": "reactionTypeCustomEmoji", "custom_emoji_id": "9223372036854775807"}}]}, "view_count": 42}),
    ]:
        add(operation, value, getattr(content, operation)(value))
    for kind in content.INLINE_BUTTONS:
        value = {"@type": "replyMarkupInlineKeyboard", "rows": [[{"@type": "inlineKeyboardButton", "text": "fixture", "type": {"@type": kind, "data": "AAAA", "url": "https://example.test", "user_id": "123", "text": "copy", "query": "fixture"}}, None]]}
        add("reply_markup", value, content.reply_markup(value))
    for value in [{"@type": "replyMarkupRemoveKeyboard"}, {"@type": "replyMarkupForceReply", "input_field_placeholder": "fixture"}, {"@type": "replyMarkupShowKeyboard", "rows": [[{"@type": "keyboardButton", "text": "fixture", "type": {"@type": "keyboardButtonTypeRequestPhoneNumber"}}]], "one_time": True}]:
        add("reply_markup", value, content.reply_markup(value))
    for index in range(70):
        blocks = []
        for j in range(rng.randrange(15)):
            node = {"@type": "richTextPlain", "text": rng.choice(["😀 text", "fixture", "", "x" * 17000])}
            for wrapper in rng.choices(["richTextBold", "richTextItalic", "richTextUrl", "richTextCustomEmoji"], k=rng.randrange(4)):
                node = {"@type": wrapper, "text": node, "url": "https://example.test", "custom_emoji_id": "9223372036854775807"}
            block = {"@type": "pageBlockParagraph", "text": node}
            for depth in range(rng.randrange(6)):
                block = {"@type": "pageBlockDetails", "blocks": [block]}
            blocks.append(block)
        value = {"@type": "richMessage", "blocks": blocks, "is_full": True, "is_rtl": False}
        add("rich_message", value, rich.rich_message(value, roots))
    for block in [{"@type": "pageBlockTable", "cells": [[{"text": {"@type": "richTextPlain", "text": "fixture"}}]]}, {"@type": "pageBlockEmbedded", "html": "<script>never executed</script>"}, {"@type": "pageBlockList", "items": [{"label": "😀", "blocks": [{"@type": "pageBlockParagraph", "text": {"@type": "richTextBold", "text": {"@type": "richTextPlain", "text": "test"}}}]}]}, {"@type": "pageBlockPhoto"}, {"@type": "pageBlockAnchor"}, {"@type": "pageBlockDivider"}]:
        value = {"@type": "richMessage", "blocks": [block]}
        add("rich_message", value, rich.rich_message(value, roots))
    for iteration in range(40):
        updates = [{"@type": "updateUser", "user": {"@type": "user", "id": 7, "first_name": "😀", "last_name": "fixture", "phone_number": "synthetic", "status": {"@type": "userStatusOnline", "expires": 100}}}, {"@type": "updateNewChat", "chat": {"@type": "chat", "id": 42, "title": "fixture", "type": {"@type": "chatTypeSupergroup", "supergroup_id": 3}, "positions": [{"@type": "chatPosition", "list": {"@type": "chatListMain"}, "order": "9223372036854775807"}, {"@type": "chatPosition", "list": {"@type": "chatListArchive"}, "order": "100"}], "notification_settings": {"use_default_mute_for": True}}}]
        for _ in range(30):
            updates.append(rng.choice([
                {"@type": "updateChatLastMessage", "chat_id": 42, "positions": []},
                {"@type": "updateChatDraftMessage", "chat_id": 42, "draft_message": {"@type": "draftMessage", "content": {"@type": "draftMessageContentText", "text": {"text": "new draft"}}}, "positions": [{"@type": "chatPosition", "list": {"@type": "chatListMain"}, "order": "42"}]},
                {"@type": "updateChatPosition", "chat_id": 42, "position": {"@type": "chatPosition", "list": {"@type": "chatListArchive"}, "order": "100"}},
                {"@type": "updateSupergroup", "supergroup": {"@type": "supergroup", "id": 3, "member_count": 20, "is_forum": True, "status": {"@type": "chatMemberStatusMember"}, "join_to_send_messages": False}},
                {"@type": "updateScopeNotificationSettings", "scope": {"@type": "notificationSettingsScopeGroupChats"}, "notification_settings": {"mute_for": 3600}},
                {"@type": "updateChatReadInbox", "chat_id": 42, "unread_count": 12, "last_read_inbox_message_id": 100},
                {"@type": "updateUserStatus", "user_id": 7, "status": {"@type": "userStatusRecently"}},
                {"@type": "updateChatAction", "chat_id": 42, "sender_id": {"@type": "messageSenderUser", "user_id": 7}, "action": {"@type": "chatActionTyping"}},
                {"@type": "updateChatNotificationSettings", "chat_id": 42, "notification_settings": {"use_default_mute_for": False, "mute_for": 0}},
                {"@type": "updateChatActiveStories", "active_stories": {"@type": "chatActiveStories", "chat_id": 42, "list": {"@type": "storyListMain"}, "order": "9223372036854775807", "stories": [{"@type": "storyInfo", "story_id": 2, "date": 20}]}},
                {"@type": "updateCall", "call": {"@type": "call", "id": 2, "user_id": 7, "state": {"@type": "callStatePending"}}},
                {"@type": "updateFile", "file": {"@type": "file", "id": 4, "size": 100000, "local": {"is_downloading_active": True, "downloaded_size": rng.randrange(100000)}}},
            ]))
        message = {"@type": "message", "id": 100, "chat_id": 42, "sender_id": {"@type": "messageSenderUser", "user_id": 7}, "is_outgoing": True, "media_album_id": "9223372036854775807", "content": {"@type": "messageVideo", "video": {"@type": "video", "video": {"@type": "file", "id": 4, "size": 43, "local": {"is_downloading_completed": True, "path": "/fixture/sent/outgoing.mp4"}}, "width": 1920, "height": 1080, "duration": 10}}}
        updates += [{"@type": "updateNewMessage", "message": message}, {"@type": "updateMessageSendSucceeded", "message": message, "old_message_id": 9}, {"@type": "updateDeleteMessages", "chat_id": 42, "message_ids": [1, 2, False, "3"], "is_permanent": True}]
        queries = [{"operation": "message", "value": message}, {"operation": "preview", "value": message}, {"operation": "chat_view", "value": 42}]
        value = {"updates": updates, "queries": queries, "meId": 7}
        owner = state.State(tuple(roots))
        owner.me_id = 7
        events = [owner.apply(update) for update in updates]
        query_results = [getattr(owner, q["operation"])(q["value"]) for q in queries]
        expected = {"events": events, "queries": query_results, "chats": owner.chat_list(), "archive": owner.chat_list("archive"), "stories": owner.story_list()}
        add("state_sequence", value, expected)
    for positions in [[{"list": {"@type": "chatListMain"}, "order": "100"}], [{"list": {"@type": "chatListArchive"}, "order": "30"}], [], [{"list": {"@type": "chatListFolder", "chat_folder_id": 1}, "order": "9"}]]:
        updates = [{"@type": "updateNewChat", "chat": {"@type": "chat", "id": cid, "title": str(cid), "type": {"@type": "chatTypePrivate", "user_id": cid}, "positions": positions if cid % 2 else [{"list": {"@type": "chatListMain"}, "order": "100"}]}} for cid in [9, 2, 7, 4]]
        py = state.State(tuple(roots))
        events = [py.apply(u) for u in updates]
        add("state_sequence", {"updates": updates, "queries": [{"operation": "all_chats"}]}, {"events": events, "queries": [py.all_chats()], "chats": py.chat_list(), "archive": py.chat_list("archive"), "stories": py.story_list()})
    # Exercise every update owner and all service projections with malformed,
    # missing and boundary values, not only the selected sequence above.
    import ast
    owners = [n.name.removeprefix("_on_") for n in ast.walk(ast.parse((ROOT / "bin/omagram_state.py").read_text())) if isinstance(n, ast.FunctionDef) and n.name.startswith("_on_")]
    warmup = [
        {"@type": "updateUser", "user": {"@type": "user", "id": 7, "first_name": "Fixture"}},
        *[{"@type": "updateNewChat", "chat": {"@type": "chat", "id": cid, "title": "Fixture", "type": kind, "positions": [{"@type": "chatPosition", "list": {"@type": "chatListMain"}, "order": "100"}]}} for cid, kind in [(42, {"@type": "chatTypeSupergroup", "supergroup_id": 3}), (43, {"@type": "chatTypePrivate", "user_id": 7}), (44, {"@type": "chatTypeBasicGroup", "basic_group_id": 5}), (45, {"@type": "chatTypeSecret", "user_id": 7, "secret_chat_id": 6})]],
    ]
    fields = {"chat_id": 42, "message_id": 13, "forum_topic_id": 1, "user_id": 7, "unread_count": 2, "unread_mention_count": 3, "unread_reaction_count": 4, "message_auto_delete_time": 86400, "edit_date": 20, "title": "😀" * 300, "default_disable_notification": True, "is_marked_as_unread": True, "has_scheduled_messages": True, "is_permanent": True, "is_pinned": True, "message_ids": [1, 2, "3"], "permissions": {"can_change_info": True}, "notification_settings": {"mute_for": 100}, "sender_id": {"@type": "messageSenderUser", "user_id": 7}, "action": {"@type": "chatActionUploadingDocument"}}
    for owner in owners:
        for update in [{"@type": owner}, dict(fields, **{"@type": owner})]:
            py = state.State(tuple(roots))
            updates = warmup + [update]
            events = [py.apply(u) for u in updates]
            add("state_sequence", {"updates": updates}, {"events": events, "queries": [], "chats": py.chat_list(), "archive": py.chat_list("archive"), "stories": py.story_list()})
    services = ["messageChatAddMembers", "messageChatJoinByLink", "messageChatJoinByRequest", "messageChatDeleteMember", "messageChatChangeTitle", "messageChatChangePhoto", "messageChatDeletePhoto", "messagePinMessage", "messageBasicGroupChatCreate", "messageSupergroupChatCreate", "messageChatUpgradeTo", "messageChatUpgradeFrom", "messageScreenshotTaken", "messageContactRegistered", "messageCall", "messageVideoChatStarted", "messageVideoChatEnded", "messageChatSetMessageAutoDeleteTime", "messageForumTopicCreated", "messageForumTopicEdited", "messageCustomServiceAction", "messageChatSetTheme", "messageChatSetBackground", "messageChatBoost"]
    for kind in services:
        for variant in [None, False, -1, 0, 7, "7", "😀" * 300, [7]]:
            body = {"@type": kind, "member_user_ids": variant, "user_id": variant, "title": variant, "name": variant, "text": variant, "duration": variant, "is_video": variant, "message_auto_delete_time": variant}
            message = {"@type": "message", "chat_id": 42, "sender_id": {"@type": "messageSenderUser", "user_id": 7}, "content": body}
            queries = [{"operation": "message", "value": message}, {"operation": "preview", "value": message}]
            py = state.State(tuple(roots))
            events = [py.apply(u) for u in warmup]
            add("state_sequence", {"updates": warmup, "queries": queries}, {"events": events, "queries": [py.message(message), py.preview(message)], "chats": py.chat_list(), "archive": py.chat_list("archive"), "stories": py.story_list()})
    command_names = ["chats.list", "chats.load", "chat.open", "chat.close", "chat.history", "topic.history", "thread.history", "chat.read", "chat.scheduled", "chat.pinned", "chat.leave", "chat.clearHistory", "chat.markUnread", "chat.pin", "chat.archive", "chat.setSilent", "chat.setAutoDelete", "chat.mute", "chat.join", "chats.search", "messages.search", "chat.nextMention", "chat.nextReaction", "chat.readMentions", "chat.readReactions", "chat.draft", "message.properties", "message.link", "message.pin", "message.viewers", "reactions.available", "reaction.set", "poll.vote", "chat.messageByDate", "thread.open", "topics.list", "user.chat", "secret.create", "secret.close", "call.decline", "story.get", "story.open", "story.close", "gifs.saved", "sessions.list", "sessions.terminateOthers", "folder.delete", "auth.password", "auth.qr", "auth.logout"]

    class Oracle(backend.Daemon):
        def __init__(self, updates, answer):
            self._state = state.State(tuple(roots))
            self.clients = {7: object()}
            self.session_for = lambda *args: self
            self._gif_bot_id = 0
            self._state.me_id = 7
            for update in updates:
                self._state.apply(update)
            self.answer = answer
            self.answers = None
            self.strict_native_errors = False
            self.td = type("InertTd", (), {"execute": lambda self, query: None})()
            self.error = None
            self.queries = []
            self.result = None

        @property
        def gif_bot_id(self):
            return self._gif_bot_id

        @gif_bot_id.setter
        def gif_bot_id(self, value):
            self._gif_bot_id = value

        @property
        def state(self):
            return self._state

        def respond(self, client, rid, result):
            self.result = result

        def respond_error(self, client, rid, message, code=0):
            self.error = message

        def td_send(self, query, client=None, rid=None, transform=None, **kwargs):
            self.queries.append(query)
            answer = self.answers.pop(0) if self.answers else self.answer
            if self.strict_native_errors and answer.get("@type") == "error":
                # Match Daemon.handle_td, not a success transform applied to an
                # error object. Only the explicit supplement enables this path;
                # the historical frozen oracle stays byte-for-byte reproducible.
                self.respond_error(client, rid, answer.get("message", ""), answer.get("code", 0))
                return
            if answer.get("@type") == "error" and kwargs.get("failed"):
                kwargs["failed"](answer)
                return
            result = transform(answer) if transform else {}
            if client is not None:
                self.result = result

    updates = [{"@type": "updateNewChat", "chat": {"@type": "chat", "id": 42, "title": "fixture", "type": {"@type": "chatTypeSupergroup", "supergroup_id": 3}, "positions": [{"@type": "chatPosition", "list": {"@type": "chatListMain"}, "order": "9223372036854775807"}]}}]
    answer = {"@type": "message", "id": 13, "chat_id": 42, "message_thread_id": 8, "total_count": 3, "chat_ids": [42, 98], "content": {"@type": "messageText", "text": {"text": "fixture"}}, "messages": [{"@type": "message", "id": 2, "content": {"@type": "messageText", "text": {"text": "fixture"}}}], "can_be_copied": True, "can_get_viewers": True}
    valid = {"chatId": 42, "messageId": 13, "topicId": 1, "threadId": None, "userId": 7, "id": 3, "pinned": True, "unread": False, "archived": True, "silent": False, "chosen": True, "emoji": "😀", "seconds": 86400, "muteFor": -1, "query": "fixture", "text": "fixture", "optionIds": [0, 1], "date": 15, "callId": 1, "storyId": 1, "password": "synthetic", "messageIds": [1, 2]}
    command_names += ["message.rich", "message.delete", "file.download", "proxies.list", "proxy.enable", "proxy.remove", "proxy.disable", "password.get", "password.checkEmailCode", "password.resendEmailCode", "account.ttl", "account.setTtl", "autoDelete.default", "autoDelete.setDefault", "contacts.list", "contacts.search", "blocked.list", "blocked.unblock", "profile.setName", "profile.setBio", "profile.setUsername", "username.chat", "bot.start", "chat.joinLink", "session.terminate", "stickers.recent", "stickers.favorites", "stickers.sets", "stickers.set", "stickers.install", "sticker.favorite", "button.callback", "chat.action"]
    command_names += ["auth.phone", "auth.code", "folder.save", "folders.reorder", "proxy.add", "password.set", "notifications.get", "privacy.get", "folders.get", "chat.mediaCounts", "profile.get", "profile.deletePhoto", "message.markdown", "link.open", "gifs.search"]
    command_names += ["chat.info", "chat.mentions", "chat.commands", "chat.members", "chat.media", "group.create", "channel.create", "message.reactions", "customEmoji.get"]
    valid.update({"url": "https://t.me/synthetic", "name": "Fixture", "includeGroups": True, "server": "example.test", "port": 123, "oldPassword": "", "newPassword": "synthetic", "hint": "fixture", "title": "Fixture", "filter": "photos", "ids": ["123"], "fileId": 7, "setId": "9223372036854775807", "installed": True, "favorite": False, "firstName": "Fixture", "lastName": "Person", "bio": "synthetic", "username": "@fixture", "link": "https://t.me/+synthetic", "parameter": "", "code": "12345", "days": 365, "type": "user", "data": "Zg==", "action": "typing", "phone": "+12345"})
    mutations = [{}, valid]
    for key in ["chatId", "messageId", "topicId", "threadId", "query", "limit", "list", "pinned", "silent", "seconds", "offset", "messageIds", "text", "optionIds", "id", "setId", "installed", "favorite", "username", "bio", "firstName", "data", "code", "fileId", "priority", "action"]:
        for bad in [None, False, "12", 1.5, -9223372036854775808, 9223372036854775808]:
            mutations.append(dict(valid, **{key: bad}))
    mutations += [dict(valid, list=value) for value in ["folder:1", "folder:2147483647", "folder:2147483648", "folder:01", "archive"]]
    mutations += [dict(valid, query="x" * length) for length in [128, 129, 256, 257]]
    client = type("Peer", (), {"sock": type("Socket", (), {"fileno": lambda self: 7})()})()
    for command in command_names:
        if command not in backend.COMMANDS:
            raise AssertionError(f"migration command not in current registry: {command}")
        for args in mutations:
            oracle = Oracle(updates, answer)
            try:
                getattr(oracle, backend.COMMANDS[command][0])(client, 1, args)
                expected = {"queries": oracle.queries}
                expected["error" if oracle.error else "result"] = oracle.error if oracle.error else oracle.result
            except backend.BadRequest as error:
                expected = {"error": str(error)}
                if oracle.queries:
                    expected["queries"] = oracle.queries
            add("command", {"command": command, "args": args, "updates": updates, "answer": answer}, expected)
    for command, args, answers in [
        ("notifications.set", {"scope": "groups", "muted": True, "preview": False}, [{"@type": "scopeNotificationSettings", "sound_id": "9223372036854775807", "show_preview": True}, {"@type": "ok"}]),
        ("notifications.set", {"scope": "channels", "muted": None, "preview": True}, [{"@type": "scopeNotificationSettings", "mute_for": 123, "sound_id": "9223372036854775807", "show_preview": True}, {"@type": "ok"}]),
        ("privacy.set", {"setting": "status", "base": "nobody"}, [{"@type": "userPrivacySettingRules", "rules": [{"@type": "userPrivacySettingRuleAllowAll"}, {"@type": "userPrivacySettingRuleAllowUsers", "user_ids": [7]}]}, {"@type": "ok"}]),
        ("privacy.set", {"setting": "findByPhone", "base": "contacts"}, [{"@type": "userPrivacySettingRules", "rules": []}, {"@type": "ok"}]),
        ("proxy.addLink", {"link": "https://t.me/proxy?server=synthetic"}, [{"@type": "internalLinkTypeProxy", "proxy": {"@type": "proxy", "server": "example.test", "port": 123, "type": {"@type": "proxyTypeSocks5"}}}, {"@type": "addedProxy", "id": 1, "proxy": {"@type": "proxy", "server": "example.test", "port": 123}}]),
        ("proxy.addLink", {"link": "https://example.test"}, [{"@type": "internalLinkTypeUnknown"}]),
        ("proxy.ping", {"id": 1}, [{"@type": "proxies", "proxies": [{"@type": "addedProxy", "id": 1, "proxy": {"@type": "proxy", "server": "example.test", "port": 123}}]}, {"@type": "seconds", "seconds": 0.125}]),
        ("proxy.ping", {"id": 1}, [{"@type": "proxies", "proxies": []}]),
    ]:
        oracle = Oracle(updates, {})
        oracle.answers = list(answers)
        getattr(oracle, backend.COMMANDS[command][0])(object(), 1, args)
        expected = {"queries": oracle.queries}
        expected["error" if oracle.error else "result"] = oracle.error if oracle.error else oracle.result
        add("command", {"command": command, "args": args, "updates": updates, "answers": answers}, expected)
    for command, args, answers in [
        ("profile.get", {}, [{"@type": "user", "id": 7, "first_name": "Synthetic", "last_name": "Fixture", "profile_photo": {"@type": "profilePhoto", "id": "9223372036854775807"}}, {"bio": {"text": "Synthetic bio"}}]),
        ("profile.get", {}, [{"@type": "user", "id": 0}]),
        ("profile.deletePhoto", {}, [{"@type": "user", "id": 7, "profile_photo": {"@type": "profilePhoto", "id": "9223372036854775807"}}, {"@type": "ok"}]),
        ("profile.get", {}, [{"@type": "error", "code": 400, "message": "synthetic profile refusal"}]),
        ("gifs.search", {"chatId": 42, "query": "synthetic"}, [{"@type": "error", "code": 400, "message": "synthetic bot refusal"}]),
        ("link.open", {"url": "https://t.me/synthetic"}, [{"@type": "error", "code": 400, "message": "synthetic link refusal"}]),
        ("gifs.search", {"chatId": 42, "query": "synthetic"}, [{"type": {"user_id": 7}}, {"inline_query_id": "9223372036854775807", "results": [], "next_offset": "next"}]),
        ("link.open", {"url": "https://t.me/synthetic"}, [{"@type": "internalLinkTypeBotStart", "bot_username": "fixture", "start_parameter": "test"}, {"id": 42}]),
        ("link.open", {"url": "tg://synthetic"}, [{"@type": "internalLinkTypeMessage", "url": "https://t.me/synthetic/7"}, {"chat_id": 42, "message": {"@type": "message", "id": 7}}]),
        ("link.open", {"url": "https://t.me/+synthetic"}, [{"@type": "internalLinkTypeChatInvite", "invite_link": "https://t.me/+synthetic"}, {"title": "Fixture", "member_count": 3, "chat_id": 42}]),
        ("link.open", {"url": "https://t.me/proxy"}, [{"@type": "internalLinkTypeProxy", "proxy": {"@type": "proxy", "server": "example.test", "port": 123, "type": {"@type": "proxyTypeMtproto", "secret": "not exposed"}}}]),
    ]:
        oracle = Oracle(updates, {})
        oracle.answers = list(answers)
        getattr(oracle, backend.COMMANDS[command][0])(object(), 1, args)
        expected = {"queries": oracle.queries}
        expected["error" if oracle.error else "result"] = oracle.error if oracle.error else oracle.result
        add("command", {"command": command, "args": args, "updates": updates, "answers": answers}, expected)
    batch_updates = updates + [{"@type": "updateChatFolders", "chat_folders": [{"@type": "chatFolderInfo", "id": 10, "name": {"text": {"text": "ten"}}}, {"@type": "chatFolderInfo", "id": 2, "name": {"text": {"text": "two"}}}], "main_chat_list_position": 1}]
    for command, count in [("notifications.get", 3), ("privacy.get", 9), ("folders.get", 2), ("chat.mediaCounts", 6)]:
        for failing in [None, 0, count - 1]:
            oracle = Oracle(batch_updates, {})
            answers = [{"@type": "error", "code": 400, "message": "synthetic rejection"} if index == failing else {"count": index, "rules": [], "show_preview": True, "name": {"text": {"text": str(index)}}} for index in range(count)]
            oracle.answers = list(answers)
            getattr(oracle, backend.COMMANDS[command][0])(client, 1, {"chatId": 42})
            expected = {"queries": oracle.queries}
            expected["error" if oracle.error else "result"] = oracle.error if oracle.error else oracle.result
            add("command", {"command": command, "args": {"chatId": 42}, "updates": batch_updates, "answers": answers}, expected)
    from unittest import mock
    for command in ["message.send", "message.edit", "message.linkPreview", "message.reschedule", "message.forward", "message.sendSticker", "message.sendDice", "message.sendContact", "message.sendLocation", "message.sendGif", "message.translate", "message.sendPoll"]:
        typed = dict(valid, fromChatId=41, latitude=12.5, longitude=-43, emoji="🎲", to="pt-BR", question="Synthetic poll", options=["One", "Two"])
        for args in [{}, typed] + [dict(typed, **{key: bad}) for key in ["text", "silent", "sendAt", "linkPreview", "queryId", "latitude", "longitude", "to", "caption"] for bad in [None, False, "12", 1.5, -1, 20]]:
            oracle = Oracle(warmup, answer)
            with mock.patch.object(backend.time, "time", return_value=0), mock.patch.object(backend.td, "language_code", return_value="en"):
                try:
                    getattr(oracle, backend.COMMANDS[command][0])(object(), 1, args)
                    expected = {"result": oracle.result, "queries": oracle.queries}
                except backend.BadRequest as error:
                    expected = {"error": str(error)}
            add("command", {"command": command, "args": args, "updates": warmup, "answer": answer, "now": 0, "language": "en"}, expected)
    for value in [None, False, {}, prefs.empty()] + [{key: bad} for key in ["shortcuts", "globalShortcuts", "playbackRate", "autoDownload", "reactionsSeen", "showStories", "quiet", "chatListWidth", "emoji", "sounds"] for bad in [None, False, "bad", 1, 1.5, [], {}, {"bad": 1}]] + [
        {"shortcuts": {"chat.open": ["Ctrl+O", "Ctrl+O"]}},
        {"shortcuts": {"global.open": []}},
        {"globalShortcuts": {"global.quickReply": "super+alt+m", "global.panel": "SUPER + ALT + M"}},
        {"emoji": {"tone": 3, "recents": {"emoji:😀": {"c": 5.5, "t": 100.2}}}},
        {"sounds": {"style": "off", "variants": {"123": 3, "bad": 4}}},
    ]:
        for strict in [True, False]:
            try:
                expected = {"result": prefs.check(value, strict)}
            except ValueError as error:
                expected = {"error": str(error)}
            add("preferences", {"value": value, "strict": strict}, expected)
    for combo in [None, False, "", "super+alt+m", "ctrl+shift+F24", "super+F01", "shift+a", "alt+enter", "win+esc", "super+super+a", "ctrl+;", "super+pageup", "ctrl+\\"]:
        add("preferences", {"operation": "combo", "value": combo}, prefs.normalize_combo(combo))
    preference_commands = ["settings.set", "settings.playback", "settings.chatListWidth", "settings.reactions", "settings.emoji", "settings.quiet", "settings.sounds", "sounds.another", "settings.autoDownload"]
    preference_args = {"settings": {"showStories": False}, "rate": 1.5, "width": 230, "seen": False, "tone": 3, "recents": {}, "quiet": True, "style": "off", "userId": 7, "rules": {"photos": False, "gifs": True, "videos": 10, "files": 50}}
    for command in preference_commands:
        for args in [{}, preference_args] + [dict(preference_args, **{key: bad}) for key in preference_args for bad in [None, False, "bad", 1.5, {}, []]]:
            oracle = Oracle([], {})
            oracle.prefs = prefs.empty()
            oracle.global_status = {}
            oracle.apply_globals = lambda: None
            played = []
            oracle.play_person = lambda seed, force=False: played.append(seed)
            oracle.broadcast = lambda value: None
            current = dict(oracle.prefs)
            with mock.patch.object(prefs, "save", return_value=None):
                try:
                    getattr(oracle, backend.COMMANDS[command][0])(object(), 1, args)
                    expected = {"settings": oracle.prefs, "applyGlobals": oracle.prefs["globalShortcuts"] != current["globalShortcuts"], "play": played[0] if played else None}
                except backend.BadRequest as error:
                    expected = {"error": str(error)}
            add("preferences_command", {"command": command, "args": args, "current": current, "meId": 7}, expected)
    import omagram_td as td
    for value in [None, False, {}, {"accounts": []}, {"active": "bad", "accounts": [{"id": "default", "meId": True}, {"id": "default"}, {"id": "../bad"}, {"id": "acc_2", "meId": 9007199254740992}]}] + [{"active": bad, "accounts": [{"id": "acc_2", "name": bad, "phone": bad, "meId": bad}]} for bad in [None, False, "synthetic", 1.5, -1, 9007199254740991]]:
        with mock.patch.object(td.safe, "read_json", return_value=value):
            expected = td.load_accounts()
        add("accounts", value, expected)
    for path in ["/fixture/image.JPG", "/fixture/clip.mp4", "/fixture/clip.webm", "/fixture/music.wav", "/fixture/music.opus", "/fixture/data"]:
        for size in [1, 10 * 1024 * 1024, 10 * 1024 * 1024 + 1]:
            for as_media in [True, False]:
                add("upload_helper", {"path": path, "size": size, "asMedia": as_media}, backend.file_kind(path, size, as_media))
    for kind in ["photo", "video", "audio", "document"]:
        for caption in [None, {"@type": "formattedText", "text": "Synthetic caption", "entities": []}]:
            add("upload_helper", {"kind": "content", "fileKind": kind, "path": "/fixture/source", "caption": caption, "probed": [1, 80, 60]}, backend.file_content("/fixture/source", kind, caption, (1, 80, 60)))
    for kinds in [[], ["photo", "document", "video", "audio", "photo", "document"], ["photo"] * 25]:
        add("upload_helper", {"kind": "groups", "kinds": kinds}, backend.album_groups(kinds))
    for listed in [b"", b"# ignored\nfile:///tmp/a%20b\nfile://foreign/tmp/private\nfile:///tmp/a%20b\nfile://localhost/tmp/c\nfile:///tmp/a%00", b"FILE:///tmp/A\nfile:/tmp/B?x#fragment\nfile://LOCALHOST/tmp/no\nfile:///tmp/a%ZZ", b"file:///tmp/\xff\nfile:///tmp/%ff", "file:///tmp/😀\u0085file:///tmp/next".encode(), b"\n".join(f"file:///tmp/{i}".encode() for i in range(15))]:
        add("clipboard_paths", base64.b64encode(listed).decode(), backend.clipboard_file_paths(listed))
    for value in [None, False, "", "../.hidden", "c:\\folder\\name.txt", " leading . ", "...", "😀" * 250, "a\u200db\u00a0c\x00", "a\ue000b", "a\u2028b"]:
        add("download_name", value, backend.download_name(value))
    import omagram_notify as notify
    for text_value in ["", "  x\\n y ", "<a href='malicious'>text</a> & more", "😀" * 350, "a\\u2028b\\x7fc", "\\x00\\u0085", "a\\u00a0b"]:
        add("notification_helper", {"kind": "body", "text": text_value}, notify.body_markup(text_value))
        for limit in [1, 120, 300]:
            add("notification_helper", {"kind": "clean", "text": text_value, "limit": limit}, notify.clean(text_value, limit))
    import omagram_sounds as sounds
    for seed in [1, 7, -42, 777000, 9007199254740991]:
        for variant in [0, 1, 99]:
            add("sound_helper", {"seed": seed, "variant": variant}, sounds.motif(seed, variant))
    for seed in [1, 7, -42]:
        for style in ["pop", "drop", "knock"]:
            add("sound_helper", {"seed": seed, "operation": "wav", "style": style}, base64.b64encode(sounds.sound(seed, style)).decode())
    import omagram_media as helpers
    import struct
    with mock.patch.object(helpers.safe, "tool", return_value=pathlib.Path("/usr/bin/ffmpeg")):
        for kind in ["voice_argv", "video_note_argv", "note_animation_argv", "profile_photo_argv", "video_capture_argv"]:
            if kind == "voice_argv":
                expected = helpers.voice_argv("/fixture/target")
            elif kind == "video_capture_argv":
                expected = helpers.video_capture_argv("/fixture/target", "/fixture/preview")
            else:
                expected = getattr(helpers, kind)("/fixture/source", "/fixture/target")
            add("media_helper", {"kind": kind}, expected[1:])
    for count in [0, 1, 3, 100, 101, 1000]:
        pcm = b"".join(struct.pack("<h", rng.randrange(-32768, 32768)) for _ in range(count))
        for samples in [1, 3, 100]:
            levels = helpers.levels_from_pcm(pcm, samples)
            add("media_helper", {"kind": "pcm", "samples": samples, "data": base64.b64encode(pcm).decode()}, {"levels": levels, "packed": base64.b64encode(helpers.waveform_bytes(levels)).decode()})
    if len(sys.argv) > 1 and sys.argv[1] == "--valid-command-corpus":
        # A distinct supplement; never regenerate or silently change the frozen
        # baseline. These routes formerly had only malformed/error examples.
        bot = [{"@type": "updateUser", "user": {"@type": "user", "id": 7, "first_name": "Synthetic", "type": {"@type": "userTypeBot"}}}, {"@type": "updateNewChat", "chat": {"@type": "chat", "id": 43, "title": "Synthetic bot", "type": {"@type": "chatTypePrivate", "user_id": 7}}}]
        secret = [{"@type": "updateNewChat", "chat": {"@type": "chat", "id": 45, "title": "Synthetic secret", "type": {"@type": "chatTypeSecret", "user_id": 7, "secret_chat_id": 6}}}]
        folders = [{"@type": "updateChatFolders", "chat_folders": [{"@type": "chatFolderInfo", "id": i, "name": {"text": {"text": str(i)}}} for i in [10, 2]], "main_chat_list_position": 1}]
        supplements = [
            ("bot.start", {"chatId": 43, "parameter": "synthetic"}, bot, answer),
            ("secret.close", {"chatId": 45}, secret, {"@type": "ok"}),
            ("thread.history", {"chatId": 42, "threadId": 8, "limit": 20, "offset": -5}, warmup, {"@type": "messages", "messages": [{"@type": "message", "chat_id": 42, "id": 13, "content": {"@type": "messageText", "text": {"text": "synthetic thread"}}}]}),
            ("folder.save", {"name": "Synthetic", "includeGroups": True, "icon": "Work", "colorId": 3, "included": [42, 43, 42], "excluded": [44]}, warmup, {"@type": "chatFolderInfo", "id": 10}),
            ("folder.save", {"id": 10, "name": "Synthetic", "includeGroups": True}, folders, {"@type": "chatFolderInfo", "id": 10}),
            ("folders.reorder", {"ids": [2, 10]}, folders, {"@type": "ok"}),
            *[("proxy.add", dict({"server": "127.0.0.1", "port": 123, "type": kind}, **options), [], {"@type": "addedProxy", "id": 9, "is_enabled": True, "proxy": {"@type": "proxy", "server": "127.0.0.1", "port": 123, "type": {"@type": "proxyTypeSocks5"}}}) for kind, options in [("socks5", {"username": "synthetic", "password": "fixture"}), ("http", {"httpOnly": True}), ("mtproto", {"secret": "0123456789abcdef0123456789abcdef"})]],
        ]
        cases = []
        for command, args, initial, response in supplements:
            for outcome in [response, {"@type": "error", "code": 400, "message": "synthetic refusal"}]:
                oracle = Oracle(initial, outcome)
                oracle.strict_native_errors = True
                try:
                    getattr(oracle, backend.COMMANDS[command][0])(object(), 1, args)
                    expected = {"queries": oracle.queries}
                    expected["error" if oracle.error else "result"] = oracle.error if oracle.error else oracle.result
                except backend.BadRequest as error:
                    expected = {"error": str(error)}
                add("command", {"command": command, "args": args, "updates": initial, "answer": outcome}, expected)
    return cases


def main():
    supplement = len(sys.argv) == 3 and sys.argv[1] == "--valid-command-corpus"
    if len(sys.argv) != 2 and not supplement:
        raise SystemExit("Usage: differential.py [--valid-command-corpus] <exact-built-Rust-binary>|--emit-corpus")
    cases = corpus()
    binary = sys.argv[2] if supplement else sys.argv[1]
    if binary == "--emit-corpus":
        for case in cases:
            print(json.dumps(case, ensure_ascii=False, separators=(",", ":")))
        return 0
    process = subprocess.run([binary, "--project"], input="".join(json.dumps({k: v for k, v in c.items() if k != "expected"}, ensure_ascii=False) + "\n" for c in cases).encode(), stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=True)
    actual = [json.loads(line) for line in process.stdout.splitlines()]
    if len(actual) != len(cases):
        raise AssertionError(f"response count {len(actual)} != {len(cases)}")
    differences = []
    for index, (case, output) in enumerate(zip(cases, actual)):
        expected = json.loads(json.dumps(case["expected"]))
        if expected != output:
            differences.append({"case": index, "input": {k: v for k, v in case.items() if k != "expected"}, "python": expected, "rust": output})
    print(json.dumps({"inputs": len(cases), "differences": len(differences), "operations": sorted(set(c["operation"] for c in cases))}))
    if differences:
        from collections import Counter
        print(json.dumps({"difference_groups": dict(Counter(d["input"]["value"].get("command", d["input"]["operation"]) for d in differences))}))
    for difference in differences[:5]:
        print(json.dumps(difference, ensure_ascii=False))
    return 1 if differences else 0


if __name__ == "__main__":
    sys.exit(main())
