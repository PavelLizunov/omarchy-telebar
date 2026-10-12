#!/usr/bin/bash
# Reviewed offscreen clipboard fixture. Never expose the live Wayland socket,
# session bus, real home, physical devices or network inside this namespace.
set -euo pipefail
if (( $# != 3 )); then
  printf 'Usage: bash rust/tests/run-clipboard-wayland.sh <exact-lib-test-binary> <verified-tdlib> <labwc-package-root>\n' >&2
  exit 2
fi
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
binary=$(realpath -- "$1")
library=$(realpath -- "$2")
fixture=$(realpath -- "$3")
[[ -x "$binary" && -f "$library" && -x "$fixture/usr/bin/labwc" ]]
manifest=$(mktemp --tmpdir telebar-wayland-helpers-XXXXXX.json)
trap 'rm -f -- "$manifest"' EXIT
/usr/bin/python3 - "$manifest" <<'PY'
import hashlib, json, pathlib, sys
paths = [pathlib.Path('/usr/bin/wl-copy'), pathlib.Path('/usr/bin/wl-paste'), pathlib.Path('/usr/bin/cat'), pathlib.Path('/usr/bin/sleep')]
for path in paths:
    actual = path.resolve()
    for entry in [actual, *actual.parents]:
        info = entry.stat()
        assert info.st_uid == 0 and not info.st_mode & 0o022
    assert actual.is_file() and actual.stat().st_mode & 0o111
pathlib.Path(sys.argv[1]).write_text(json.dumps({str(p.resolve()): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}))
PY
timeout 25s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home \
  --ro-bind "$fixture" /fixture --dir /native --dir /native/lib \
  --ro-bind "$library" /native/lib/libtdjson.so --ro-bind "$binary" /test-runtime \
  --ro-bind "$manifest" /test-helper-manifest.json \
  --ro-bind "$root/rust/tests/clipboard_wayland.py" /test.py \
  --clearenv --setenv TELEBAR_ISOLATED_WAYLAND 1 --setenv PATH /usr/bin \
  /usr/bin/python3 -B /test.py
