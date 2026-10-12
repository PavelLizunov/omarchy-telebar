#!/usr/bin/bash
# Actual Rust/socket consumer without host network, accounts, keyring or devices.
set -euo pipefail
if (( $# != 2 )); then
  printf 'Usage: bash rust/tests/run-consumer-socket.sh <exact-runtime-binary> <verified-tdlib>\n' >&2
  exit 2
fi
binary=$(realpath -- "$1")
library=$(realpath -- "$2")
script=$(realpath -- "$(dirname -- "$0")/consumer_socket.py")
[[ -x "$binary" && -f "$library" && -f "$script" ]]
exec timeout 25s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home \
  --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group \
  --ro-bind "$binary" /test-runtime --ro-bind "$library" /native/lib/libtdjson.so \
  --ro-bind "$script" /consumer_socket.py --clearenv --setenv PATH /usr/bin \
  --setenv TELEBAR_ISOLATED_CONSUMER 1 \
  /usr/bin/python3 -B /consumer_socket.py /test-runtime /native/lib/libtdjson.so
