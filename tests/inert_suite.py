#!/usr/bin/python3
"""Worker/CI entry point; run only inside the reviewed isolated filesystem."""
import json
import os
import pathlib
import subprocess
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
if os.environ.get("TELEBAR_INERT_WORKER") != "1":
    raise SystemExit("Run this suite inside the documented inert worker, not on the workstation")
os.chdir(ROOT)
os.environ["XDG_RUNTIME_DIR"] = "/tmp/runtime"
os.environ.pop("HYPRLAND_INSTANCE_SIGNATURE", None)
pathlib.Path("/tmp/runtime").mkdir(mode=0o700)
sys.dont_write_bytecode = True
sys.path[:0] = [str(ROOT / "tests"), str(ROOT / "bin")]
import plugin_safety  # noqa: E402

# The isolated worker has no passwd database, real home or account storage.
plugin_safety.home_dir = lambda: "/tmp/home"
pathlib.Path("/tmp/home").mkdir(mode=0o700)
suite = unittest.TestSuite()
for path in sorted((ROOT / "tests").glob("*_test.py")):
    # Immutable exports omit Git; CI runs these three identity checks separately.
    if path.name != "rename_test.py":
        suite.addTests(unittest.defaultTestLoader.loadTestsFromName(path.stem))
result = unittest.TextTestRunner(verbosity=1).run(suite)
js_exits = []
for path in sorted((ROOT / "tests").glob("*-test.js")):
    checked = subprocess.run(["node", str(path)], timeout=15, check=False)
    js_exits.append(checked.returncode)
print("AUDIT_COUNTS", json.dumps({"python": result.testsRun,
    "failures": len(result.failures), "errors": len(result.errors),
    "skipped": len(result.skipped), "js": len(js_exits)}), flush=True)
sys.exit(0 if result.wasSuccessful() and not any(js_exits) else 1)
