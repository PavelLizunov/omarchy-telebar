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
exec timeout 25s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home \
  --ro-bind "$fixture" /fixture --dir /native --dir /native/lib \
  --ro-bind "$library" /native/lib/libtdjson.so --ro-bind "$binary" /test-runtime \
  --ro-bind "$root/rust/tests/clipboard_wayland.py" /test.py \
  --clearenv --setenv TELEBAR_ISOLATED_WAYLAND 1 --setenv PATH /usr/bin \
  /usr/bin/python3 -B /test.py
