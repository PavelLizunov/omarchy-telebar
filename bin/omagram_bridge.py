"""Owner for the bundled proxy subprocess; no network/crypto work in the daemon loop."""
import json
import os
import pathlib
import secrets
import selectors
import signal
import subprocess
import threading
import time

import plugin_safety as safe
import omagram_settings as prefs
import omagram_td as td


class Bridge:
    def __init__(self, diagnostic_log):
        self.log = diagnostic_log
        self.proc = None
        self.port = 0
        self.secret = ""
        self.metrics = {}
        self.lock = threading.Lock()
        self.metadata = safe.read_json(prefs.CONFIG / "bridge.json", 8192, default={},
                                       max_depth=4, max_items=80, max_string=64) or {}
        if not isinstance(self.metadata, dict):
            self.metadata = {}
        self.metadata = {aid: entry for aid, entry in self.metadata.items()
                         if td.valid_account_id(aid) and isinstance(entry, dict)
                         and all(isinstance(entry.get(k, 0), int) and not isinstance(entry.get(k, 0), bool)
                                 and 0 <= entry.get(k, 0) < 2 ** 31 for k in ("proxy_id", "previous_id"))}
        self.metadata = dict(list(self.metadata.items())[:10])

    def save(self):
        safe.write_json(prefs.CONFIG / "bridge.json", self.metadata)

    def status(self, account_id):
        running = self.proc is not None and self.proc.poll() is None
        return {"running": running, "enabled": account_id in self.metadata, "port": self.port if running else 0,
                "metrics": dict(self.metrics), "pid": self.proc.pid if running else 0}

    def start(self):
        """Called in bounded background work. Read readiness before configuring TDLib."""
        with self.lock:
            if self.proc is not None and self.proc.poll() is None:
                return self.port
            if self.proc is not None:
                self._stop_locked()
            self.secret = secrets.token_hex(16)
            runner = pathlib.Path(__file__).with_name("omagram-proxy")
            env = {"PATH": "/usr/bin", "HOME": safe.home_dir(), "LANG": "C.UTF-8"}
            proc = subprocess.Popen(["/usr/bin/python3", "-I", str(runner)], stdin=subprocess.PIPE,
                                    stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env,
                                    start_new_session=True, close_fds=True)
            self.proc = proc
            try:
                proc.stdin.write((json.dumps({"secret": self.secret}) + "\n").encode())
                proc.stdin.flush()  # Keep open: EOF tells the child its owner is gone.
                with selectors.DefaultSelector() as selector:
                    selector.register(proc.stdout, selectors.EVENT_READ)
                    deadline, line = time.monotonic() + 5, bytearray()
                    while not line.endswith(b"\n"):
                        remaining = deadline - time.monotonic()
                        if remaining <= 0 or not selector.select(remaining):
                            raise OSError("proxy startup timed out")
                        byte = os.read(proc.stdout.fileno(), 1)
                        if not byte or len(line) >= 4096:
                            raise OSError("invalid proxy readiness")
                        line.extend(byte)
                ready = safe.loads(bytes(line), max_depth=2, max_items=8, max_string=80)
                if len(line) > 4096 or ready.get("event") != "ready" or not 1 <= ready.get("port", 0) <= 65535:
                    raise OSError("proxy did not become ready")
                self.port = ready["port"]
                threading.Thread(target=self._read_metrics, args=(proc,), name="omagram-proxy-metrics", daemon=True).start()
                self.log.emit("proxy_started", port=self.port, child_pid=proc.pid)
                return self.port
            except Exception:
                self._stop_locked()
                raise

    def _read_metrics(self, proc):
        try:
            while True:
                line = proc.stdout.readline(4097)
                if not line:
                    return
                if len(line) > 4096:
                    proc.terminate()
                    return
                data = safe.loads(line, max_depth=2, max_items=16, max_string=80)
                if data.get("event") == "stats":
                    self.metrics = {k: v for k, v in data.items() if isinstance(v, int) and 0 <= v <= 10 ** 15}
                    self.log.emit("proxy_stats", **self.metrics)
                elif data.get("event") == "connection_error":
                    self.log.emit("proxy_connection_error", exception_type=str(data.get("exception_type", "Error"))[:80])
        except (OSError, ValueError, safe.UnsafeError):
            self.log.emit("proxy_metrics_error")
        finally:
            proc.stdout.close()

    def _stop_locked(self):
        proc, self.proc = self.proc, None
        if proc is not None:
            if proc.stdin and not proc.stdin.closed:
                proc.stdin.close()
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    os.killpg(proc.pid, signal.SIGKILL)
                    proc.wait(timeout=2)
        self.port = 0
        self.secret = ""

    def stop(self):
        with self.lock:
            self._stop_locked()
        self.log.emit("proxy_stopped")
