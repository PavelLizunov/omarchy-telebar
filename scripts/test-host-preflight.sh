#!/usr/bin/env bash
# Inspect the existing dedicated guest through its configured DSH SSH owner.
set -euo pipefail
if (( $# )); then
  printf 'Usage: bash scripts/test-host-preflight.sh\n' >&2
  exit 2
fi
ssh -o BatchMode=yes -o ConnectTimeout=10 dsh \
  'sudo -n -u dsh ssh -o BatchMode=yes -o ConnectTimeout=8 omarchy-test bash -s' <<'GUEST'
set -euo pipefail
printf 'HOST %s USER %s\n' "$(hostname)" "$(id -un)"
printf '\nVERSIONS\n'
quickshell --version
pacman -Q qt6-base qt6-multimedia omarchy
cargo --version
node --version
printf '\nTOOLS\n'
for tool in git ffmpeg grim wtype; do command -v "$tool"; done
test -x /usr/lib/qt6/bin/qmltestrunner
printf '%s\n' /usr/lib/qt6/bin/qmltestrunner
printf '\nDESKTOP\n'
pgrep -a -x Hyprland
pgrep -a -x quickshell
# Names only: do not export the full manager environment into task evidence.
systemctl --user show-environment | grep -E '^(WAYLAND_DISPLAY|HYPRLAND_INSTANCE_SIGNATURE|XDG_RUNTIME_DIR)=' | cut -d= -f1
printf '\nRESOURCES\n'
free -m
df -h / /home
printf '\nUSER MANAGER\n'
state=$(systemctl --user is-system-running) || true
printf '%s\n' "$state"
[[ "$state" == running || "$state" == degraded ]]
GUEST
