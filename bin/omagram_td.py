"""omagram_td -- TDLib's JSON interface and the keyring, for Omagram's service.

TDLib is loaded from Omagram's own data directory and only if the library is a regular file
you own that nobody else can write, inside directories you own: a library is code, and the
process that loads it holds the Telegram session.

Secrets never pass through argv, files or logs. The API id and hash you enter, and the key
that encrypts TDLib's local database, live in the Secret Service keyring and move through
`secret-tool` on stdin and stdout. TDLib's own log is switched off, because at higher
verbosity it contains message text.
"""
import base64
import binascii
import ctypes
import json
import locale
import os
import pathlib
import re
import secrets
import stat
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import plugin_safety as safe  # noqa: E402

DATA = pathlib.Path(safe.home_dir()) / ".local/share/omagram"
LIB = DATA / "lib" / "libtdjson.so"
DATABASE = DATA / "database"
FILES = DATA / "files"
# Where TDLib puts downloaded media. Stickers, thumbnails, profile photos and wallpapers
# live beside its database; everything else under FILES. Only paths under these are ever
# shown to the UI -- never the database directory itself (db.sqlite, td.binlog).
# What you send stays readable too: voice and video messages and photos are prepared in SENT (kept
# in step with omagram_media), and TDLib goes on pointing at those files after sending. REC is where
# recordings were made before they moved to SENT.
SENT = DATA / "sent"
REC = pathlib.Path(safe.runtime_dir()) / "omagram" / "rec"
MEDIA_ROOTS = ((str(FILES),) + tuple(str(DATABASE / name) for name in ("stickers", "thumbnails", "profile_photos", "wallpapers", "stories"))
               + (str(SENT), str(REC)))

DEFAULT_ACCOUNT_ID = "default"
ACCOUNTS_DIR = DATA / "accounts"
ACCOUNTS_FILE = DATA / "accounts.json"
ACCOUNT_ID_RE = re.compile(r"^[A-Za-z0-9_.-]{1,64}$")


def valid_account_id(account_id):
    return (isinstance(account_id, str) and account_id not in (".", "..")
            and ACCOUNT_ID_RE.fullmatch(account_id) is not None and ".." not in account_id)


def account_paths(account_id=DEFAULT_ACCOUNT_ID):
    """Paths for a given account: default account preserves historical paths directly in DATA."""
    if not valid_account_id(account_id):
        raise safe.UnsafeError("invalid account id")
    if account_id == DEFAULT_ACCOUNT_ID:
        return {
            "id": DEFAULT_ACCOUNT_ID,
            "root": DATA,
            "database": DATABASE,
            "files": FILES,
            "sent": SENT,
        }
    acc_root = ACCOUNTS_DIR / account_id
    return {
        "id": account_id,
        "root": acc_root,
        "database": acc_root / "database",
        "files": acc_root / "files",
        "sent": acc_root / "sent",
    }


def media_roots_for(account_id=DEFAULT_ACCOUNT_ID):
    p = account_paths(account_id)
    db = p["database"]
    files = p["files"]
    sent = p["sent"]
    return ((str(files),) + tuple(str(db / name) for name in ("stickers", "thumbnails", "profile_photos", "wallpapers", "stories"))
            + (str(sent), str(REC)))


def all_database_dirs():
    """Every database directory across all accounts (for upload and protection checks)."""
    dirs = [str(DATABASE)]
    data = load_accounts()
    for acc in data.get("accounts", []):
        aid = acc.get("id")
        if aid and aid != DEFAULT_ACCOUNT_ID:
            db_dir = str(account_paths(aid)["database"])
            if db_dir not in dirs:
                dirs.append(db_dir)
    return tuple(dirs)


def load_accounts():
    default_data = {
        "active": DEFAULT_ACCOUNT_ID,
        "accounts": [
            {"id": DEFAULT_ACCOUNT_ID, "name": "Account 1", "phone": "", "meId": 0}
        ]
    }
    data = safe.read_json(ACCOUNTS_FILE, 64 * 1024, default=None, max_depth=6, max_items=256, max_string=1024)
    if not isinstance(data, dict):
        return default_data
    accounts = data.get("accounts")
    if not isinstance(accounts, list) or not accounts:
        return default_data
    valid_accounts = []
    seen = set()
    for a in accounts[:10]:
        if isinstance(a, dict) and valid_account_id(a.get("id")) and a["id"] not in seen:
            seen.add(a["id"])
            me_id = a.get("meId", 0)
            if isinstance(me_id, bool) or not isinstance(me_id, int) or not 0 <= me_id < 2 ** 53:
                me_id = 0
            valid_accounts.append({
                "id": a["id"],
                "name": (a.get("name") if isinstance(a.get("name"), str) else a["id"])[:80],
                "phone": (a.get("phone") if isinstance(a.get("phone"), str) else "")[:32],
                "meId": me_id
            })
    if not valid_accounts:
        return default_data
    active = data.get("active")
    ids = [a["id"] for a in valid_accounts]
    if active not in ids:
        active = ids[0]
    return {"active": active, "accounts": valid_accounts}


