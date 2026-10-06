#!/usr/bin/python3
"""Startup-parameter errors with fake TDLib/keyring and an isolated IPC consumer."""
import unittest
from daemon_test import Harness, auth_update


class ParameterContract(Harness):
    def start_parameters(self, conn):
        self.keyring.credentials = (12345, self.HASH)
        before = self.sent_count("setTdlibParameters")
        self.td_event(auth_update("authorizationStateWaitTdlibParameters"))
        return self.next_query("setTdlibParameters", before)

    def test_rejection_reaches_auth_event_and_snapshot_without_raw_secrets(self):
        conn = self.connect()
        observer = self.connect()
        query = self.start_parameters(conn)
        self.answer(query, {"@type": "error", "code": 400, "message": "synthetic-secret-parameter-details"})
        self.wait(lambda: query["@extra"] not in self.daemon.pending)
        snapshot = self.request(conn, 1, "hello")["result"]["auth"]
        self.assertEqual(snapshot["state"], "error")
        event = self.read(observer, lambda v: v.get("event") == "auth" and v["auth"]["state"] == "error")
        self.assertEqual(event["auth"], snapshot)
        self.assertIn("400", snapshot["reason"])
        self.assertNotIn("synthetic-secret", snapshot["reason"])
        self.assertLessEqual(len(snapshot["reason"]), 200)
        self.assertEqual(event["account"], "default")
        self.assertFalse(self.daemon.active_session.bridge_ready)

    def test_error_code_is_bounded_and_explicit_retry_still_works(self):
        conn = self.connect()
        query = self.start_parameters(conn)
        self.answer(query, {"@type": "error", "code": 10 ** 100, "message": "synthetic private detail"})
        self.wait(lambda: query["@extra"] not in self.daemon.pending)
        event = self.read(conn, lambda v: v.get("event") == "auth" and v["auth"]["state"] == "error")
        self.assertLessEqual(len(event["auth"]["reason"]), 200)
        self.assertNotIn(str(10 ** 100), event["auth"]["reason"])
        before = self.sent_count("setTdlibParameters")
        answer = self.request(conn, 3, "credentials.set", apiId=12345, apiHash=self.HASH)
        self.assertTrue(answer["ok"])
        retry = self.next_query("setTdlibParameters", before)
        self.answer(retry, {"@type": "ok"})
        self.wait(lambda: self.daemon.active_session.bridge_ready)
        self.assertEqual(self.daemon.active_session.auth, {"state": "starting"})

    def test_success_preserves_existing_bridge_contract(self):
        conn = self.connect()
        query = self.start_parameters(conn)
        self.answer(query, {"@type": "ok"})
        self.wait(lambda: query["@extra"] not in self.daemon.pending)
        self.assertTrue(self.daemon.active_session.bridge_ready)
        self.assertEqual(self.daemon.active_session.auth["state"], "starting")

    def test_generation_replacement_ignores_both_completions(self):
        conn = self.connect()
        session = self.daemon.active_session
        query = self.start_parameters(conn)
        success, failure = self.daemon.pending[query["@extra"]][2:4]
        self.assertTrue(callable(failure))
        session.client_id += 10
        session.auth = {"state": "starting"}
        success({"@type": "ok"})
        failure({"@type": "error", "code": 400})
        self.assertFalse(session.bridge_ready)
        self.assertEqual(session.auth, {"state": "starting"})

    def test_new_attempt_and_advanced_auth_ignore_old_failure(self):
        conn = self.connect()
        query = self.start_parameters(conn)
        success, failure = self.daemon.pending[query["@extra"]][2:4]
        self.assertTrue(callable(failure))
        self.daemon.send_parameters(self.daemon.active_session)
        success({"@type": "ok"})
        failure({"@type": "error", "code": 400})
        self.assertFalse(self.daemon.active_session.bridge_ready)
        latest = self.last_query("setTdlibParameters")
        newest_failure = self.daemon.pending[latest["@extra"]][3]
        self.daemon.active_session.auth = {"state": "phone"}
        newest_failure({"@type": "error", "code": 400})
        self.assertEqual(self.daemon.active_session.auth, {"state": "phone"})

    def test_other_account_rejection_does_not_change_active_auth(self):
        conn = self.connect()
        query = self.start_parameters(conn)
        added = self.request(conn, 2, "account.add", name="Synthetic second account")["result"]
        self.assertNotEqual(added["activeAccount"], "default")
        before = self.daemon.active_session.auth.copy()
        self.answer(query, {"@type": "error", "code": 400, "message": "synthetic failure"})
        self.wait(lambda: query["@extra"] not in self.daemon.pending)
        self.assertEqual(self.daemon.accounts["default"].auth["state"], "error")
        self.assertEqual(self.daemon.active_session.auth, before)


if __name__ == "__main__":
    unittest.main()
