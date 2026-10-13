#!/usr/bin/bash
# Official proxy config/reopen/restore on disposable unauthorized databases.
# Zero network/home/credentials/bus/devices; application ELF ownership retained.
set -euo pipefail
(( $# == 3 )) || { printf 'Usage: bash rust/tests/run-native-proxy.sh <exact-test-binary> <verified-tdlib> <exact-runtime>\n' >&2; exit 2; }
binary=$(realpath -- "$1")
library=$(realpath -- "$2")
runtime=$(realpath -- "$3")
[[ -x "$binary" && -f "$library" && -x "$runtime" ]]
exec timeout 50s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home \
  --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group \
  --dir /native --dir /native/lib --ro-bind "$library" /native/lib/libtdjson.so \
  --ro-bind "$runtime" /native/telebar-runtime \
  --ro-bind "$binary" /test-native-proxy --clearenv --setenv PATH /usr/bin \
  --setenv TELEBAR_ISOLATED_NATIVE_PROXY 1 --setenv TELEBAR_TEST_TDLIB /native/lib/libtdjson.so \
  --setenv TELEBAR_TEST_PROXY_BINARY /native/telebar-runtime \
  /test-native-proxy official_proxy_restore_preserves_saved_and_previous_account_identity --ignored --exact --nocapture
