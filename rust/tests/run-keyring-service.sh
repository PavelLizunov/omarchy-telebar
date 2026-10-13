#!/usr/bin/bash
# Root-helper identity preserved by Landlock, not remapping host root. Only
# immutable /usr, four inert pseudo-devices and a new private tree are admitted.
# AF_UNIX-only seccomp and Landlock ABI9 Unix/file/device denial fail closed.
set -euo pipefail
if (( $# != 3 )); then
  printf 'Usage: bash rust/tests/run-keyring-service.sh <exact-test-binary> <verified-tdlib> <compiled-keyring-sandbox>\n' >&2
  exit 2
fi
binary=$(realpath -- "$1")
library=$(realpath -- "$2")
sandbox=$(realpath -- "$3")
fixture=$(realpath -- "$(dirname -- "$0")/keyring_fixture.py")
[[ -x "$binary" && -f "$library" && -x "$sandbox" && -f "$fixture" ]]
unit="telebar-rust-private-keyring-$$-$RANDOM"
exec timeout 65s systemd-run --user --quiet --collect --wait --pipe --service-type=exec \
  --unit="$unit" --property=RuntimeMaxSec=55s --property=TimeoutStopSec=5s \
  --property=KillMode=control-group --property=NoNewPrivileges=yes \
  --property=RestrictAddressFamilies=AF_UNIX \
  /usr/bin/env -i PATH=/usr/bin /usr/bin/python3 -I "$fixture" "$binary" "$library" "$sandbox"
