"""Development-only real selection consumer inside the reviewed private namespace."""
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

assert os.environ.get("TELEBAR_ISOLATED_WAYLAND") == "1"
assert Path("/test-runtime").is_file()
assert Path("/native/lib/libtdjson.so").is_file()


def stop_owned(process):
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=2)


with tempfile.TemporaryDirectory(prefix="telebar-wayland-") as temporary:
    root = Path(temporary)
    os.chmod(root, 0o700)
    environment = {
        "PATH": "/usr/bin",
        "HOME": temporary,
        "XDG_RUNTIME_DIR": temporary,
        "XDG_CONFIG_HOME": temporary,
        "WLR_BACKENDS": "headless",
        "WLR_RENDERER": "pixman",
        "WLR_HEADLESS_OUTPUTS": "1",
        "LD_LIBRARY_PATH": "/fixture/usr/lib",
        "XKB_CONFIG_ROOT": "/usr/share/X11/xkb",
    }
    with (root / "labwc.log").open("wb") as log:
        server = subprocess.Popen(
            ["/fixture/usr/bin/labwc", "-C", temporary],
            env=environment, stdout=log, stderr=log, start_new_session=True,
        )
        try:
            deadline = time.monotonic() + 5
            sockets = []
            while time.monotonic() < deadline and server.poll() is None:
                sockets = [p for p in root.glob("wayland-*") if p.is_socket()]
                if sockets:
                    break
                time.sleep(0.02)
            if len(sockets) != 1:
                raise RuntimeError("headless compositor not ready: " + (root / "labwc.log").read_text())
            environment.update({
                "WAYLAND_DISPLAY": sockets[0].name,
                "TELEBAR_ISOLATED_WAYLAND": "1",
                "TELEBAR_TEST_TDLIB": "/native/lib/libtdjson.so",
            })
            subprocess.run(
                ["/test-runtime", "native_driver::tests::clipboard_offer_is_verified_by_the_actual_isolated_consumer",
                 "--ignored", "--exact", "--nocapture"],
                env=environment, check=True, timeout=15,
            )
            print(json.dumps({"isolated_wayland": True, "actual_driver_offer": True,
                              "read_after_ack_and_disconnect": True,
                              "actual_image_and_files_requests": True,
                              "uri_database_missing_and_duplicate_admission": True,
                              "mismatched_selection_rejected": True,
                              "owned_shutdown": True, "real_session_access": False}))
        finally:
            stop_owned(server)
