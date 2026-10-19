#!/usr/bin/bash
set -euo pipefail
# Build the reviewed Qt test helper outside the checkout, then use a private bus.
root=$(cd -- "$(dirname -- "$0")/.." && pwd)
fixture=${1:?Usage: run-file-portal.sh /absolute/prepared/fixture}
work=$(mktemp -d "${TMPDIR:-/tmp}/telebar-file-portal-XXXXXX")
trap 'rm -rf -- "$work"' EXIT
g++ -std=c++17 -fPIC "$root/tests/file-portal-runner.cpp" $(pkg-config --cflags --libs Qt6QuickTest Qt6DBus) -o "$work/portal-test"
mkdir "$work/services"
printf '<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">\n<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><policy context="default"><allow own="*"/><allow send_destination="*"/><allow eavesdrop="true"/></policy><servicedir>%s/services</servicedir></busconfig>\n' "$work" > "$work/bus.conf"
# No service activation directories and no compositor environment are inherited.
dbus-run-session --config-file="$work/bus.conf" -- env -u WAYLAND_DISPLAY -u DISPLAY -u HYPRLAND_INSTANCE_SIGNATURE -u QT_QPA_PLATFORMTHEME \
  bash -c '
    set -euo pipefail
    "$1/portal-test" --server >"$1/server.log" 2>&1 &
    server=$!
    trap '\''kill "$server" 2>/dev/null || true; wait "$server" 2>/dev/null || true; cat "$1/server.log"'\'' EXIT
    for ((i=0; i<100; i++)); do grep -q PORTAL_READY "$1/server.log" && break; sleep 0.02; done
    grep -q PORTAL_READY "$1/server.log"
    TELEBAR_FILE_PORTAL_TEST=1 QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_QPA_PLATFORMTHEME=xdgdesktopportal \
      "$1/portal-test" -input "$2/tests/visual/tst_file_portal.qml" -import "$2/imports" -import "$2/inert" -import "$2"
    grep -q "PORTAL_OPEN multiple=1 photo=0" "$1/server.log"
    grep -q "PORTAL_OPEN multiple=0 photo=1" "$1/server.log"
  ' _ "$work" "$fixture"
