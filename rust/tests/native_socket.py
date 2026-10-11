#!/usr/bin/python3
"""Bounded real native socket acceptance, no parameter/profile/keyring access."""
import json
import os
import pathlib
import signal
import socket
import subprocess
import sys
import tempfile

binary, library = sys.argv[1:]
with tempfile.TemporaryDirectory(prefix="telebar-native-socket-") as name:
    root = pathlib.Path(name)
    process = subprocess.Popen([binary, "--inert-native", name, library], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        assert json.loads(process.stdout.readline())["production_profiles"] is False
        with socket.socket(socket.AF_UNIX) as peer:
            peer.settimeout(5)
            peer.connect(str(root / "migration.sock"))
            stream = peer.makefile("rwb", buffering=0)
            stream.write(b'{"id":1,"cmd":"fixture.version"}\n')
            events = []
            for _ in range(20):
                answer = json.loads(stream.readline())
                if answer.get("id") == 1:
                    assert answer["ok"] is True, answer
                    assert answer["result"]["@type"] == "optionValueString"
                    assert answer["result"]["value"] == "1.8.67"
                    break
                events.append(answer)
            else:
                raise AssertionError("no native terminal reply")
            stream.write(b'{"id":2,"cmd":"chat.history","args":{"chatId":42}}\n')
            for _ in range(20):
                answer = json.loads(stream.readline())
                if answer.get("id") == 2:
                    assert answer["error"] == "not signed in"
                    break
            else:
                raise AssertionError("no readiness terminal reply")
        process.send_signal(signal.SIGTERM)
        assert process.wait(timeout=7) == 0
        assert not (root / "migration.sock").exists()
        print(json.dumps({"native_socket": True, "native_query": "execute:getOption(version)", "ready_gate": True, "shutdown": True, "parameters_sent": False, "profile_opened": False}))
    finally:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=5)
