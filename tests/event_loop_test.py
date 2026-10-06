"""Queue fairness checks with synthetic events; no TDLib, network, keyring or shell."""
import os
import unittest
from unittest import mock

from daemon_test import load_daemon


class EventLoop(unittest.TestCase):
    def setUp(self):
        self.module = load_daemon()
        with mock.patch.object(self.module.prefs, "load", return_value=self.module.prefs.empty()):
            self.daemon = self.module.Daemon()
        self.addCleanup(self.daemon.sel.close)
        self.addCleanup(os.close, self.daemon.wake_r)
        self.addCleanup(os.close, self.daemon.wake_w)

    def test_continuous_td_updates_yield_to_socket_loop(self):
        processed = []

        def receive(event):
            processed.append(event)
            if len(processed) < 3000:
                self.daemon.events.put({"@type": "update"})

        self.daemon.handle_td = receive
        self.daemon.events.put({"@type": "update"})
        self.daemon.process_events()
        self.assertLessEqual(len(processed), 128, "A busy TDLib stream must yield before starving sockets")
        self.assertFalse(self.daemon.events.empty())

    def test_malformed_state_scalars_do_not_stop_following_updates(self):
        session = self.daemon.session_for()
        received = []
        self.daemon.broadcast = received.append
        for mid, text, location in (("--1", "bad id", None), (1, "bad\ud800 text", None),
                                    (2, "", {"@type": "location", "latitude": 10 ** 400, "longitude": 0})):
            with self.subTest(mid=mid):
                content = {"@type": "messageLocation", "location": location} if location else {
                    "@type": "messageText", "text": {"text": text}}
                self.daemon.events.put({"@type": "updateNewMessage", "@client_id": session.client_id,
                                        "message": {"@type": "message", "id": mid, "chat_id": 7, "content": content}})
                self.daemon.process_events()
        self.daemon.events.put({"@type": "updateNewMessage", "@client_id": session.client_id,
                                "message": {"@type": "message", "id": 3, "chat_id": 7,
                                            "content": {"@type": "messageText", "text": {"text": "still responsive"}}}})
        self.daemon.process_events()
        self.assertTrue(self.daemon.events.empty())
        self.assertEqual(received[-1]["message"]["content"]["text"], "still responsive")
        self.assertEqual(received[-1]["account"], session.id)
        self.assertEqual(len(received), 4)

    def test_malformed_title_is_encodable_by_actual_ipc_consumer(self):
        from types import SimpleNamespace
        session = self.daemon.session_for()
        client = SimpleNamespace(outbox=bytearray(), pid=0)
        self.daemon.clients[1] = client
        try:
            self.daemon.events.put({"@type": "updateNewChat", "@client_id": session.client_id,
                                    "chat": {"@type": "chat", "id": 7, "title": "bad\ud800 title",
                                             "type": {"@type": "chatTypePrivate", "user_id": 7}}})
            with mock.patch.object(self.daemon, "flush"):
                self.daemon.process_events()
            import json
            event = json.loads(client.outbox)
            self.assertEqual(event["chat"]["title"], "bad? title")
            self.assertEqual(event["account"], session.id)
        finally:
            self.daemon.clients.pop(1, None)

    def test_malformed_entity_url_is_encodable_by_actual_ipc_consumer(self):
        import json
        from types import SimpleNamespace
        session = self.daemon.session_for()
        client = SimpleNamespace(outbox=bytearray(), pid=0)
        self.daemon.clients[1] = client
        try:
            self.daemon.events.put({"@type": "updateNewMessage", "@client_id": session.client_id,
                "message": {"@type": "message", "id": 4, "chat_id": 7,
                    "content": {"@type": "messageText", "text": {"text": "link", "entities": [
                        {"offset": 0, "length": 4, "type": {"@type": "textEntityTypeTextUrl",
                            "url": "https://example.com/\ud800/path"}}]}}}})
            with mock.patch.object(self.daemon, "flush"):
                self.daemon.process_events()
            event = json.loads(client.outbox)
            entity = event["message"]["content"]["entities"][0]
            self.assertEqual(entity, {"type": "textUrl", "offset": 0, "length": 4,
                                      "url": "https://example.com/?/path"})
            self.assertEqual(event["account"], session.id)
        finally:
            self.daemon.clients.pop(1, None)

    def test_malformed_callback_data_cannot_break_actual_ipc_consumer(self):
        import json
        from types import SimpleNamespace
        session = self.daemon.session_for()
        client = SimpleNamespace(outbox=bytearray(), pid=0)
        self.daemon.clients[1] = client
        try:
            for data, expected in (("eWVz", "callback"), ("bad\ud800", "unsupported"),
                                   ("café", "unsupported"), ("действие", "unsupported")):
                client.outbox.clear()
                self.daemon.events.put({"@type": "updateNewMessage", "@client_id": session.client_id,
                    "message": {"@type": "message", "id": 5, "chat_id": 7,
                        "content": {"@type": "messageText", "text": {"text": "pick"}},
                        "reply_markup": {"@type": "replyMarkupInlineKeyboard", "rows": [[{
                            "@type": "inlineKeyboardButton", "text": "Synthetic button",
                            "type": {"@type": "inlineKeyboardButtonTypeCallback", "data": data}}]]}}})
                with mock.patch.object(self.daemon, "flush"):
                    self.daemon.process_events()
                event = json.loads(client.outbox)
                button = event["message"]["markup"]["rows"][0][0]
                self.assertEqual(button["kind"], expected)
                if expected == "callback":
                    self.assertEqual(button["data"], data)
                else:
                    self.assertNotIn("data", button)
        finally:
            self.daemon.clients.pop(1, None)

    def test_malformed_media_strings_cannot_break_actual_ipc_consumer(self):
        import json
        from types import SimpleNamespace
        session = self.daemon.session_for()
        session.state.files_root = "/synthetic/files"
        client = SimpleNamespace(outbox=bytearray(), pid=0)
        self.daemon.clients[1] = client
        try:
            for data, path in (("bad\ud800", "/synthetic/files/photo.jpg"),
                               ("AAAA", "/synthetic/files/bad\ud800.jpg")):
                client.outbox.clear()
                self.daemon.events.put({"@type": "updateNewMessage", "@client_id": session.client_id,
                    "message": {"@type": "message", "id": 6, "chat_id": 7,
                        "content": {"@type": "messagePhoto", "photo": {"@type": "photo",
                            "minithumbnail": {"@type": "minithumbnail", "data": data},
                            "sizes": [{"@type": "photoSize", "width": 320, "height": 200,
                                "photo": {"@type": "file", "id": 9, "local": {
                                    "path": path, "is_downloading_completed": True}}}]}}}})
                with mock.patch.object(self.daemon, "flush"):
                    self.daemon.process_events()
                media = json.loads(client.outbox)["message"]["content"]["media"]
                if not data.isascii():
                    self.assertIsNone(media["mini"])
                    self.assertEqual(media["file"]["path"], path)
                else:
                    self.assertEqual(media["mini"]["data"], data)
                    self.assertEqual(media["file"]["path"], "")
        finally:
            self.daemon.clients.pop(1, None)

    def test_sticker_conversion_is_deferred_and_bounded(self):
        work = []
        responses = []
        self.daemon.respond = lambda *args: responses.append(args)
        self.daemon.background = lambda task, done, **kwargs: work.append((task, done))
        self.daemon.td_send = lambda query, client, rid, transform: setattr(self, "convert", transform)
        with mock.patch.object(self.module.model, "file_view", return_value={"path": "/synthetic/sticker.tgs"}), \
                mock.patch.object(self.module, "lottie_json", return_value="/synthetic/sticker.json") as inflate:
            self.daemon.cmd_sticker_lottie(None, 7, {"fileId": 1})
            self.assertIs(self.convert({}), self.module.DEFERRED)
            inflate.assert_not_called()
            task, done = work.pop()
            done(task(), None)
            self.assertEqual(responses, [(None, 7, {"path": "/synthetic/sticker.json"})])
            self.daemon.jobs = self.module.JOBS_MAX
            with self.assertRaises(self.module.BadRequest):
                self.convert({})
            self.assertEqual(work, [])

    def test_optional_media_work_respects_admission_limit(self):
        self.daemon.jobs = self.module.JOBS_MAX
        with mock.patch.object(self.daemon, "background") as background, \
                mock.patch.object(self.module, "local_upload", return_value=("/synthetic/video.mp4", 10)), \
                mock.patch.object(self.module, "file_kind", return_value="video"), \
                mock.patch.object(self.daemon, "typed_text", return_value=None), \
                mock.patch.object(self.module.media, "new_path", return_value="/synthetic/profile.jpg") as target:
            for command, args in (
                    (self.daemon.cmd_profile_set_photo, {"path": "/synthetic/photo.png"}),
                    (self.daemon.cmd_message_send_file, {"chatId": 42, "path": "/synthetic/video.mp4"}),
                    (self.daemon.cmd_message_send_files, {"chatId": 42, "paths": ["/synthetic/video.mp4"]})):
                with self.subTest(command=command.__name__):
                    with self.assertRaises(self.module.BadRequest):
                        command(None, 7, args)
            target.assert_not_called()
            with mock.patch.object(self.module.os.path, "isfile", return_value=False):
                self.daemon.play_person(42, force=True)
            background.assert_not_called()
            self.daemon.jobs = self.module.JOBS_MAX - 1
            self.daemon.cmd_profile_set_photo(None, 7, {"path": "/synthetic/photo.png"})
            background.assert_called_once()

    def test_qr_encoding_yields_and_cannot_replace_newer_auth(self):
        work = []
        self.daemon.background = lambda task, done, session=None: work.append((task, done))
        self.daemon.broadcast = lambda event: None
        session = self.daemon.session_for()
        value = {"@type": "authorizationStateWaitOtherDeviceConfirmation", "link": "tg://login?token=AQIDBAUG"}
        with mock.patch.object(self.daemon, "with_qr", side_effect=lambda auth: dict(auth, image="synthetic-png")) as encode:
            self.daemon.on_auth(session, value)
            encode.assert_not_called()
            self.assertEqual(session.auth["state"], "qr")
            self.assertTrue(session.auth["imagePending"])
            task, done = work.pop()
            done(task(), None)
            self.assertEqual(session.auth["image"], "synthetic-png")
            self.assertFalse(session.auth["imagePending"])
            self.daemon.on_auth(session, value)
            task, done = work.pop()
            self.daemon.set_auth(session, {"state": "phone"})
            done(task(), None)
            self.assertEqual(session.auth, {"state": "phone"})
            self.daemon.on_auth(session, value)
            task, done = work.pop()
            newer = dict(value, link="tg://login?token=BQIDBAUG")
            self.daemon.on_auth(session, newer)
            done(task(), None)
            self.assertEqual(session.auth["link"], newer["link"])
            task, done = work.pop()
            done(None, RuntimeError("synthetic encoding failure"))
            self.assertFalse(session.auth["imagePending"])
            self.daemon.jobs = self.module.JOBS_MAX
            self.daemon.on_auth(session, value)
            self.assertEqual(work, [])
            self.assertEqual(session.auth["state"], "qr")
            self.assertFalse(session.auth["imagePending"])


if __name__ == "__main__":
    unittest.main()