def save_accounts(data):
    safe.ensure_dir(DATA, 0o700)
    safe.write_json(ACCOUNTS_FILE, data, mode=0o600)


KEYRING_SERVICE = "omagram"
KEYRING_TIMEOUT = 60        # an unlock prompt from the keyring daemon may be on screen
KEYRING_OUTPUT_MAX = 4096
# A single TDLib object: a page of chat history with long texts stays far below this.
RESPONSE_MAX = 32 * 1024 * 1024

API_ID = re.compile(r"[1-9][0-9]{0,9}")
API_HASH = re.compile(r"[0-9a-f]{32}")


class TdUnavailable(Exception):
    """TDLib cannot be used: not built yet, unsafe to load, or its key cannot be had."""


# ---------------------------------------------------------------- the library

def _owned_dir(path):
    st = os.stat(path, follow_symlinks=False)
    if not stat.S_ISDIR(st.st_mode) or st.st_uid != os.getuid() or st.st_mode & 0o022:
        raise TdUnavailable(f"{path} is not a private directory you own")


def library_path(lib=None):
    lib = pathlib.Path(lib or LIB)
    try:
        for directory in (lib.parent.parent, lib.parent):
            _owned_dir(directory)
        st = os.stat(lib, follow_symlinks=False)
    except FileNotFoundError:
        raise TdUnavailable("TDLib is not built yet") from None
    except OSError as e:
        raise TdUnavailable(f"TDLib cannot be checked: {e.strerror}") from None
    if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid() or st.st_mode & 0o022:
        raise TdUnavailable(f"{lib} is not a regular file you own that only you can write")
    return str(lib)


class TdClient:
    """One TDLib client over the JSON interface. td_send and td_receive are thread-safe;
    td_receive returns updates for every client in the process, tagged with @client_id."""

    def __init__(self, path):
        try:
            self.lib = ctypes.CDLL(path)
        except OSError as e:
            raise TdUnavailable(f"TDLib could not be loaded: {e}") from None
        self.lib.td_create_client_id.restype = ctypes.c_int
        self.lib.td_create_client_id.argtypes = []
        self.lib.td_send.restype = None
        self.lib.td_send.argtypes = [ctypes.c_int, ctypes.c_char_p]
        self.lib.td_receive.restype = ctypes.c_char_p
        self.lib.td_receive.argtypes = [ctypes.c_double]
        self.lib.td_execute.restype = ctypes.c_char_p
        self.lib.td_execute.argtypes = [ctypes.c_char_p]
        self.execute({"@type": "setLogStream", "log_stream": {"@type": "logStreamEmpty"}})
        self.execute({"@type": "setLogVerbosityLevel", "new_verbosity_level": 0})
        self.client_id = self.lib.td_create_client_id()

    def new_client(self):
        """A fresh client id in the same library, e.g. after logging out."""
        self.client_id = self.lib.td_create_client_id()
        return self.client_id

    def create_client(self):
        """Create an additional independent client id in this library for multi-account."""
        return self.lib.td_create_client_id()

    def send_client(self, client_id, query):
        """Send a query to a specific client id."""
        self.lib.td_send(client_id, json.dumps(query, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))

    def execute(self, query):
        raw = self.lib.td_execute(json.dumps(query, separators=(",", ":")).encode("utf-8"))
        return json.loads(raw) if raw else None

    def send(self, query):
        self.lib.td_send(self.client_id, json.dumps(query, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))

    def receive(self, timeout):
        raw = self.lib.td_receive(timeout)
        if not raw or len(raw) > RESPONSE_MAX:
            return None
        try:
            value = json.loads(raw)
        except ValueError:
            return None
        return value if isinstance(value, dict) else None


def open_client():
    return TdClient(library_path())


# ---------------------------------------------------------------- the keyring

