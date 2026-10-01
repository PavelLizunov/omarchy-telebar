"""Local log safety and hang reporting, with synthetic data and private temporary files."""
import json
import os
import pathlib
import shutil
import sys
import tempfile
import time
import threading
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent / "bin"))
import omagram_diagnostics as diag
import plugin_safety as safe


class Diagnostics(unittest.TestCase):
    def setUp(self):
        self.root = pathlib.Path(tempfile.mkdtemp(prefix="omagram-diagnostics-", dir=safe.runtime_dir()))
        self.addCleanup(shutil.rmtree, self.root, True)

    def test_writer_rotates_and_keeps_private_files(self):
        log = diag.Diagnostics(self.root / "logs", max_bytes=1200, backups=2)
        log.start(lambda: {"synthetic": True})
        for number in range(100):
            log.emit("synthetic", number=number)
        log.close()
        files = list((self.root / "logs").iterdir())
        self.assertLessEqual(len(files), 3)
        self.assertGreaterEqual(len(files), 2)
        self.assertEqual(log.write_errors, 0)
        for file in files:
            self.assertEqual(file.stat().st_mode & 0o777, 0o600)
            self.assertLessEqual(file.stat().st_size, 1200)
            for line in file.read_text().splitlines():
                self.assertIsInstance(json.loads(line), dict)

    def test_caller_strings_and_exception_secrets_are_not_logged(self):
        secret = "PASSWORD-SECRET-API-HASH-TEXT"
        fields = diag.numeric_fields({"pending": 7, "width": 900, "height": float("nan"),
                                      "rx_bytes": float("inf"), "frame_count": -1,
                                      "oldest_ms": secret, "password": secret, "nested": {"secret": secret}})
        self.assertEqual(fields, {"pending": 7, "width": 900})
        log = diag.Diagnostics(self.root / "logs")
        log.start(lambda: {})
        log.failure("test", ValueError(secret))
        log.last_progress = time.monotonic() - 10
        self.assertTrue(log.check_stall())
        log.emit("ui_sample", metrics=fields)
        log.close()
        text = "".join(file.read_text() for file in (self.root / "logs").iterdir())
        self.assertNotIn(secret, text)
        self.assertNotIn(str(self.root), text)
        self.assertIn("main_loop_stall", text)
        self.assertIn("exception_type", text)

    def test_symlink_log_is_refused_without_touching_target(self):
        directory = self.root / "logs"
        directory.mkdir(mode=0o700)
        target = self.root / "target"
        target.write_text("untouched")
        (directory / diag.LOG_NAME).symlink_to(target)
        log = diag.Diagnostics(directory)
        log.start(lambda: {})
        log.close()
        self.assertEqual(target.read_text(), "untouched")
        self.assertEqual(log.write_errors, 1)

    def test_queue_is_bounded_even_when_no_writer_runs(self):
        log = diag.Diagnostics(self.root)
        for _ in range(diag.QUEUE_MAX + 100):
            log.emit("synthetic")
        self.assertEqual(log.records.qsize(), diag.QUEUE_MAX)
        self.assertEqual(log.dropped, 100)

    def test_watchdog_reports_blocked_owner_from_independent_thread(self):
        log = diag.Diagnostics(self.root / "logs")
        log.start(lambda: {"synthetic": True})
        log.last_progress = time.monotonic() - 10
        # The owner does not call check_stall; the independent watchdog must catch it.
        time.sleep(1.2)
        log.close()
        records = [json.loads(line) for file in (self.root / "logs").iterdir()
                   for line in file.read_text().splitlines()]
        stalls = [r for r in records if r["event"] == "main_loop_stall"]
        self.assertTrue(stalls)
        self.assertEqual(stalls[0]["main_thread"], threading.get_ident())

    def test_numeric_process_metrics_have_no_process_arguments(self):
        fields = diag.process_metrics(os.getpid())
        self.assertGreater(fields["rss_kb"], 0)
        self.assertIn("user_ms", fields)
        self.assertNotIn("cmdline", fields)


if __name__ == "__main__":
    unittest.main()
