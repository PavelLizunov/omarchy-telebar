#!/usr/bin/python3
"""Optional QtTest consumer; requires dynamic Quickshell QML plugins.

Stock Arch Quickshell supplies only statically linked modules. In that case this
runner reports a missing import, not PASS. ClientConsumer.qml through the reviewed
qml-preview static adapter is the actual socket consumer acceptance path.
"""
import json
import os
import pathlib
import subprocess
import sys
import tempfile
assert os.environ.get("TELEBAR_ISOLATED_CONSUMER") == "1"
binary, library = sys.argv[1:]
with tempfile.TemporaryDirectory(prefix="telebar-qml-") as name:
    server = subprocess.Popen([binary, "--inert-consumer", name, library], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        assert json.loads(server.stdout.readline())["production_profiles"] is False
        env = dict(os.environ, TELEBAR_FIXTURE_SOCKET=name+"/migration.sock", QT_QPA_PLATFORM="offscreen", QT_QUICK_BACKEND="software")
        test = subprocess.run(["/usr/lib/qt6/bin/qmltestrunner", "-input", "/workspace/rust/tests/tst_qml_socket.qml"], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=15)
        sys.stdout.buffer.write(test.stdout)
        assert test.returncode == 0
        assert server.wait(timeout=7) == 0
        assert not pathlib.Path(name, "migration.sock").exists()
        assert not list(pathlib.Path(name).glob(".local/share/omagram/**/database"))
        print(json.dumps({"actual_qml_client": True, "socket_shape_order": True, "profile_opened": False}))
    finally:
        if server.poll() is None: server.terminate()
        server.wait(timeout=7)
