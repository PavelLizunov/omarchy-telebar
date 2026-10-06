# Inert QML test imports

Use this directory only as an explicit offscreen test import path. Never install
it into Omarchy, add it to a production import environment, or use it as a live
Quickshell consumer. Actual application components are still loaded by fixtures.

`qs.Commons` contains the reviewed Omarchy 4.0.4-1 token/geometry files. Provenance
and hashes are recorded in `provenance.json`; the accompanying MIT notice applies.
They retain their token logic. Their Process/FileView dependencies below are inert.
`qs.Ui` exposes only a namespace anchor for content consumers. It does not implement
host surfaces, panel positioning, monitor selection or accessibility integration.

`Quickshell` and `Quickshell.Io` are small test adapters: detached commands, writes,
file reads/watchers, and process execution do nothing. Environment values are
synthetic. No Socket, PanelWindow, PopupWindow, Hyprland or Wayland module is supplied.
A fixture needing those modules is unsupported here, not host-verified.

Requires Qt Quick/Controls/Test and the actual components' optional Qt modules
(e.g. Multimedia and FolderListModel). Record installed Qt and renderer versions.
These adapters do not verify GPU effects, real playback, account I/O or compositor
focus/placement. Pure token compatibility is not a complete runtime host contract.

```sh
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_send_contract.qml -import tests/visual/imports -import tests/visual/imports/inert
```

Commons sources live once under `qs/Commons/`. `qs/shell.qml` is only a scanner
anchor for the optional static-module preview adapter. It is never run as a shell.

QtTest uses `imports` and `imports/inert`. QML-preview with the static adapter
uses `imports/qs` and `imports/inert`, deliberately excluding the `imports` root.
That lets the adapter resolve Commons consistently while keeping Quickshell I/O
inert. Adding the root to that MCP consumer can load duplicate Style singletons
and silently lose the fixture's nonzero radius. Consumers assert the actual
composer radius before becoming ready. Hash these files as explicit dependencies.
Run fresh tool discovery and healthcheck;
inspect rendered PNGs separately from geometry/interaction checks.
