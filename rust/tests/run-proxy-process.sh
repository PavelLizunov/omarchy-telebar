#!/usr/bin/bash
# Bounded actual loopback proxy with no host accounts/bus/devices/network.
set -euo pipefail
(( $# == 1 )) || { printf 'Usage: bash rust/tests/run-proxy-process.sh <exact-runtime>\n' >&2; exit 2; }
binary=$(realpath -- "$1")
script=$(realpath -- "$(dirname -- "$0")/proxy_process.py")
exec timeout 15s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home \
  --ro-bind "$binary" /test-runtime --ro-bind "$script" /test.py \
  --clearenv --setenv PATH /usr/bin --setenv TELEBAR_ISOLATED_PROXY 1 \
  /usr/bin/python3 -B /test.py /test-runtime
