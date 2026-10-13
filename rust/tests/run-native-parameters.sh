#!/usr/bin/bash
# Official native startup in private temporary profiles with zero network/home
# credentials/session bus/desktop/devices. The two database trees are disposable.
set -euo pipefail
if (( $# != 2 )); then
  printf 'Usage: bash rust/tests/run-native-parameters.sh <exact-test-binary> <verified-tdlib>\n' >&2
  exit 2
fi
binary=$(realpath -- "$1")
library=$(realpath -- "$2")
[[ -x "$binary" && -f "$library" ]]
exec timeout 30s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home \
  --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group \
  --dir /native --dir /native/lib --ro-bind "$library" /native/lib/libtdjson.so \
  --ro-bind "$binary" /test-native-parameters --clearenv --setenv PATH /usr/bin \
  --setenv TELEBAR_ISOLATED_PARAMETERS 1 --setenv TELEBAR_TEST_TDLIB /native/lib/libtdjson.so \
  /test-native-parameters official_parameters_reach_authorization_with_distinct_disposable_databases --ignored --exact --nocapture
