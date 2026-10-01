"""Bounded local hang diagnostics. No message text, request arguments or frame locals.

The event loop only queues records; disk writes and watchdog sampling run separately.
Files are private, rotated, and opened relative to an owner-checked directory descriptor.
"""
import collections
import hashlib
import json
import math
import os
import pathlib
import queue
import stat
import sys
import threading
import time

import plugin_safety as safe

DIRECTORY = pathlib.Path(safe.home_dir()) / ".local/state/omagram/diagnostics"
LOG_NAME = "trace.jsonl"
LOG_BYTES = 4 * 1024 * 1024
LOG_BACKUPS = 4
QUEUE_MAX = 2048
RECORD_MAX = 64 * 1024
SAMPLE_SECONDS = 5
STALL_SECONDS = 2.5
STACK_INTERVAL = 15
UI_FIELDS = {
    "timer_max_ms", "timer_late_count", "frame_count", "frame_max_ms", "frame_slow_count",
    "pending", "pending_max", "oldest_ms", "rx_lines", "rx_bytes", "tx_requests",
    "response_max_ms", "response_slow_count", "handler_max_ms", "handler_slow_count",
    "parse_errors", "connected", "focused", "visible", "chats", "messages", "files",
    "stories", "width", "height", "chat_list_width", "generation",
}


def numeric_fields(value, allowed=UI_FIELDS):
    """Accept only bounded finite numbers/bools; never persist caller strings or nested data."""
    if not isinstance(value, dict):
        return {}
    return {key: number for key, number in value.items()
            if key in allowed and isinstance(number, (int, float))
            and math.isfinite(number) and 0 <= number <= 10 ** 12}


def process_metrics(pid):
    """Read numeric process counters only, never argv, environment, open files or memory."""
    try:
        root = pathlib.Path("/proc") / str(int(pid))
        text = (root / "status").read_text()[:65536]
        fields = dict(line.split(":", 1) for line in text.splitlines() if ":" in line)
        result = {"pid": int(pid)}
        for source, key in (("VmRSS", "rss_kb"), ("VmSwap", "swap_kb"), ("Threads", "threads")):
            result[key] = int(fields.get(source, "0").split()[0])
        raw = (root / "stat").read_text()[:8192]
        parts = raw[raw.rfind(")") + 2:].split()
        ticks = os.sysconf("SC_CLK_TCK")
        result.update(minor_faults=int(parts[7]), major_faults=int(parts[9]),
                      user_ms=round(int(parts[11]) * 1000 / ticks),
                      system_ms=round(int(parts[12]) * 1000 / ticks))
        try:
            io = dict(line.split(":", 1) for line in (root / "io").read_text()[:8192].splitlines())
            result.update(read_bytes=int(io.get("read_bytes", 0)), write_bytes=int(io.get("write_bytes", 0)))
        except (OSError, ValueError):
            pass
        return result
    except (OSError, ValueError, IndexError):
        return {"pid": int(pid), "unavailable": True}


def memory_pressure():
    try:
        lines = pathlib.Path("/proc/pressure/memory").read_text()[:4096].splitlines()
        return {line.split()[0] + "_avg10": float(line.split()[1].split("=")[1]) for line in lines}
    except (OSError, ValueError, IndexError):
        return {}


def stack_locations():
    """Location-only stacks: no source lines, exception messages, locals or absolute paths."""
    names = {t.ident: t.name for t in threading.enumerate()}
    stacks = []
    for ident, frame in list(sys._current_frames().items())[:32]:
        locations = []
        for _ in range(32):
            if frame is None:
                break
            locations.append({"file": pathlib.Path(frame.f_code.co_filename).name,
                              "function": frame.f_code.co_name, "line": frame.f_lineno})
            frame = frame.f_back
        stacks.append({"thread": names.get(ident, "thread"), "ident": ident, "frames": locations})
    return stacks


