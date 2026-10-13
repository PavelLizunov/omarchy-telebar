#!/usr/bin/bash
# One requested-migration public preauthorization transport check, no retry.
# Share only host networking: private home/proc/devices/tmp, no session bus,
# credentials or account databases. The proxy admits fixed Telegram targets.
set -euo pipefail
(( $# == 1 )) || { printf 'Usage: bash rust/tests/run-proxy-remote.sh <exact-runtime>\n' >&2; exit 2; }
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
binary=$(realpath -- "$1")
[[ -x "$binary" ]]
exec timeout 85s bwrap --unshare-all --share-net --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --ro-bind /etc/ssl /etc/ssl \
  --ro-bind /etc/ca-certificates/extracted /etc/ca-certificates/extracted \
  --ro-bind /etc/resolv.conf /etc/resolv.conf \
  --ro-bind /etc/nsswitch.conf /etc/nsswitch.conf \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home \
  --ro-bind "$binary" /test-runtime \
  --ro-bind "$root/rust/tests/proxy_remote.py" /test.py \
  --ro-bind "$root/bin/vendor/flowseal/transport.py" /oracle/transport.py \
  --clearenv --setenv PATH /usr/bin --setenv TELEBAR_PUBLIC_PREAUTH_PROXY 1 \
  /usr/bin/python3 -I /test.py /test-runtime