def keyring_get(key):
    """(found, value). found is False only when the keyring answered that there is no such
    secret; a keyring that could not be asked raises TdUnavailable, so a transient failure
    is never mistaken for "no key yet"."""
    try:
        r = safe.run(["secret-tool", "lookup", "service", KEYRING_SERVICE, "key", key],
                     timeout=KEYRING_TIMEOUT, max_output=KEYRING_OUTPUT_MAX)
    except (safe.UnsafeError, OSError) as e:
        raise TdUnavailable(f"the keyring cannot be reached: {e}") from None
    if r.timed_out or r.truncated:
        raise TdUnavailable("the keyring did not answer")
    if r.returncode == 0 and r.stdout:
        return True, r.stdout.decode("utf-8", "replace")
    if r.returncode in (0, 1) and not r.stdout:
        return False, None
    raise TdUnavailable("the keyring refused the lookup")


def keyring_set(key, label, value):
    try:
        r = safe.run(["secret-tool", "store", f"--label={label}", "service", KEYRING_SERVICE, "key", key],
                     input=value, timeout=KEYRING_TIMEOUT, max_output=KEYRING_OUTPUT_MAX)
    except (safe.UnsafeError, OSError):
        return False
    return r.ok


def keyring_clear(key):
    try:
        r = safe.run(["secret-tool", "clear", "service", KEYRING_SERVICE, "key", key],
                     timeout=KEYRING_TIMEOUT, max_output=KEYRING_OUTPUT_MAX)
    except (safe.UnsafeError, OSError):
        return False
    return r.ok


def valid_credentials(api_id, api_hash):
    return (isinstance(api_id, str) and API_ID.fullmatch(api_id) is not None
            and isinstance(api_hash, str) and API_HASH.fullmatch(api_hash) is not None)


def load_credentials():
    """(api_id, api_hash) from the keyring, or None if they are not there (or malformed)."""
    found_id, api_id = keyring_get("api_id")
    found_hash, api_hash = keyring_get("api_hash")
    if found_id and found_hash and valid_credentials(api_id, api_hash):
        return int(api_id), api_hash
    return None


def save_credentials(api_id, api_hash):
    if not valid_credentials(api_id, api_hash):
        return False
    return (keyring_set("api_id", "Omagram API id", api_id)
            and keyring_set("api_hash", "Omagram API hash", api_hash))


def _valid_key(value):
    try:
        return isinstance(value, str) and len(base64.b64decode(value, validate=True)) == 32
    except (binascii.Error, ValueError):
        return False


def database_exists(directory=None, account_id=DEFAULT_ACCOUNT_ID):
    if directory is None:
        directory = DATABASE if account_id == DEFAULT_ACCOUNT_ID else account_paths(account_id)["database"]
    directory = pathlib.Path(directory)
    try:
        with os.scandir(directory) as entries:
            return any(entries)
    except FileNotFoundError:
        return False


def database_key(account_id=DEFAULT_ACCOUNT_ID):
    """The database encryption key, created on first use. A key is only ever created when
    there is no database yet: making a new one for an existing database would lock the
    cached chats away for good, so a missing key with a database present is an error."""
    key_name = "database_key" if account_id == DEFAULT_ACCOUNT_ID else f"database_key_{account_id}"
    label = "Omagram database key" if account_id == DEFAULT_ACCOUNT_ID else f"Omagram database key ({account_id})"
    found, value = keyring_get(key_name)
    if found:
        if not _valid_key(value):
            raise TdUnavailable("the database key in the keyring is malformed")
        return value
    db_dir = DATABASE if account_id == DEFAULT_ACCOUNT_ID else account_paths(account_id)["database"]
    if database_exists(db_dir):
        raise TdUnavailable("the database key is missing from the keyring")
    value = base64.b64encode(secrets.token_bytes(32)).decode("ascii")
    if not keyring_set(key_name, label, value):
        raise TdUnavailable("the database key could not be stored in the keyring")
    return value


def language_code():
    lang = (locale.getlocale()[0] or os.environ.get("LANG") or "en").split(".")[0]
    code = lang.replace("_", "-")
    return code if re.fullmatch(r"[A-Za-z]{2,3}(?:-[A-Za-z0-9]{2,8})?", code) else "en"


def tdlib_parameters(api_id, api_hash, db_key, version, account_id=DEFAULT_ACCOUNT_ID):
    p = account_paths(account_id)
    for directory in (p["root"], p["database"], p["files"], p["sent"]):
        safe.ensure_dir(directory, 0o700)
    return {
        "@type": "setTdlibParameters",
        "use_test_dc": False,
        "database_directory": str(p["database"]),
        "files_directory": str(p["files"]),
        "database_encryption_key": db_key,
        "use_file_database": True,
        "use_chat_info_database": True,
        "use_message_database": True,
        "use_secret_chats": True,
        "api_id": api_id,
        "api_hash": api_hash,
        "system_language_code": language_code(),
        "device_model": "Omarchy",
        "system_version": "",
        "application_version": version,
    }
