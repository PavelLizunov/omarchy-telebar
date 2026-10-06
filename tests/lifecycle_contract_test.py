"""Deterministic backend lifecycle checks with inert transport and completed jobs."""
import os
import unittest
from types import SimpleNamespace
from unittest import mock

from daemon_test import FakeTd, load_daemon


class LifecycleContract(unittest.TestCase):
    def setUp(self):
        self.m = load_daemon()
        with mock.patch.object(self.m.prefs, "load", return_value=self.m.prefs.empty()):
            self.d = self.m.Daemon()
        self.addCleanup(self.d.sel.close)
        self.addCleanup(os.close, self.d.wake_r)
        self.addCleanup(os.close, self.d.wake_w)
        self.d.td = FakeTd()
        self.s = self.d.session_for()
        self.s.client_id = 1
        self.d.by_client_id[1] = self.s
        self.replies = []
        self.d.respond_error = lambda *args: self.replies.append(args)

    def answer(self, query, kind="ok"):
        self.d.handle_td({"@type": kind, "@client_id": 1, "@extra": query["@extra"]})

    def test_unanswered_transport_has_bounded_admission(self):
        failed = []
        for _ in range(1200):
            self.d.td_send({"@type": "getMe"}, transform=lambda _: None, failed=failed.append)
        self.assertLess(len(self.d.pending), 1200)
        self.assertEqual(len(self.d.pending), self.m.TD_PENDING_MAX)
        self.assertEqual(len(self.d.td.sent), self.m.TD_PENDING_MAX)
        self.assertEqual(len(failed), 1200 - self.m.TD_PENDING_MAX)
        self.assertEqual(set(self.d.pending), set(self.d.td_times))

    def test_expiry_retires_before_failure_and_ignores_late_duplicate(self):
        successes, failures = [], []
        def failed(event):
            self.assertFalse(self.d.pending)
            self.assertFalse(self.d.td_times)
            failures.append(event)
        with mock.patch.object(self.m.time, "monotonic", return_value=10):
            self.d.td_send({"@type": "sendMessage"}, transform=successes.append, failed=failed)
        query = self.d.td.sent[-1]
        self.d.expire_requests(now=10 + self.m.REQUEST_TIMEOUT - 0.01)
        self.assertEqual(len(self.d.pending), 1)
        self.d.expire_requests(now=10 + self.m.REQUEST_TIMEOUT)
        self.d.expire_requests(now=9999)
        self.answer(query)
        self.answer(query)
        self.assertEqual(successes, [])
        self.assertEqual(len(failures), 1)
        self.assertIn("may still complete", failures[0]["message"])

    def test_timeout_callbacks_keep_account_and_cannot_break_sweep(self):
        second = self.m.AccountSession("synthetic")
        second.client_id = 2
        self.d.accounts[second.id] = second
        observed = []
        def broken(_):
            observed.append(self.d.session_for().id)
            raise RuntimeError("synthetic")
        with mock.patch.object(self.m.time, "monotonic", return_value=10):
            self.d.td_send({"@type": "getMe"}, transform=lambda _: None, failed=broken)
            self.d.td_send({"@type": "getMe"}, transform=lambda _: None,
                           failed=lambda _: observed.append(self.d.session_for().id), session=second)
        self.d.expire_requests(now=9999)
        self.assertEqual(observed, [self.s.id, second.id])
        self.assertIsNone(self.d.current_session)
        self.assertEqual(self.d.pending, {})

    def test_success_and_send_exception_have_one_terminal_callback(self):
        got, failed = [], []
        self.d.td_send({"@type": "getMe"}, transform=got.append, failed=failed.append)
        query = self.d.td.sent[-1]
        self.answer(query)
        self.answer(query)
        with mock.patch.object(self.d.td, "send_client", side_effect=RuntimeError("secret")):
            self.d.td_send({"@type": "getMe"}, transform=got.append, failed=failed.append)
        self.assertEqual(len(got), 1)
        self.assertEqual(len(failed), 1)
        self.assertNotIn("secret", failed[0]["message"])
        self.assertEqual(self.d.pending, {})
        self.assertEqual(self.d.td_times, {})

    def test_malformed_td_transform_is_one_failure_not_success(self):
        import socket
        for direct in (False, True):
            with self.subTest(direct=direct):
                server, peer = socket.socketpair()
                self.addCleanup(peer.close)
                self.addCleanup(server.close)
                client = self.m.Client(server)
                self.d.clients[server.fileno()] = client
                replies = []
                self.d.send_to = lambda _client, message: replies.append(message)
                self.d.respond_error = self.m.Daemon.respond_error.__get__(self.d)
                self.d.request_times[(client, 7)] = (10, "synthetic", self.s, 1, 1)
                self.d.current_request = (client, 7, 1)
                def broken(_event):
                    raise ValueError("synthetic secret must not reach IPC")
                self.d.td_send({"@type": "getMe"}, client if direct else None,
                               7 if direct else None, transform=broken)
                query = self.d.td.sent[-1]
                self.d.current_request = None
                self.answer(query)
                self.answer(query)
                self.assertEqual(len(replies), 1)
                self.assertFalse(replies[0]["ok"])
                self.assertNotIn("secret", str(replies[0]))
                self.assertNotIn((client, 7), self.d.request_times)
                self.assertIsNone(self.d.current_request)
                self.assertIsNone(self.d.current_session)

    def test_td_failure_callback_exception_fails_captured_request_once(self):
        import socket
        for stale in (False, True):
            with self.subTest(stale=stale):
                server, peer = socket.socketpair()
                self.addCleanup(peer.close)
                self.addCleanup(server.close)
                client = self.m.Client(server)
                self.d.clients[server.fileno()] = client
                replies = []
                self.d.send_to = lambda _client, message: replies.append(message)
                self.d.respond_error = self.m.Daemon.respond_error.__get__(self.d)
                self.d.request_times[(client, 7)] = (10, "synthetic", self.s, self.s.client_id, 1)
                self.d.current_request = (client, 7, 1)
                def broken(*_):
                    raise ValueError("synthetic secret")
                self.d.td_send({"@type": "getMe"}, transform=lambda _: None,
                               failed=broken, cancelled=broken)
                self.d.current_request = None
                if stale:
                    self.s.client_id += 1
                extra = self.d.td.sent[-1]["@extra"]
                entry = self.d.pending.pop(extra)
                self.d.td_times.pop(extra)
                self.d.fail_td(entry, "synthetic transport failure")
                self.d.fail_td(entry, "synthetic duplicate")
                self.assertEqual(len(replies), 1)
                self.assertFalse(replies[0]["ok"])
                self.assertNotIn("secret", str(replies[0]))
                self.assertEqual(self.d.request_times, {})
                self.assertIsNone(self.d.current_request)
                self.assertIsNone(self.d.current_session)

    def test_failure_cleanup_cannot_fail_reused_request_id(self):
        import socket
        for broken_cleanup in (False, True):
            with self.subTest(broken_cleanup=broken_cleanup):
                server, peer = socket.socketpair()
                self.addCleanup(peer.close)
                self.addCleanup(server.close)
                client = self.m.Client(server)
                self.d.clients[server.fileno()] = client
                replies, cleaned = [], []
                self.d.send_to = lambda _client, message: replies.append(message)
                self.d.respond_error = self.m.Daemon.respond_error.__get__(self.d)
                self.d.request_times[(client, 7)] = (10, "old", self.s, self.s.client_id, 1)
                self.d.current_request = (client, 7, 1)
                def cleanup():
                    cleaned.append(True)
                    if broken_cleanup:
                        raise OSError("synthetic secret")
                self.d.td_send({"@type": "getMe"}, transform=lambda _: None, cancelled=cleanup)
                extra = self.d.td.sent[-1]["@extra"]
                entry = self.d.pending.pop(extra)
                self.d.td_times.pop(extra)
                self.d.request_times[(client, 7)] = (20, "new", self.s, self.s.client_id, 2)
                outer = (client, 99, 3)
                self.d.current_request = outer
                self.d.fail_td(entry, "synthetic failure")
                self.assertEqual(cleaned, [True])
                self.assertEqual(replies, [])
                self.assertEqual(self.d.request_times[(client, 7)][4], 2)
                self.assertIs(self.d.current_request, outer)
                self.assertIsNone(self.d.current_session)
                self.d.request_times.pop((client, 7))
                self.d.fail_td(entry, "synthetic late failure")
                self.assertEqual(replies, [])

    def test_stale_td_generation_cleans_without_business_callback(self):
        got, cancelled = [], []
        self.d.td_send({"@type": "getMe"}, transform=got.append, cancelled=lambda: cancelled.append(True))
        query = self.d.td.sent[-1]
        self.s.client_id = 2
        self.answer(query)
        self.assertEqual(got, [])
        self.assertEqual(cancelled, [True])
        self.assertEqual(self.d.pending, {})

    def test_background_stale_bridge_releases_lock_without_new_td_request(self):
        self.d.bridge = SimpleNamespace(start=lambda: 1443, stop=lambda: None, metadata={}, secret="00")
        self.d.bridge_restore.add(self.s.id)
        with mock.patch.object(self.m.threading, "Thread") as thread:
            self.d.enable_bridge(None, None, self.s)
            thread.call_args.kwargs["target"]()
        self.assertIn(self.s.id, self.d.bridge_busy)
        self.s.client_id = 2
        self.d.run_tasks()
        self.assertEqual(self.d.jobs, 0)
        self.assertNotIn(self.s.id, self.d.bridge_busy)
        self.assertEqual(self.d.td.sent, [])
        self.assertIn(self.s.id, self.d.bridge_restore)

    def test_background_cancel_cleanup_runs_once_even_if_account_object_replaced(self):
        done, cleaned = [], []
        with mock.patch.object(self.m.threading, "Thread") as thread:
            self.d.background(lambda: 42, lambda *args: done.append(args),
                              cancelled=lambda result, error: cleaned.append((result, error)))
            thread.call_args.kwargs["target"]()
        replacement = self.m.AccountSession(self.s.id)
        replacement.client_id = 1
        self.d.accounts[self.s.id] = replacement
        self.d.run_tasks()
        self.d.run_tasks()
        self.assertEqual(done, [])
        self.assertEqual(cleaned, [(42, None)])
        self.assertEqual(self.d.jobs, 0)
        self.assertIsNone(self.d.current_session)

    def test_account_closed_retires_resource_request_without_failure_on_new_session(self):
        cancelled, failed = [], []
        self.d.td_send({"@type": "getProxies"}, transform=lambda _: None, failed=failed.append,
                       cancelled=lambda: cancelled.append(True))
        query = self.d.td.sent[-1]
        self.d.on_auth(self.s, {"@type": "authorizationStateClosed"})
        self.assertEqual(cancelled, [True])
        self.assertEqual(failed, [])
        self.answer(query)
        self.assertEqual(cancelled, [True])
        self.assertEqual(self.d.pending, {})
        self.assertEqual(self.s.client_id, 2)

    def test_stale_clipboard_files_cleanup_preserves_original_uploads(self):
        for result, expected in [({"paths": ["/synthetic/recordings/paste.png"], "skipped": 0,
                                   "_generated": "/synthetic/recordings/paste.png"},
                                  ["/synthetic/recordings/paste.png"]),
                                 ({"paths": ["/synthetic/user/report.pdf"], "skipped": 0}, []),
                                 ({"paths": ["/synthetic/recordings/existing.ogg"], "skipped": 0}, [])]:
            with self.subTest(result=result):
                removed = []
                with mock.patch.object(self.d, "clipboard_files", return_value=result), \
                     mock.patch.object(self.m.media, "remove", side_effect=removed.append), \
                     mock.patch.object(self.m.threading, "Thread") as thread:
                    self.d.cmd_clipboard_files(None, 7, {})
                    thread.call_args.kwargs["target"]()
                    self.s.client_id += 1
                    self.d.run_tasks()
                self.assertEqual(removed, expected)
        # The owning media guard, not arbitrary caller-provided paths, decides
        # which outputs may be deleted. Original uploads are outside its roots.
        with mock.patch.object(self.m.os, "unlink") as unlink:
            self.m.media.remove("/synthetic/user/report.pdf")
            unlink.assert_not_called()

    def test_profile_conversion_and_td_cancel_delete_only_generated_copy(self):
        for after_conversion in (False, True):
            with self.subTest(after_conversion=after_conversion):
                removed = []
                with mock.patch.object(self.m, "local_upload", return_value=("/synthetic/source.png", 42)), \
                     mock.patch.object(self.m.media, "new_path", return_value="/synthetic/profile.jpg"), \
                     mock.patch.object(self.m.media, "prepare_profile_photo", return_value=None), \
                     mock.patch.object(self.m.media, "remove", side_effect=removed.append), \
                     mock.patch.object(self.m.threading, "Thread") as thread:
                    self.d.cmd_profile_set_photo(None, 7, {"path": "/synthetic/source.png"})
                    thread.call_args.kwargs["target"]()
                    if after_conversion:
                        self.d.run_tasks()
                        self.assertEqual(self.d.td.sent[-1]["@type"], "setProfilePhoto")
                        self.d.on_auth(self.s, {"@type": "authorizationStateClosed"})
                    else:
                        self.s.client_id += 1
                        self.d.run_tasks()
                    self.assertEqual(removed, ["/synthetic/profile.jpg"])
                    self.assertNotIn("/synthetic/source.png", removed)

    def test_recording_stop_preserves_active_recording_when_jobs_are_full(self):
        for kind in ("voice", "video"):
            for send in (True, False):
                with self.subTest(kind=kind, send=send):
                    proc = mock.Mock()
                    rec = {"kind": kind, "proc": proc, "path": "/synthetic/recording",
                           "preview": "/synthetic/preview", "chatId": 42, "account": self.s.id}
                    self.d.recording = rec
                    self.d.jobs = self.m.JOBS_MAX
                    stop = self.d.cmd_voice_stop if kind == "voice" else self.d.cmd_videonote_stop
                    with mock.patch.object(self.d, "background") as background, \
                         mock.patch.object(self.d, "broadcast") as broadcast, \
                         mock.patch.object(self.m.media, "new_sent_path") as target:
                        with self.assertRaises(self.m.BadRequest):
                            stop(None, 7, {"send": send})
                        self.assertIs(self.d.recording, rec)
                        proc.send_signal.assert_not_called()
                        broadcast.assert_not_called()
                        target.assert_not_called()
                        background.assert_not_called()
                        self.assertEqual(self.d.jobs, self.m.JOBS_MAX)
                        self.d.jobs -= 1
                        stop(None, 8, {"send": send})
                        self.assertIsNone(self.d.recording)
                        proc.send_signal.assert_called_once()
                        broadcast.assert_called_once()
                        background.assert_called_once()

    def test_video_stop_allocation_failure_preserves_recording_for_retry(self):
        proc = mock.Mock()
        rec = {"kind": "video", "proc": proc, "path": "/synthetic/recording",
               "preview": "/synthetic/preview", "chatId": 42, "account": self.s.id}
        self.d.recording = rec
        with mock.patch.object(self.d, "background") as background, \
             mock.patch.object(self.d, "broadcast") as broadcast, \
             mock.patch.object(self.m.media, "new_sent_path", side_effect=OSError("synthetic disk failure")):
            with self.assertRaises(OSError):
                self.d.cmd_videonote_stop(None, 7, {"send": True})
            self.assertIs(self.d.recording, rec)
            proc.send_signal.assert_not_called()
            broadcast.assert_not_called()
            background.assert_not_called()
        with mock.patch.object(self.d, "background") as background, \
             mock.patch.object(self.d, "broadcast"), \
             mock.patch.object(self.m.media, "new_sent_path", return_value="/synthetic/target"):
            self.d.cmd_videonote_stop(None, 8, {"send": True})
            self.assertIsNone(self.d.recording)
            proc.send_signal.assert_called_once()
            background.assert_called_once()

    def test_recording_stale_completion_removes_owned_output_without_send(self):
        for kind in ("voice", "video"):
            with self.subTest(kind=kind):
                proc = mock.Mock()
                rec = {"kind": kind, "proc": proc, "path": "/synthetic/recording",
                       "preview": "/synthetic/preview", "chatId": 42, "account": self.s.id}
                self.d.recording = rec
                removed = []
                with mock.patch.object(self.m.media, "new_sent_path", return_value="/synthetic/target"), \
                     mock.patch.object(self.m.media, "prepare_voice", return_value=(2, "waveform")), \
                     mock.patch.object(self.m.media, "prepare_video_note", return_value=2), \
                     mock.patch.object(self.m.media, "remove", side_effect=removed.append), \
                     mock.patch.object(self.m.threading, "Thread") as thread:
                    stop = self.d.cmd_voice_stop if kind == "voice" else self.d.cmd_videonote_stop
                    stop(None, 7, {"send": True})
                    thread.call_args.kwargs["target"]()
                    self.s.client_id += 1
                    self.d.run_tasks()
                self.assertEqual(self.d.jobs, 0)
                self.assertEqual(self.d.td.sent, [])
                self.assertEqual(removed, ["/synthetic/recording"] if kind == "voice" else
                                 ["/synthetic/preview", "/synthetic/recording", "/synthetic/target"])

    def test_disable_bridge_td_cancel_releases_lock_preserves_metadata(self):
        self.d.bridge = SimpleNamespace(metadata={self.s.id: {"proxy_id": 4}}, status=lambda _: {})
        self.d.disable_bridge(None, 7, self.s)
        query = self.d.td.sent[-1]
        self.assertIn(self.s.id, self.d.bridge_busy)
        self.d.on_auth(self.s, {"@type": "authorizationStateClosed"})
        self.assertNotIn(self.s.id, self.d.bridge_busy)
        self.assertEqual(self.d.bridge.metadata[self.s.id], {"proxy_id": 4})
        self.answer(query)
        self.assertNotIn(self.s.id, self.d.bridge_busy)

    def test_multistep_ipc_close_answers_once_and_does_not_strand_request(self):
        import json
        import socket
        server, peer = socket.socketpair()
        self.addCleanup(peer.close)
        self.addCleanup(server.close)
        client = self.m.Client(server)
        self.d.clients[server.fileno()] = client
        replies = []
        self.d.send_to = lambda _client, message: replies.append(message)
        self.d.respond_error = self.m.Daemon.respond_error.__get__(self.d)
        self.s.auth = {"state": "ready"}
        self.d.handle_line(client, json.dumps({"id": 1, "cmd": "profile.get"}).encode())
        query = self.d.td.sent[-1]
        self.assertIn((client, 1), self.d.request_times)
        self.d.on_auth(self.s, {"@type": "authorizationStateClosed"})
        terminal = [r for r in replies if r.get("id") == 1]
        self.assertEqual(len(terminal), 1)
        self.assertFalse(terminal[0]["ok"])
        self.assertNotIn((client, 1), self.d.request_times)
        self.answer(query)
        self.assertEqual(len([r for r in replies if r.get("id") == 1]), 1)

    def test_expired_ipc_job_cannot_respond_to_reused_id_or_send(self):
        import socket
        server, peer = socket.socketpair()
        self.addCleanup(peer.close)
        self.addCleanup(server.close)
        client = self.m.Client(server)
        self.d.clients[server.fileno()] = client
        replies, business, cleanup = [], [], []
        self.d.send_to = lambda _client, message: replies.append(message)
        self.d.respond_error = self.m.Daemon.respond_error.__get__(self.d)
        self.d.request_times[(client, 7)] = (10, "synthetic", self.s, 1, 1)
        self.d.current_request = (client, 7, 1)
        with mock.patch.object(self.m.threading, "Thread") as thread:
            self.d.background(lambda: 42, lambda *args: business.append(args),
                              cancelled=lambda *_: cleanup.append(True))
            thread.call_args.kwargs["target"]()
        self.d.current_request = None
        self.d.expire_requests(now=10 + self.m.REQUEST_TIMEOUT)
        self.assertEqual(len(replies), 1)
        self.assertEqual(self.d.request_times, {})
        self.d.request_times[(client, 7)] = (200, "new", self.s, 1, 2)
        self.d.run_tasks()
        self.assertEqual(cleanup, [True])
        self.assertEqual(business, [])
        self.assertEqual(len(replies), 1)
        self.assertEqual(self.d.request_times[(client, 7)][4], 2)

    def test_expired_export_keeps_completed_download_without_second_reply(self):
        import socket
        server, peer = socket.socketpair()
        self.addCleanup(peer.close)
        self.addCleanup(server.close)
        client = self.m.Client(server)
        self.d.clients[server.fileno()] = client
        replies, removed = [], []
        self.d.send_to = lambda _client, message: replies.append(message)
        self.d.respond_error = self.m.Daemon.respond_error.__get__(self.d)
        self.d.request_times[(client, 7)] = (10, "file.save", self.s, 1, 1)
        self.d.current_request = (client, 7, 1)
        with mock.patch.object(self.d, "downloaded_path", return_value="/synthetic/cache/original"), \
             mock.patch.object(self.d, "checked_file_action", side_effect=lambda c, r, a, action: action()), \
             mock.patch.object(self.m, "copy_out", return_value="/synthetic/Downloads/completed.bin") as copy, \
             mock.patch.object(self.m.media, "remove", side_effect=removed.append), \
             mock.patch.object(self.m.threading, "Thread") as thread:
            self.d.cmd_file_save(client, 7, {"chatId": 42, "messageId": 9, "fileId": 5,
                                           "fileName": "completed.bin"})
            self.d.handle_td({"@type": "file", "id": 5, "@client_id": 1,
                              "@extra": self.d.td.sent[-1]["@extra"]})
            self.d.current_request = None
            self.d.expire_requests(now=10 + self.m.REQUEST_TIMEOUT)
            thread.call_args.kwargs["target"]()
            self.d.run_tasks()
            copy.assert_called_once()
        self.assertEqual(len(replies), 1)
        self.assertFalse(replies[0]["ok"])
        self.assertEqual(removed, [])
        self.assertEqual(self.d.request_times, {})

    def test_invalid_command_cannot_consume_an_outstanding_request_id(self):
        import json
        import socket
        server, peer = socket.socketpair()
        self.addCleanup(peer.close)
        self.addCleanup(server.close)
        client = self.m.Client(server)
        self.d.clients[server.fileno()] = client
        replies = []
        self.d.send_to = lambda _client, message: replies.append(message)
        self.d.respond_error = self.m.Daemon.respond_error.__get__(self.d)
        self.s.auth = {"state": "ready"}
        self.d.handle_line(client, json.dumps({"id": 7, "cmd": "profile.get"}).encode())
        self.d.handle_line(client, json.dumps({"id": 7, "cmd": "unknown-synthetic"}).encode())
        self.assertNotIn(server.fileno(), self.d.clients)
        self.assertNotIn((client, 7), self.d.request_times)
        self.assertFalse(any(r.get("id") == 7 for r in replies))

    def test_ipc_admission_and_reused_pending_id_are_bounded(self):
        import json
        import socket
        server, peer = socket.socketpair()
        self.addCleanup(peer.close)
        self.addCleanup(server.close)
        client = self.m.Client(server)
        self.d.clients[server.fileno()] = client
        replies = []
        self.d.send_to = lambda _client, message: replies.append(message)
        self.d.respond_error = self.m.Daemon.respond_error.__get__(self.d)
        self.s.auth = {"state": "ready"}
        with mock.patch.object(self.m, "IPC_PENDING_MAX", 2):
            for rid in (1, 2, 3):
                self.d.handle_line(client, json.dumps({"id": rid, "cmd": "profile.get"}).encode())
        self.assertEqual(len(self.d.request_times), 2)
        self.assertEqual(len(self.d.td.sent), 2)
        self.assertFalse([r for r in replies if r.get("id") == 3][0]["ok"])
        self.d.handle_line(client, json.dumps({"id": 1, "cmd": "profile.get"}).encode())
        self.assertEqual(server.fileno(), -1)
        self.assertEqual(self.d.request_times, {})
        self.d.expire_requests(now=10**9)
        self.assertEqual(self.d.pending, {})

    def test_stale_request_still_gets_one_terminal_error(self):
        import socket
        server, peer = socket.socketpair()
        self.addCleanup(peer.close)
        self.addCleanup(server.close)
        client = self.m.Client(server)
        self.d.clients[server.fileno()] = client
        replies = []
        self.d.send_to = lambda _client, message: replies.append(message)
        self.d.respond_error = self.m.Daemon.respond_error.__get__(self.d)
        self.d.request_times[(client, 7)] = (10, "synthetic", self.s, 1, 1)
        self.d.current_request = (client, 7, 1)
        with mock.patch.object(self.m.threading, "Thread") as thread:
            self.d.background(lambda: 42, lambda *_: self.fail("stale business callback"),
                              cancelled=lambda *_: self.d.cancel_work(client, 7))
            thread.call_args.kwargs["target"]()
        self.d.current_request = None
        self.s.client_id = 2
        self.d.run_tasks()
        self.assertEqual(len(replies), 1)
        self.assertFalse(replies[0]["ok"])
        self.assertEqual(self.d.request_times, {})

    def test_background_callback_exception_fails_captured_request_once(self):
        import socket
        server, peer = socket.socketpair()
        self.addCleanup(peer.close)
        self.addCleanup(server.close)
        client = self.m.Client(server)
        self.d.clients[server.fileno()] = client
        replies = []
        self.d.send_to = lambda _client, message: replies.append(message)
        self.d.respond_error = self.m.Daemon.respond_error.__get__(self.d)
        self.d.request_times[(client, 7)] = (10, "synthetic", self.s, 1, 1)
        self.d.current_request = (client, 7, 1)
        def broken(*_):
            raise ValueError("synthetic secret")
        with mock.patch.object(self.m.threading, "Thread") as thread:
            self.d.background(lambda: 42, broken)
            thread.call_args.kwargs["target"]()
        self.d.current_request = None
        self.d.run_tasks()
        self.d.run_tasks()
        self.assertEqual(len(replies), 1)
        self.assertFalse(replies[0]["ok"])
        self.assertNotIn("secret", str(replies[0]))
        self.assertEqual(self.d.request_times, {})
        self.assertEqual(self.d.jobs, 0)
        self.assertIsNone(self.d.current_request)
        self.assertIsNone(self.d.current_session)

    def test_nested_task_dispatch_restores_outer_context(self):
        outer = self.m.AccountSession("outer")
        self.d.current_session = outer
        self.d.current_request = None
        seen = []
        with mock.patch.object(self.m.threading, "Thread") as thread:
            self.d.background(lambda: 42, lambda *_: (self.d.run_tasks(), seen.append(self.d.current_session)), session=self.s)
            thread.call_args.kwargs["target"]()
        self.d.run_tasks()
        self.assertEqual(seen, [self.s])
        self.assertIs(self.d.current_session, outer)

    def test_direct_cancellation_cleanup_sends_one_reply(self):
        import socket
        server, peer = socket.socketpair()
        self.addCleanup(peer.close)
        self.addCleanup(server.close)
        client = self.m.Client(server)
        self.d.clients[server.fileno()] = client
        self.d.td_send({"@type": "getMe"}, client, 7, cancelled=lambda: self.d.cancel_work(client, 7))
        self.s.client_id = 2
        self.answer(self.d.td.sent[-1])
        self.assertEqual(len(self.replies), 1)

    def test_current_background_completion_retains_original_account(self):
        done, cancelled = [], []
        with mock.patch.object(self.m.threading, "Thread") as thread:
            self.d.background(lambda: 42, lambda *args: done.append((self.d.session_for().id, args)),
                              cancelled=lambda *args: cancelled.append(args))
            thread.call_args.kwargs["target"]()
        self.d.run_tasks()
        self.assertEqual(done, [(self.s.id, (42, None))])
        self.assertEqual(cancelled, [])
        self.assertEqual(self.d.jobs, 0)


if __name__ == "__main__":
    unittest.main()
