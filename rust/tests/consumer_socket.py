#!/usr/bin/python3
"""Compatible hello on the actual Rust socket in a disposable pre-parameter session.

Must run inside run-consumer-socket.sh; no session bus, network or working home.
Python is a test driver only. No native mutation or database parameters are sent.
"""
import json
import os
import pathlib
import socket
import subprocess
import sys
import tempfile

assert os.environ.get("TELEBAR_ISOLATED_CONSUMER") == "1"
binary, library = sys.argv[1:]
no_library = library == "--no-library"
with tempfile.TemporaryDirectory(prefix="telebar-consumer-") as name:
    root = pathlib.Path(name)
    catalog = root / ".local/share/omagram/accounts.json"
    catalog.parent.mkdir(parents=True, mode=0o700)
    catalog.write_text(json.dumps({"active": "other", "accounts": [{"id": "other", "name": "First", "phone": "fixture", "meId": 7}, {"id": "default", "name": "Second"}]}))
    argv = [binary, "--inert-no-library", name] if no_library else [binary, "--inert-consumer", name, library]
    process = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        assert json.loads(process.stdout.readline())["production_profiles"] is False
        with socket.socket(socket.AF_UNIX) as sock:
            sock.settimeout(5)
            sock.connect(str(root / "migration.sock"))
            stream = sock.makefile("rwb", buffering=0)
            captured = []
            def request(rid, command, args=None):
                stream.write((json.dumps({"id": rid, "cmd": command, "args": args or {}}) + "\n").encode())
                for _ in range(50):
                    response = json.loads(stream.readline())
                    captured.append(response)
                    if response.get("id") == rid:
                        return response
                raise AssertionError("missing terminal response")
            hello = request(1, "hello")
            assert hello["ok"]
            view = hello["result"]
            assert view["activeAccount"] == "other" and view["meId"] == 7
            assert [row["id"] for row in view["accounts"]] == ["other", "default"]
            assert view["accounts"][0]["phone"] == "fixture"
            assert "allChats" not in view and "stories" not in view
            required = {"version", "auth", "meId", "chats", "open", "folders", "mainPosition", "settings", "globalStatus", "calls", "connection", "accounts", "activeAccount", "accountRevision", "recordedSend"}
            assert set(view) == required
            if no_library:
                assert view["auth"]["state"] == "noLibrary"
                for rid, cmd, args in [(20, "auth.phone", {"phone":"+12025550123"}), (21, "proxies.list", {}), (22,"account.logout",{})]:
                    assert request(rid, cmd, args)["error"] == "Telegram is not available"
                assert request(23,"hello")["result"]["auth"]["state"] == "noLibrary"
            window = request(2, "hello", {"window": True})["result"]
            assert set(window) == required | {"allChats", "stories"}
            assert window["allChats"] == [] and window["stories"] == []
            switched = request(3, "account.switch", {"id": "default"})
            assert switched["result"]["activeAccount"] == "default"
            assert "allChats" in switched["result"]
            assert request(4, "chat.history", {"chatId": 42})["error"] == "not signed in"
            changed = request(6, "settings.set", {"settings": {"globalShortcuts": {"global.panel": "SUPER + P"}}})
            assert changed["ok"] and changed["result"]["globalStatus"]["global.panel"] == "unavailable"
            refreshed = request(7, "hello")["result"]
            assert refreshed["globalStatus"]["global.panel"] == "unavailable"
            assert refreshed["settings"]["globalShortcuts"]["global.panel"] == "SUPER + P"
            assert any(frame.get("event") == "settings" and frame["globalStatus"]["global.panel"] == "unavailable" for frame in captured)
            # One read batch admits concurrent commands before helper completion.
            # Global work and later settings must remain serial, reply then event.
            pipelined = [(8, "settings.set", {"settings": {"globalShortcuts": {"global.openWindow": "CTRL + O"}}}),
                         (9, "settings.quiet", {"quiet": True}),
                         (10, "shortcuts.apply", {})]
            stream.write(b"".join((json.dumps({"id": rid, "cmd": cmd, "args": args}) + "\n").encode()
                                  for rid, cmd, args in pipelined))
            ordered = []
            for _ in range(50):
                frame = json.loads(stream.readline())
                captured.append(frame)
                if frame.get("id") in (8, 9, 10) or frame.get("event") == "settings":
                    ordered.append(frame)
                if frame.get("id") == 10:
                    break
            replies = [frame for frame in ordered if "id" in frame]
            assert [frame["id"] for frame in replies] == [8, 9, 10]
            assert all(frame["ok"] for frame in replies)
            assert replies[0]["result"]["settings"]["quiet"] is False
            assert replies[1]["result"]["settings"]["quiet"] is True
            assert replies[2]["result"] == replies[1]["result"]
            assert replies[0]["result"]["globalStatus"]["global.openWindow"] == "unavailable"
            assert next(i for i, frame in enumerate(ordered) if frame.get("id") == 8) < next(i for i, frame in enumerate(ordered) if frame.get("event") == "settings" and frame["settings"]["globalShortcuts"].get("global.openWindow") == "CTRL + O")
            # Use the compatible shared consumer, not the older fixture.snapshot
            # route. Redaction, rate limiting and peer identity must hold here.
            report = request(24, "diagnostics.ui", {"window": True, "pending": 3,
                "password": "PRIVATE-CONTENT", "files": "PRIVATE-CONTENT",
                "width": -1, "height": 1e100})
            assert report["ok"]
            limited = request(25, "diagnostics.ui", {"pending": 99})
            assert limited["error"] == "diagnostic reports are rate limited"
            status = request(26, "diagnostics.status")
            assert status["ok"]
            diagnostic = status["result"]
            assert diagnostic["clients"] == 1 and diagnostic["account_count"] == 2
            assert diagnostic["pending_ipc"] == 0 and diagnostic["process"]["pid"] == process.pid
            assert diagnostic["migration_complete"] is False
            assert any(row["metrics"] == {"pending": 3} and row["surface"] == "window"
                       for row in diagnostic["ui"])
            assert "PRIVATE-CONTENT" not in json.dumps(diagnostic) and "password" not in json.dumps(diagnostic)
            assert request(27, "hello")["ok"]
            assert request(5, "app.quit")["ok"]
            assert process.wait(timeout=7) == 0
        assert not (root / "migration.sock").exists()
        assert not list(root.glob(".local/share/omagram/**/database"))
        assert not list(root.glob(".local/share/omagram/**/db.sqlite"))
        records = (root / ".local/state/omagram/diagnostics/trace.jsonl").read_text()
        assert "PRIVATE-CONTENT" not in records and "password" not in records
        assert any(json.loads(line)["event"] == "ui_sample" for line in records.splitlines())
        print(json.dumps({"compatible_hello": True, "window_shape": True, "catalog_order": True, "switch": True, "shutdown": True, "diagnostics": True, "diagnostic_redaction": True, "parameters_sent": False, "profile_opened": False, "no_library": no_library, "frames": len(captured)}))
    finally:
        if process.poll() is None:
            process.terminate()
        process.wait(timeout=7)
