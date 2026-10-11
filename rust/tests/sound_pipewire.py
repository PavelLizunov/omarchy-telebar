"""Private real PipeWire consumer. No real home, bus, network or audio devices."""
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

assert os.environ.get("TELEBAR_ISOLATED_PIPEWIRE") == "1"
with tempfile.TemporaryDirectory(prefix="telebar-pipewire-") as temporary:
    env = {"PATH": "/usr/bin", "HOME": temporary, "XDG_RUNTIME_DIR": temporary,
           "XDG_CONFIG_HOME": temporary, "TELEBAR_ISOLATED_PIPEWIRE": "1"}
    with (Path(temporary) / "server.log").open("wb") as log:
        server = subprocess.Popen(["/usr/bin/pipewire", "-c", "/pipewire-null.conf"],
                                  env=env, stdout=log, stderr=log, start_new_session=True)
        try:
            deadline = time.monotonic() + 5
            while not (Path(temporary) / "pipewire-0").is_socket() and time.monotonic() < deadline and server.poll() is None:
                time.sleep(0.02)
            if not (Path(temporary) / "pipewire-0").is_socket():
                raise RuntimeError("private PipeWire not ready: " + (Path(temporary) / "server.log").read_text())
            consumer = subprocess.Popen(["/test-sound", "actual_notification_sound_uses_only_the_isolated_null_sink", "--ignored", "--exact", "--nocapture"], env=env)
            try:
                deadline = time.monotonic() + 3
                ports = []
                while time.monotonic() < deadline and consumer.poll() is None:
                    output = subprocess.run(["/usr/bin/pw-link", "-o"], env=env, capture_output=True, timeout=1, check=True).stdout.decode()
                    dump = json.loads(subprocess.run(["/usr/bin/pw-dump"], env=env, capture_output=True, timeout=1, check=True).stdout)
                    nodes = [obj for obj in dump if obj.get("type") == "PipeWire:Interface:Node" and obj.get("info", {}).get("props", {}).get("application.name") == "pw-play"]
                    for node in nodes:
                        subprocess.run(["/usr/bin/pw-cli", "set-param", str(node["id"]), "PortConfig", "{ direction = Output mode = dsp format = { mediaType = audio mediaSubtype = raw format = F32P rate = 32000 channels = 1 position = [ MONO ] } }"], env=env, capture_output=True, timeout=1, check=True)
                    ports = [line.strip() for line in output.splitlines() if line.strip().startswith("pw-play:")]
                    if ports:
                        break
                    time.sleep(0.02)
                inputs = subprocess.run(["/usr/bin/pw-link", "-i"], env=env, capture_output=True, timeout=1, check=True).stdout.decode()
                sinks = [line.strip() for line in inputs.splitlines() if line.strip().startswith("telebar-null:")]
                if not ports or not sinks:
                    raise RuntimeError("private audio ports missing: " + output + inputs)
                for port in ports:
                    suffix = port.rsplit("_", 1)[-1]
                    sink = next((s for s in sinks if s.endswith("_" + suffix)), sinks[0])
                    subprocess.run(["/usr/bin/pw-link", port, sink], env=env, check=True, timeout=1)
                assert consumer.wait(timeout=7) == 0
            finally:
                if consumer.poll() is None:
                    consumer.kill()
                consumer.wait(timeout=2)
            print(json.dumps({"private_pipewire": True, "null_sink": True,
                              "actual_pw_play": True, "real_session_access": False}))
        finally:
            if server.poll() is None:
                os.killpg(server.pid, signal.SIGTERM)
            try:
                server.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(server.pid, signal.SIGKILL)
                server.wait(timeout=2)
