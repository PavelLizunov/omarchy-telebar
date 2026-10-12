#!/usr/bin/bash
# Actual bus/service consumer in a private namespace, never the host session bus.
set -euo pipefail
if (( $# < 1 || $# > 2 )); then
  printf 'Usage: bash rust/tests/run-notification-bus.sh <exact-test-binary>\n' >&2
  exit 2
fi
binary=$(realpath -- "$1")
[[ -x "$binary" ]]
native=()
if (( $# == 2 )); then native=(--ro-bind "$(realpath -- "$2")" /native/lib/libtdjson.so --setenv TELEBAR_TEST_TDLIB /native/lib/libtdjson.so); fi
exec timeout 20s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home \
  --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group \
  --ro-bind "$binary" /test-notification --clearenv --setenv PATH /usr/bin \
  "${native[@]}" \
  --setenv TELEBAR_ISOLATED_NOTIFICATION_BUS 1 \
  /usr/bin/dbus-run-session -- /test-notification \
  actual_notification_bus_uses_private_server_and_retires --ignored --exact --nocapture
