#!/usr/bin/python3
"""Development-only legacy TDLib producer on exact disposable profile identities.

Run only through the identity-preserving private Secret Service fixture. No test
credentials/keys are printed. The legacy source files are explicit code inputs,
never an application fallback or ambient installed import.
"""
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import time

stage = "admission"
def report_failure(kind, value, traceback):
    # Stage/type only: never echo exception messages, native payloads or keys.
    print(json.dumps({"handoffFailureStage": stage, "exceptionType": kind.__name__}), file=sys.stderr)
sys.excepthook = report_failure
assert os.environ.get("TELEBAR_PRIVATE_KEYRING") == "1"
assert len(sys.argv) == 2 and sys.argv[1] in ("--lock-refusal", "--handoff")
root = Path(os.environ["HOME"])
runtime = Path(os.environ["XDG_RUNTIME_DIR"])
private = Path(os.environ["TELEBAR_PRIVATE_ROOT"])
assert root.is_relative_to(private) and runtime.is_relative_to(private)
assert root.is_dir() and not root.is_symlink() and root.stat().st_mode & 0o777 == 0o700
assert not Path("/run/user/1000/bus").is_file()
fd = os.open(runtime / "omagram.lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
try:
    fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
except BlockingIOError:
    os.close(fd)
    if sys.argv[1] == "--lock-refusal":
        print("blocked")
        raise SystemExit(73)
    raise AssertionError("private handoff still has another owner") from None
assert sys.argv[1] == "--handoff", "lock-refusal control unexpectedly acquired ownership"
code = private / "legacy-oracle"
# Each dependency is frozen by the outer wrapper. No ambient import paths.
manifest = json.loads((code / "source-hashes.json").read_text())
for name in ("omagram_td.py", "plugin_safety.py"):
    assert hashlib.sha256((code / name).read_bytes()).hexdigest() == manifest[name]
# Legacy safety derives home from passwd. This test binds only that existing
# path context to the explicit private home, not executable/root trust checks.
stage = "oracle-import"
spec = importlib.util.spec_from_file_location("plugin_safety", code / "plugin_safety.py")
safe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(safe)
safe.home_dir = lambda: str(root)
safe.runtime_dir = lambda: str(runtime)
sys.modules["plugin_safety"] = safe
spec = importlib.util.spec_from_file_location("legacy_td_oracle", code / "omagram_td.py")
td = importlib.util.module_from_spec(spec)
spec.loader.exec_module(td)
lib = Path(os.environ["TELEBAR_TEST_TDLIB"])
assert lib.is_relative_to(private)
stage = "native-open"
client = td.TdClient(td.library_path(lib))
stage = "catalog"
catalog_before = td.ACCOUNTS_FILE.read_bytes()
catalog = td.load_accounts()
assert [row["id"] for row in catalog["accounts"]] == ["other", "default"]
assert catalog["active"] == "other"
stage = "credentials"
credentials = td.load_credentials()
assert credentials is not None
api_id, api_hash = credentials
stage = "keys"
keys = {account: td.database_key(account) for account in ("default", "other")}
clients = {client.client_id: "other", client.create_client(): "default"}
# The real legacy Daemon.start_td activates every new lazy client with getOption.
# create_client_id alone does not start authorization updates on this JSON ABI.
for cid in clients:
    client.send_client(cid, {"@type": "getOption", "name": "version"})
prepared = set()
phone = set()
closed = set()
stage = "parameters"
end = time.monotonic() + 8
while time.monotonic() < end and len(phone) < 2:
    event = client.receive(.05)
    if not event:
        continue
    cid = event.get("@client_id")
    if cid not in clients or event.get("@type") != "updateAuthorizationState":
        continue
    state = event.get("authorization_state", {}).get("@type")
    if state == "authorizationStateWaitTdlibParameters":
        assert cid not in prepared
        prepared.add(cid)
        account = clients[cid]
        client.send_client(cid, td.tdlib_parameters(api_id, api_hash, keys[account], "private-legacy-handoff", account))
    elif state == "authorizationStateWaitPhoneNumber":
        phone.add(cid)
assert len(phone) == len(prepared) == 2, "legacy disposable startup did not reach phone"
stage = "close"
for cid in clients:
    client.send_client(cid, {"@type": "close"})
end = time.monotonic() + 4
while time.monotonic() < end and len(closed) < 2:
    event = client.receive(.05)
    if event and event.get("@client_id") in clients and event.get("@type") == "updateAuthorizationState" and event.get("authorization_state", {}).get("@type") == "authorizationStateClosed":
        closed.add(event["@client_id"])
assert len(closed) == 2, "legacy clients did not close before deadline"
stage = "post-close-identities"
assert td.ACCOUNTS_FILE.read_bytes() == catalog_before
assert all(td.database_key(account) == key for account, key in keys.items())
print(json.dumps({"legacyOfficialAccounts": 2, "parametersOnce": 2, "authorization": "phone",
                  "closed": 2, "catalogPreserved": True, "keysPreserved": True,
                  "workingProfiles": False, "network": False}))
os.close(fd)
