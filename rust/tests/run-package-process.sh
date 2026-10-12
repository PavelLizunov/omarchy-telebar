#!/usr/bin/bash
# Explicit reviewed package only. Never install it as part of this check.
set -euo pipefail
(( $# == 1 )) || { printf 'Usage: bash rust/tests/run-package-process.sh <native-candidate-package>\n' >&2; exit 2; }
package=$(realpath -- "$1")
script=$(realpath -- "$(dirname -- "$0")/package_process.py")
[[ -x "$package/bin/telebar-runtime" && -f "$package/runtime-candidate.json" ]]
exec timeout 25s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home \
  --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group \
  --ro-bind "$package" /candidate --ro-bind "$script" /package_process.py \
  --clearenv --setenv PATH /usr/bin --setenv TELEBAR_ISOLATED_PACKAGE 1 \
  /usr/bin/python3 -B /package_process.py /candidate
