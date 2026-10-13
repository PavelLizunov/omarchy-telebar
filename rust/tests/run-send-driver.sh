#!/usr/bin/bash
# Actual Driver/helper send acceptance against the explicit synthetic ABI only.
# Reuse the reviewed identity-preserving Landlock/AF_UNIX confinement, never a
# real profile, physical device, host bus/keyring, network or Telegram send.
set -euo pipefail
if (( $# != 3 )); then
  printf 'Usage: bash rust/tests/run-send-driver.sh <exact-test-binary> <compiled-send-fixture> <compiled-keyring-sandbox>\n' >&2
  exit 2
fi
binary=$(realpath -- "$1")
library=$(realpath -- "$2")
sandbox=$(realpath -- "$3")
fixture=$(realpath -- "$(dirname -- "$0")/keyring_fixture.py")
[[ -x "$binary" && -f "$library" && -x "$sandbox" && -f "$fixture" ]]
unit="telebar-rust-private-send-$$-$RANDOM"
exec timeout 65s systemd-run --user --quiet --collect --wait --pipe --service-type=exec \
  --unit="$unit" --property=RuntimeMaxSec=55s --property=TimeoutStopSec=5s \
  --property=KillMode=control-group --property=NoNewPrivileges=yes \
  --property=RestrictAddressFamilies=AF_UNIX \
  /usr/bin/env -i PATH=/usr/bin /usr/bin/python3 -I "$fixture" "$binary" "$library" "$sandbox" --send-fixture
