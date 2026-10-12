#!/usr/bin/bash
set -euo pipefail
(( $# == 2 )) || { printf 'Usage: bash rust/tests/run-qml-socket.sh <exact-runtime> <verified-tdlib>\n' >&2; exit 2; }
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
binary=$(realpath -- "$1")
library=$(realpath -- "$2")
exec timeout 25s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home \
  --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group \
  --ro-bind "$root" /workspace --ro-bind "$binary" /runtime --ro-bind "$library" /native/lib/libtdjson.so \
  --clearenv --setenv PATH /usr/bin --setenv TELEBAR_ISOLATED_CONSUMER 1 \
  /usr/bin/python3 -B /workspace/rust/tests/qml_socket.py /runtime /native/lib/libtdjson.so
