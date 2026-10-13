#!/usr/bin/bash
# Actual test-build Driver video commands; cfg(test) lavfi only, no device inputs.
set -euo pipefail
(( $# == 3 )) || { printf 'Usage: bash rust/tests/run-video-driver.sh <exact-lib-test> <compiled-send-fixture> <compiled-keyring-sandbox>\n' >&2; exit 2; }
binary=$(realpath -- "$1")
library=$(realpath -- "$2")
sandbox=$(realpath -- "$3")
fixture=$(realpath -- "$(dirname -- "$0")/keyring_fixture.py")
[[ -x "$binary" && -f "$library" && -x "$sandbox" && -f "$fixture" ]]
unit="telebar-rust-private-video-$$-$RANDOM"
exec timeout 65s systemd-run --user --quiet --collect --wait --pipe --service-type=exec \
  --unit="$unit" --property=RuntimeMaxSec=55s --property=TimeoutStopSec=5s \
  --property=KillMode=control-group --property=NoNewPrivileges=yes \
  --property=RestrictAddressFamilies=AF_UNIX \
  /usr/bin/env -i PATH=/usr/bin /usr/bin/python3 -I "$fixture" "$binary" "$library" "$sandbox" --video-fixture
