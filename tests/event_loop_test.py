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

    def test_sticker_conversion_is_deferred_and_bounded(self):
        work = []
        responses = []
        self.daemon.respond = lambda *args: responses.append(args)
        self.daemon.background = lambda task, done: work.append((task, done))
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
