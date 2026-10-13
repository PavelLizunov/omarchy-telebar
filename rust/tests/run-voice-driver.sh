#!/usr/bin/bash
# Actual voice.start/stop/Capture/Pulse routes with synthetic audio and send ABI.
# Root tool identity remains intact inside the reviewed Landlock/AF_UNIX owner.
set -euo pipefail
if (( $# != 3 )); then
  printf 'Usage: bash rust/tests/run-voice-driver.sh <exact-send-test> <compiled-send-fixture> <compiled-keyring-sandbox>\n' >&2
  exit 2
fi
binary=$(realpath -- "$1")
library=$(realpath -- "$2")
sandbox=$(realpath -- "$3")
fixture=$(realpath -- "$(dirname -- "$0")/keyring_fixture.py")
[[ -x "$binary" && -f "$library" && -x "$sandbox" && -f "$fixture" ]]
unit="telebar-rust-private-voice-$$-$RANDOM"
exec timeout 65s systemd-run --user --quiet --collect --wait --pipe --service-type=exec \
  --unit="$unit" --property=RuntimeMaxSec=55s --property=TimeoutStopSec=5s \
  --property=KillMode=control-group --property=NoNewPrivileges=yes \
  --property=RestrictAddressFamilies=AF_UNIX \
  /usr/bin/env -i PATH=/usr/bin /usr/bin/python3 -I "$fixture" "$binary" "$library" "$sandbox" --voice-fixture
