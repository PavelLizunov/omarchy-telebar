#!/usr/bin/python3
"""Finite actual local/native owner probe. Explicit disposable home, no parameters.

The fixture-only snapshot command translates into Application's hello internally;
compatible production hello remains rejected to avoid claiming complete parity.
"""
import json
import pathlib
import socket
import subprocess
import sys
import tempfile

binary, library = sys.argv[1:]
with tempfile.TemporaryDirectory(prefix="telebar-application-socket-") as name:
    root = pathlib.Path(name)
    catalog = root / ".local/share/omagram/accounts.json"
    catalog.parent.mkdir(parents=True, mode=0o700)
    catalog.write_text(json.dumps({"active": "other", "accounts": [{"id": "other", "name": "Other first", "meId": 7}, {"id": "default", "name": "Default second"}]}))
    process = subprocess.Popen([binary, "--inert-application", name, library], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        assert json.loads(process.stdout.readline())["production_profiles"] is False
        with socket.socket(socket.AF_UNIX) as peer, socket.socket(socket.AF_UNIX) as observer:
            for sock in [peer, observer]:
                sock.settimeout(5)
                sock.connect(str(root / "migration.sock"))
            stream = peer.makefile("rwb", buffering=0)
            watch = observer.makefile("rwb", buffering=0)
            def request(rid, command, args=None):
                stream.write((json.dumps({"id": rid, "cmd": command, "args": args or {}}) + "\n").encode())
                events = []
                for _ in range(40):
                    answer = json.loads(stream.readline())
                    if answer.get("id") == rid:
                        return answer, events
                    events.append(answer)
                raise AssertionError("missing terminal reply")
            answer, _ = request(1, "hello")
            assert answer["error"] == "migration command is not implemented"
            answer, _ = request(2, "fixture.snapshot", {"window": True})
            assert answer["ok"] and answer["result"]["allChats"] == []
            assert [a["id"] for a in answer["result"]["accounts"]] == ["other", "default"]
            assert answer["result"]["accounts"][0]["name"] == "Other first"
            assert answer["result"]["meId"] == 7
            answer, events = request(3, "account.switch", {"id": "other"})
            assert answer["result"]["activeAccount"] == "other"
            assert events[-1]["event"] == "accountSwitched"
            for _ in range(40):
                event = json.loads(watch.readline())
                if event.get("event") == "accountSwitched":
                    assert event["activeAccount"] == "other"
                    break
            else:
                raise AssertionError("observer lost account broadcast")
            assert json.loads(catalog.read_text())["active"] == "other"
            answer, _ = request(4, "settings.playback", {"rate": 1.5})
            assert answer["ok"] and answer["result"]["settings"]["playbackRate"] == 1.5
            assert json.loads((root / ".config/omagram/settings.json").read_text())["playbackRate"] == 1.5
            answer, _ = request(5, "chat.history", {"chatId": 42})
            assert answer["error"] == "not signed in"
            catalog.unlink()
            catalog.symlink_to("foreign")
            answer, events = request(6, "account.switch", {"id": "default"})
            assert answer["ok"] is False and not any(e.get("event") == "accountSwitched" for e in events)
            answer, _ = request(7, "fixture.snapshot")
            assert answer["result"]["activeAccount"] == "other" and answer["result"]["accountRevision"] == 1
            catalog.unlink()
            answer, events = request(8, "account.add", {"name": "Synthetic new account"})
            added = answer["result"]["activeAccount"]
            assert added.startswith("acc_") and events[-1]["event"] == "accountAdded"
            assert json.loads(catalog.read_text())["active"] == added
            answer, _ = request(9, "fixture.snapshot")
            assert len(answer["result"]["accounts"]) == 3
            answer, _ = request(11, "diagnostics.ui", {"window": True, "pending": 3, "password": "PRIVATE-CONTENT"})
            assert answer["ok"]
            answer, _ = request(12, "diagnostics.ui", {"pending": 99})
            assert answer["error"] == "diagnostic reports are rate limited"
            answer, _ = request(13, "diagnostics.status")
            assert answer["ok"] and answer["result"]["clients"] == 2
            assert answer["result"]["account_count"] == 3
            assert answer["result"]["pending_ipc"] == 0
            assert answer["result"]["process"]["pid"] == process.pid
            assert any(row["metrics"] == {"pending": 3} for row in answer["result"]["ui"])
            answer, events = request(10, "app.quit")
            assert answer["ok"] and answer["result"] == {}
            assert events[-1]["event"] == "quit"
            for _ in range(40):
                if json.loads(watch.readline()).get("event") == "quit":
                    break
            else:
                raise AssertionError("observer lost graceful quit broadcast")
            assert process.wait(timeout=7) == 0
        assert not (root / "migration.sock").exists()
        records = (root / ".local/state/omagram/diagnostics/trace.jsonl").read_text()
        assert "PRIVATE-CONTENT" not in records and "password" not in records
        assert any(json.loads(line)["event"] == "ui_sample" for line in records.splitlines())
        print(json.dumps({"application_socket": True, "native_clients": 3, "account_add": True, "persisted_switch": True, "observer_broadcast": True, "failed_save_preserved": True, "settings": True, "diagnostics": True, "graceful_quit": True, "shutdown": True, "parameters_sent": False, "profile_opened": False, "production_hello": False}))
    finally:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=5)
