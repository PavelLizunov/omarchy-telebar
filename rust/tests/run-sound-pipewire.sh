#!/usr/bin/bash
# Only the private null-sink consumer; no host devices/home/network/session bus.
set -euo pipefail
if (( $# != 1 )); then
  printf 'Usage: bash rust/tests/run-sound-pipewire.sh <exact-sound-test-binary>\n' >&2
  exit 2
fi
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
binary=$(realpath -- "$1")
[[ -x "$binary" ]]
exec timeout 20s bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --proc /proc --dev /dev --tmpfs /tmp --dir /home \
  --ro-bind "$binary" /test-sound \
  --ro-bind "$root/rust/tests/pipewire-null.conf" /pipewire-null.conf \
  --ro-bind "$root/rust/tests/sound_pipewire.py" /test.py \
  --clearenv --setenv TELEBAR_ISOLATED_PIPEWIRE 1 --setenv PATH /usr/bin \
  /usr/bin/python3 -B /test.py
