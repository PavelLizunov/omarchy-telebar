#!/usr/bin/bash
# Private normal bridge route with actual owned proxy and synthetic native ping.
set -euo pipefail
(( $# == 3 )) || { printf 'Usage: bash rust/tests/run-bridge-driver.sh <exact-send-test> <compiled-send-fixture> <exact-runtime>\n' >&2; exit 2; }
binary=$(realpath -- "$1")
library=$(realpath -- "$2")
runtime=$(realpath -- "$3")
[[ -x "$binary" && -f "$library" && -x "$runtime" ]]
exec timeout 20s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home \
  --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group \
  --dir /native --dir /native/lib --ro-bind "$library" /native/lib/libtdjson.so \
  --ro-bind "$runtime" /native/telebar-runtime --ro-bind "$binary" /test-send \
  --clearenv --setenv PATH /usr/bin --setenv TELEBAR_ISOLATED_BRIDGE 1 \
  --setenv TELEBAR_TEST_INERT_ABI /native/lib/libtdjson.so \
  --setenv TELEBAR_TEST_PROXY_BINARY /native/telebar-runtime \
  /test-send actual_bridge_test_route_owns_one_proxy_and_preserves_metadata --ignored --exact --nocapture
