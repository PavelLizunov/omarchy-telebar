# Consumer visual review

Fixtures load the actual application QML components with `FixtureApp.qml` synthetic
models. Requests are inert; no account, network, camera, microphone or production
socket is used. `sample-photo.svg` is locally authored fixture media.

Reviewed states: icon sheet, normal/compact/long chat, topics and retry error, settings
profile/connection/shortcuts, initial setup, phone and password login, new conversation,
forward picker, poll composer, user info, quick chat list/reply, media controls, photo,
story error, emoji, sticker empty state, context menu and reacting people.

The composed chat fixture is not the `Main.qml` window: host/compositor placement
requires separate live checks. Sidebar.qml uses the same SidebarHandle as Main.qml;
its geometry and interactions are measured offscreen. Bar.qml attempts the actual bar
consumer; missing PopupWindow/host imports are a blocker, not a visual pass.

The configured QML-preview MCP is now 0.3.0. Call healthcheck, then render with explicit
importPaths, locale, DPR, readyProperty and dependencyPaths. measureObjects records
named logical scene bounds. Historical reports describe the earlier legacy renderer;
they are not evidence for subsequently changed files.
The existing `/tmp/opencode/qml-mock` adapter supplies inert Quickshell Process/FileView
and qs.Commons tokens. Effects/masks and media playback are not verified by software
captures. Record dependency hashes and renderer limitations in the evidence manifest.

`mcp_all_pages.py --paired-radius` covers 223 cases, including QR preparation and
unavailable-image states at radii 0/8. `warningsPolicy: error` and explicit
`geometryChecks` gate renders; PNG inspection and interaction checks remain separate.

View each PNG through the harness. Do not start an external image viewer or production
panel to obtain preview evidence. No global theme or renderer modifications are needed.

Interaction check for original matched-contour transitions:

```bash
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_morph.qml
```

This verifies intermediate geometry and settled states, not native GPU frame pacing.

Consumer keyboard/state checks (settings initial load/Escape, menu arrows/Enter/Escape,
forum selection, QR/phone/password transitions) use the existing inert adapter:

```bash
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_consumers.qml -import /tmp/opencode/qml-mock
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_sidebar.qml -import /tmp/opencode/qml-mock
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_quick.qml -import /tmp/opencode/qml-mock
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_bar_menu.qml -import /tmp/opencode/qml-mock
```

The review is a focused native UI walkthrough, not every dialog/action or every theme.
Narrow.qml and its state wrappers cover a 15-unit monospace font with scaled spacing,
compact actions, drafts, information overlay, selection, confirmation, attachments and
recording. The consumer test resizes the actual ChatView and checks draft retention,
Send, overflow menu activation/dismissal and information layout with inert requests.
Sidebar tests exercise click, drag from the icon and rail, drag-release without an
accidental toggle, limits, keyboard resizing, hover pixels and radius 0 → 8 → 0.
Video/camera capture and network actions are not executed in fixtures. A loading/empty
fixture is evidence of that state only. The actual bar render is blocked by the installed
adapter's missing `Quickshell.Hyprland` plugin; the QuickView content is reviewed separately.
BarMenuConsumer.qml now loads the same BarMenu.qml used by BarWidget.qml, so content,
action routing and theme geometry can be checked independently of PopupCard's window.
This does not verify the host popup's placement, focus grab or lifecycle.
QuickConsumer.qml covers sent/read/sending/failed receipts, inert send success/failure,
long drafts, attachments and recording layout. SettingsConsumer.qml covers narrow
editing/confirmation and SearchConsumer.qml covers scoped search and ten accounts.

FileDialogsConsumer.qml loads the actual shared FilePicker from ChatView and
SettingsView with synthetic local files. `tst_file_dialogs.qml` checks multiple
attachments, image filtering/profile request, cancellation, repeated open/close,
path validation and radius changes. It does not reconnect USB devices or write to
mounted media. The Qt 6.11.2 native-dialog fallback was tested and rejected because
Ctrl-click did not preserve multi-selection.

```sh
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_QPA_PLATFORMTHEME=gtk3 QT_QUICK_CONTROLS_STYLE=Fusion /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_file_dialogs.qml -import /tmp/opencode/qml-mock
```

During the USB workaround review the configured MCP wrapper reported 0.2.0 while
its renderer reported 0.3.0; render failed with `Renderer/wrapper version mismatch`.
`tst_filepicker_capture.qml` provides supplemental actual-consumer QtTest captures,
not a substitute MCP PASS. See REVIEW-USB-2026-09-30.md for scoped evidence.

First-open Attach regression tests now exercise the actual full-window paperclip,
compact overflow selection and profile-photo action without a seeded directory.
QuickView tests exercise its paperclip, cancellation, two-file selection and inert
message.sendFiles routing. `tst_media_open.qml` loads the actual shell media viewer,
checks photo/video navigation and delayed file.open, without starting a video player.
`tst_attach_capture.qml` records supplemental native consumer states at radii 0/8.
Quick-view geometry checks now cover width transitions 350→534→700→350→534 with
font sizes 12/14/15/18 and empty/multiline drafts. Each control's hit area must fit
inside the composer and must not overlap the text viewport. Settled-state images
alone had missed the conditional-anchor transition defect.