class Diagnostics:
    def __init__(self, directory=None, *, enabled=True, max_bytes=LOG_BYTES, backups=LOG_BACKUPS):
        self.directory = pathlib.Path(directory or DIRECTORY)
        self.enabled = enabled
        self.max_bytes, self.backups = max_bytes, backups
        self.records = queue.Queue(maxsize=QUEUE_MAX)
        self.stop_event = threading.Event()
        self.writer = self.watcher = None
        self.last_progress = time.monotonic()
        self.phase = "startup"
        self.main_ident = threading.get_ident()
        self.dropped = 0
        self.write_errors = 0
        self.written = 0
        self.run_id = f"{os.getpid()}-{time.time_ns()}"
        self.counters = collections.Counter()

    def emit(self, event, **fields):
        if not self.enabled:
            return
        record = {"time": time.time(), "mono": time.monotonic(), "pid": os.getpid(),
                  "run": self.run_id, "event": event, **fields}
        try:
            self.records.put_nowait(record)
        except queue.Full:
            self.dropped += 1

    def failure(self, phase, error):
        # Exception messages can include Telegram text, login data or a filename.
        self.emit("exception", phase=phase, exception_type=type(error).__name__, stacks=stack_locations())

    def start(self, snapshot):
        if not self.enabled:
            return
        self.main_ident = threading.get_ident()
        self.progress("startup")
        self.writer = threading.Thread(target=self._write_loop, name="omagram-log-writer", daemon=True)
        self.watcher = threading.Thread(target=self._watch_loop, args=(snapshot,), name="omagram-watchdog", daemon=True)
        self.writer.start()
        self.watcher.start()
        digest = hashlib.sha256()
        for name in ("omagramd", "omagram_td.py", "omagram_diagnostics.py", "../app/Main.qml",
                     "../app/OmagramClient.qml", "../app/StoryViewer.qml", "../app/MediaView.qml"):
            try:
                digest.update((pathlib.Path(__file__).parent / name).read_bytes())
            except OSError:
                pass
        self.emit("start", code_sha256=digest.hexdigest(), python=sys.version.split()[0],
                  max_log_bytes=self.max_bytes * (self.backups + 1), queue_limit=QUEUE_MAX)

    def progress(self, phase="idle"):
        self.last_progress = time.monotonic()
        self.phase = phase

    def check_stall(self, now=None):
        now = time.monotonic() if now is None else now
        lag = now - self.last_progress
        if lag >= STALL_SECONDS:
            self.emit("main_loop_stall", lag_ms=round(lag * 1000), phase=self.phase,
                      main_thread=self.main_ident, stacks=stack_locations())
            return True
        return False

    def _watch_loop(self, snapshot):
        last_sample = last_stack = 0.0
        while not self.stop_event.wait(1):
            now = time.monotonic()
            if now - self.last_progress >= STALL_SECONDS and now - last_stack >= STACK_INTERVAL:
                self.check_stall(now)
                last_stack = now
            if now - last_sample >= SAMPLE_SECONDS:
                try:
                    values = snapshot()
                    values.update(memory_pressure=memory_pressure(), logger_queue=self.records.qsize(),
                                  logger_dropped=self.dropped, logger_write_errors=self.write_errors)
                    self.emit("sample", **values)
                except Exception as error:
                    self.emit("sample_error", exception_type=type(error).__name__)
                last_sample = now

    def _open_log(self, directory_fd):
        fd = os.open(LOG_NAME, os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_NOFOLLOW
                     | os.O_NONBLOCK | os.O_CLOEXEC, 0o600, dir_fd=directory_fd)
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid() or st.st_nlink != 1:
            os.close(fd)
            raise safe.UnsafeError("unsafe diagnostic log")
        os.fchmod(fd, 0o600)
        return fd

    def _write_loop(self):
        directory_fd = fd = None
        try:
            anchor, parts = safe._split_dir(self.directory)
            directory_fd = safe._open_dir(anchor, parts, create=True, mode=0o700)
            os.fchmod(directory_fd, 0o700)
            fd = self._open_log(directory_fd)
            size = os.fstat(fd).st_size
            while not self.stop_event.is_set() or not self.records.empty():
                try:
                    record = self.records.get(timeout=0.2)
                except queue.Empty:
                    continue
                data = (json.dumps(record, ensure_ascii=True, separators=(",", ":"), allow_nan=False) + "\n").encode()
                if len(data) > RECORD_MAX:
                    self.dropped += 1
                    continue
                if size + len(data) > self.max_bytes:
                    os.close(fd)
                    fd = None
                    for i in range(self.backups, 0, -1):
                        source = LOG_NAME if i == 1 else f"{LOG_NAME}.{i - 1}"
                        try:
                            os.replace(source, f"{LOG_NAME}.{i}", src_dir_fd=directory_fd, dst_dir_fd=directory_fd)
                        except FileNotFoundError:
                            pass
                    fd = self._open_log(directory_fd)
                    size = 0
                view = memoryview(data)
                while view:
                    written = os.write(fd, view)
                    if written <= 0:
                        raise OSError("diagnostic write made no progress")
                    view = view[written:]
                size += len(data)
                self.written += 1
        except (OSError, safe.UnsafeError, ValueError):
            self.write_errors += 1
            # Fixed message only. Logging failures must not interrupt the Telegram session.
            print("omagram: diagnostic log unavailable", file=sys.stderr)
        finally:
            if fd is not None:
                os.close(fd)
            if directory_fd is not None:
                os.close(directory_fd)

    def close(self):
        if not self.enabled:
            return
        self.emit("stop")
        self.stop_event.set()
        for thread in (self.watcher, self.writer):
            if thread and thread is not threading.current_thread():
                thread.join(timeout=2)
