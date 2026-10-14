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

## Offscreen rendering

Discover the currently available QML-preview MCP and call healthcheck. For icon
or action renders, use generated `imports/qs` and fixture root, explicit dependency
paths, locale, DPR 1/2, radius 0/8, a settled `ready` property and strict diagnostics.
The native static adapter is offscreen, not a sandbox. Inspect each PNG through
the harness. Geometry/hash checks, image/native design review and pointer/key
QtTest are separate evidence. Do not start a second shared shell or change the
live theme to produce preview evidence.

`MainSidebarConsumer.qml` preserves complete Main with `autoStart:false` and blank
socket, unshown native surfaces and the actual content subtree. Direct signal/
state activation is not physical desktop input. `Sidebar.qml` uses actual
SidebarHandle. `tst_send_contract.qml` keeps recorder CaptureSession inactive and
checks original-destination/recovery ownership without devices.

The bundled [inert imports](imports/README.md) and their provenance remain for
content-only QtTest. Fixtures requiring unsupported native windows/imports are
not automatically certified by that adapter. Optional suites requiring external
media/native host contexts stay separate from default checks.
