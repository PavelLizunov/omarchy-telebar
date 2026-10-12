#!/usr/bin/bash
set -euo pipefail
(( $# == 1 )) || { printf 'Usage: bash rust/tests/run-helper-parity.sh <exact-runtime>\n' >&2; exit 2; }
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
binary=$(realpath -- "$1")
exec timeout 25s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home --dir /home/worker \
  --ro-bind "$root" /workspace --ro-bind "$binary" /runtime --chdir /workspace \
  --clearenv --setenv PATH /usr/bin --setenv HOME /home/worker --setenv LANG C.UTF-8 \
  /usr/bin/python3 -B /workspace/rust/tests/helper_parity.py /runtime
