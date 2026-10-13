#!/usr/bin/python3
"""Bounded outer negative-control consumer; never receives user credentials."""
import json
import pathlib
import selectors
import shutil
import socket
import subprocess
import sys
import tempfile

mode = sys.argv[4] if len(sys.argv) == 5 else "--keyring-fixture"
assert mode in ("--keyring-fixture", "--send-fixture", "--voice-fixture", "--video-fixture")
binary, library, sandbox = (pathlib.Path(p).resolve(strict=True) for p in sys.argv[1:4])
voice = mode == "--voice-fixture"
sending = mode in ("--send-fixture", "--voice-fixture", "--video-fixture")
root = pathlib.Path(tempfile.mkdtemp(prefix="telebar-private-send-" if sending else "telebar-private-keyring-"))
exposed = root / "exposed"
exposed.mkdir(mode=0o700)
shutil.copyfile(binary, exposed / "test-keyring")
(exposed / "test-keyring").chmod(0o700)
(exposed / "lib").mkdir(mode=0o700)
shutil.copyfile(library, exposed / "lib/libtdjson.so")
(exposed / "lib/libtdjson.so").chmod(0o600)
if voice:
    for config in ("pipewire-voice.conf", "pulse-private.conf"):
        shutil.copyfile(pathlib.Path(__file__).parent / config, exposed / config)
        (exposed / config).chmod(0o600)
sentinel = root / "external-sentinel"
sentinel.write_bytes(b"inert non-secret external file")
host = socket.socket(socket.AF_UNIX)
host.bind(str(root / "external.sock"))
host.listen(1)
try:
    # Baseline controls prove that failures below are actual denial rather than
    # nonexistent inputs. Close the inert accepted connection without any data.
    with socket.socket(socket.AF_UNIX) as control:
        control.connect(str(root / "external.sock"))
        accepted, _ = host.accept()
        accepted.close()
    assert sentinel.read_bytes() == b"inert non-secret external file"
    with open("/dev/full", "rb", buffering=0):
        pass  # Existing inert device baseline; no device bytes or writes.
    env = {"PATH": "/usr/bin", "TELEBAR_PRIVATE_KEYRING": "1",
           "TELEBAR_TEST_TDLIB": str(exposed / "lib/libtdjson.so"),
           "TELEBAR_PRIVATE_ROOT": str(exposed), "TELEBAR_DENIED_FILE": str(sentinel),
           "TELEBAR_DENIED_SOCKET": str(root / "external.sock")}
    test = "actual_private_keyring_and_driver_startup_preserve_account_key_identity"
    if sending:
        env["TELEBAR_PRIVATE_SEND"] = "1"
        env["TELEBAR_TEST_INERT_ABI"] = env["TELEBAR_TEST_TDLIB"]
        test = "actual_send_driver_acceptance_keeps_playback_inputs_and_originals"
    if voice:
        env["TELEBAR_PRIVATE_VOICE"] = "1"
        test = "actual_voice_driver_uses_only_the_private_synthetic_pulse_source"
    if mode == "--video-fixture":
        env["TELEBAR_PRIVATE_VIDEO"] = "1"
        test = "native_driver::tests::actual_video_request_routes_retain_retry_and_discard_owned_inputs"
    result = subprocess.run([str(sandbox), str(exposed), str(exposed / "test-keyring"),
                             test, "--ignored", "--exact", "--nocapture"], env=env,
                            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=45, check=False)
    sys.stdout.buffer.write(result.stdout)
    sys.stderr.buffer.write(result.stderr)
    with selectors.DefaultSelector() as poll:
        poll.register(host, selectors.EVENT_READ)
        assert not poll.select(0), "sandbox connected to the external Unix fixture"
    assert sentinel.read_bytes() == b"inert non-secret external file"
    assert result.returncode == 0, f"exact sandbox consumer exit {result.returncode}"
    print(json.dumps({"externalFileBaseline": True, "externalUnixBaseline": True,
                      "externalDenied": True, "deniedDeviceBaseline": True,
                      "privateRoot": True, "bounded": True, "sendFixture": sending,
                      "voiceFixture": voice}))
finally:
    host.close()
    shutil.rmtree(root)
