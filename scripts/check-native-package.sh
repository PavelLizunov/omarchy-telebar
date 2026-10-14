#!/usr/bin/bash
# Actual artifact in a read-only, no-network, no-session disposable consumer.
set -euo pipefail
(( $# == 1 )) || { printf 'Usage: bash scripts/check-native-package.sh <native-package>\n' >&2; exit 2; }
package=$(realpath -- "$1")
source=$(realpath -- "$(dirname -- "$0")/..")
[[ -x "$package/bin/telebar-runtime" && -f "$package/runtime-candidate.json" ]]
exec timeout 30s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
 --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
 --proc /proc --dev /dev --tmpfs /tmp --dir /home \
 --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group \
 --ro-bind "$source" /workspace --ro-bind "$package" /candidate \
 --clearenv --setenv PATH /usr/bin --setenv TELEBAR_ISOLATED_PACKAGE 1 \
 /usr/bin/node /workspace/tests/native-package-consumer.mjs /candidate
