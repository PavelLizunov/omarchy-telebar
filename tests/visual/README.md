# Actual-consumer QML checks

Fixtures load the real application/compact components with synthetic models.
They do not establish production account actions, compositor placement, device
capture, screen-reader behavior or perceived playback quality.

## Native fixture preparation

```sh
node scripts/prepare-preview.mjs action /absolute/new/action-fixture
node scripts/prepare-preview.mjs icon /absolute/new/icon-fixture
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input /absolute/new/action-fixture/tests/visual/tst_panel_action.qml -import /absolute/new/action-fixture/imports -import /absolute/new/action-fixture/inert
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input /absolute/new/action-fixture/tests/visual/tst_quick.qml -import /absolute/new/action-fixture/imports -import /absolute/new/action-fixture/inert
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input /absolute/new/icon-fixture/tests/visual/tst_ring_mark.qml -import /absolute/new/icon-fixture/imports -import /absolute/new/icon-fixture/inert
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input /absolute/new/icon-fixture/tests/visual/tst_bar_icon.qml -import /absolute/new/icon-fixture/imports -import /absolute/new/icon-fixture/inert
```

The Node.js tool copies admitted actual UI and binds product Process/FileView to
a uniquely named inert module. The client's default socket is blank before
construction. Native Commons I/O is bound explicitly to a relative inert import;
same-name shadowing is not presumed. Icon mode replaces only the internal panel/
popup window boundary. Action mode retains actual Panel/controller/QuickView with
an inert KeyboardPanel surface. The fixture tool does not deploy, start the shared
shell or connect to a production socket.

The Open Telebar label and mark exercise the real parent handler, unavailable
service and pending attachment/send/draft retention. Other suites preserve theme
radius, mark optical/hit geometry, unread states, sidebar interactions and shared
composition. `FixtureApp.qml` supplies only synthetic requests/media.

## System file chooser

`app/FilePicker.qml` delegates browsing to QtQuick.Dialogs.FileDialog. The native
window launcher selects `QT_QPA_PLATFORMTHEME=xdgdesktopportal`, so the desktop's
FileChooser preference owns the UI (for example Flea), not a Telebar file browser.
The shared shell retains its own platform theme; no global configuration changes.
A running old window must reopen to receive the launch environment.

```sh
node scripts/prepare-preview.mjs action /absolute/new/action-fixture
bash tests/run-file-portal.sh /absolute/new/action-fixture
```

The private D-Bus fixture has no activation service directories or compositor
access. It observes actual Qt portal OpenFile calls and returns temporary local
files through the real attachment/profile/compact consumers. It covers multiple
selection, cancellation and no send-on-pick. It does not verify Flea's live window
appearance or human selection. `tst_file_dialogs.qml` tests caller lifecycle with
Qt's explicit non-native offscreen option; `tst_quick.qml` verifies focus after
chooser close and send ownership. Native dialogs are separate windows and are
not included in the caller's QML subtree PNG. Capture fixtures only show callers;
do not label them screenshots of the system dialog.

## Archive refresh regression

`node tests/archive-refresh-test.js` exercises the actual navigation/model logic:
500 main chats plus 41 archived chats, cached-list hydration even when TDLib
returns load-complete 404 without new-chat events, revisits, retries and stale
account/snapshot guards. Per-list display stays bounded at 500; the shared known
chat cache is separately bounded at 5,000.

`ArchiveConsumer.qml` loads complete Main with unshown native surfaces, a blank
socket and synthetic transport replies. Readiness requires all 41 archived rows,
all 500 main rows and an updated visible preview/unread count after a streamed
chat event. Use the static native adapter and generated `imports/qs` plus fixture
root. Strict diagnostics reject the expected absent-Hyprland warning; an explicit
report-policy render may retain that isolation warning only. Synthetic events do
not establish arrival of a new real Telegram message or human acceptance.

## Offscreen rendering

Discover the currently available QML-preview MCP and call healthcheck. For icon
or action renders, use generated `imports/qs` and fixture root, explicit dependency
paths, locale, DPR 1/2, radius 0/8, a settled `ready` property and strict diagnostics.
The native static adapter is offscreen, not a sandbox. Inspect each PNG through
the harness. Geometry/hash checks, image/native design review and pointer/key
QtTest are separate evidence. Do not start a second shared shell or change the
live theme to produce preview evidence.

`MainSelectionConsumer.qml` preserves complete Main with synthetic history and
uses QtTest pointer events to drag a fragment through the actual window subtree.
Its `ready` becomes true only when `selectedText` is `fragment` and the message
owns focus. Render it with the static native adapter, generated `imports/qs` and
fixture root. The absent-Hyprland warning is expected offscreen: retain the strict
failure and use report policy only for this documented isolation warning. This
checks synthetic input, not physical Wayland input or the user's reported failure.
`tst_message_text.qml` separately checks multiline dragging, word selection,
fragment clipboard copying, links, context menu and Ctrl+click selection. Stock
QtTest needs generated `imports`, `inert` and fixture root on its import path.

`MainSidebarConsumer.qml` preserves complete Main with `autoStart:false` and blank
socket, unshown native surfaces and the actual content subtree. Direct signal/
state activation is not physical desktop input. `Sidebar.qml` uses actual
SidebarHandle. `tst_send_contract.qml` keeps recorder CaptureSession inactive and
checks original-destination/recovery ownership without devices.

The bundled [inert imports](imports/README.md) and their provenance remain for
content-only QtTest. `RoundVideoConsumer.qml` uses an explicitly generated local
480×480 H.264 test clip, never private media. `tst_round_video.qml` exercises the
real decoder, inline expand button, zoom, drag, pause, reset and close. Generate
`round-480.mp4` in the new fixture, not in the repository:

```sh
ffmpeg -hide_banner -loglevel error -f lavfi -i "testsrc2=size=480x480:rate=20:duration=4" -an -c:v libx264 -pix_fmt yuv420p -preset veryfast -movflags +faststart /absolute/new/action-fixture/tests/visual/round-480.mp4
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input /absolute/new/action-fixture/tests/visual/tst_round_video.qml -import /absolute/new/action-fixture/imports -import /absolute/new/action-fixture/inert
```

`RoundParentConsumer.qml` preserves actual Main/Quick media surfaces under the
static native adapter, with their original content reparented and surfaces kept
unshown. Stock QtTest lacks those static window modules. Qt's software video
output can decode without showing video pixels; decoder dimensions/position alone
are not visual acceptance. Use the reviewed OpenGL offscreen native adapter for
original-frame pixel changes and circular clipping. FFmpeg writes stream metadata
to stderr; strict generic rendering rejects those lines, even without a playback
error. Preserve the failed render and classify any report-policy exception.
Fixtures requiring unsupported native windows/imports are
not automatically certified by that adapter. Optional suites requiring external
media/native host contexts stay separate from default checks.
