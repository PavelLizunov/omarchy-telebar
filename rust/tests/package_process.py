#!/usr/bin/python3
"""Finite actual native package process/argv check. Development driver only.

Must run inside run-package-process.sh: no home, network, bus or session devices.
The one native Rust ELF still uses system libraries; no application Python code
is packaged, loaded or allowed as a fallback.
"""
import hashlib
import json
import os
import pathlib
import socket
import subprocess
import sys
import tempfile

assert os.environ.get("TELEBAR_ISOLATED_PACKAGE") == "1"
package = pathlib.Path(sys.argv[1])
assert os.statvfs(package).f_flag & os.ST_RDONLY
assert all(name == "lo" for _, name in socket.if_nameindex())
record = json.loads((package / "runtime-candidate.json").read_text())
assert record["runtime"] == "rust" and isinstance(record["cutoverReady"], bool)
for path, digest in record["sourceHashes"].items():
    assert hashlib.sha256((package / path).read_bytes()).hexdigest() == digest
files = [p for p in package.rglob("*") if p.is_file()]
assert not any(p.is_symlink() or p.suffix in (".py", ".pyc") for p in files)
with tempfile.TemporaryDirectory(prefix="telebar-package-process-") as name:
    root = pathlib.Path(name)
    marker = root / "unexpected-shell-startup"
    poison = root / "bash-env"
    poison.write_text(f"touch {marker}\nexit 71\n")
    environment = dict(os.environ, BASH_ENV=str(poison), PYTHONPATH=str(root), HOME=str(root))
    # In a launch-enabled package use invalid argv only: never test production
    # startup under this isolated package consumer's passwd/home environment.
    controls = [("omagram", ["--invalid"]), ("omagramd", ["--invalid"]),
                ("omagram-build-tdlib", ["--bad"]), ("omagram-menu-install", ["bad"])]
    if not record["cutoverReady"]:
        controls[:2] = [("omagram", []), ("omagramd", [])]
    for entry, args in controls:
        result = subprocess.run([str(package / "bin" / entry), *args], env=environment, capture_output=True, timeout=4)
        assert result.returncode == 1, (entry, result.returncode)
        assert not marker.exists()
    proxy = subprocess.Popen([str(package / "bin/omagram-proxy")], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=environment)
    try:
        proxy.stdin.write(b'{"secret":"0123456789abcdef0123456789abcdef"}\n')
        proxy.stdin.flush()
        ready = json.loads(proxy.stdout.readline())
        assert ready["event"] == "ready" and 0 < ready["port"] <= 65535
        assert pathlib.Path(f"/proc/{proxy.pid}/exe").readlink() == package / "bin/telebar-runtime"
        argv = pathlib.Path(f"/proc/{proxy.pid}/cmdline").read_bytes().split(b"\0")
        assert argv[1] == b"--proxy" and b"python" not in argv[0]
        proxy.stdin.close()
        assert proxy.wait(timeout=4) == 0
    finally:
        if proxy.poll() is None:
            proxy.terminate()
        proxy.wait(timeout=4)
    assert not marker.exists()
    assert not list(root.rglob("db.sqlite"))
print(json.dumps({"package_files":len(files),"python_files":0,"actual_rust_proxy_shim":True,"owner_eof_shutdown":True,"bash_env_ignored":True,"production_gate_matches_manifest":True,"host_profiles":False}))
