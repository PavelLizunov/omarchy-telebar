# Theme geometry and sidebar handle

Scope: original sidebar control in Main.qml, extracted SidebarHandle.qml consumer,
message surfaces in MessageRow.qml/QuickView.qml, silent-send chip, bar menu row.
AGENTS.md records the project-owned theme/interaction contract for future changes.

Native direction: calm utility client, energy/rhythm/motion 1/1/1. The icon is smaller
than its pointer target; the background is solid beneath the native translucent hover
role. The divider is interrupted behind the button. All changed surface radii bind
to Style.cornerRadius. Semantic avatars, round video, rings and dots remain circular.

## Interaction evidence

Executed offscreen, software, existing inert imports:

```sh
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_sidebar.qml -import /tmp/opencode/qml-mock
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_consumers.qml -import /tmp/opencode/qml-mock
node tests/icons-test.js
omarchy plugin validate .
git diff --check
```

Results: 5 sidebar/theme passes and 7 existing consumer passes (including lifecycle
hooks), 53 icon drawings, validation and diff checks exit 0. Tests cover actual mouse
press/move/release on the icon and rail, bounds, click after drag, keyboard toggle and
resize, radius 0 → 8 → 0 without rebuilding surfaces, opaque hover backing/pixels.
Earlier test failures were corrected before this result. No live service restart.

## Render and image inspection

Healthcheck: renderer/tool 0.2.0, Qt 6.11.2, offscreen/software, DPR 1 and 2 supported.
All images use explicit locale ru_RU, readiness property ready, inert models and
`/tmp/opencode/qml-mock` imports. Default synthetic dark palette; no live theme change.
Final PNGs were inspected in the harness; native hierarchy/spacing fit the shown
layouts and there is no divider through the handle's face.

| Image under /tmp/opencode | Logical / pixels | State | SHA-256 |
| --- | --- | --- | --- |
| omagram-sidebar-square-verified.png | 900×650 / same, DPR 1 | radius 0, focused, expanded | 7052d8cd47be680d3397d7a1adcfc8fc40711434958ad142233d9e9d920d4759 |
| omagram-sidebar-rounded-verified.png | 552×760 / 1104×1520, DPR 2 | radius 8, font 15, focused, compact | 4cc10de29e9f140ca7f8d407c794ea49391b873bc980a250d626a6214cca65ef |
| omagram-quick-square-final2.png | 500×600 / same, DPR 1 | radius 0, loaded quick reply | dabab1682cb5a045aee98f3b465ec4f06cf9c93cce6e5f57ee5d9c18126b41ce |
| omagram-quick-rounded-final2.png | 500×600 / same, DPR 1 | radius 8, loaded quick reply | a2ffc435ff8d060b687a42725cf80eb9dfa8b95aa0c90ebbc4c2e433dfc3af24 |

Measured baseline: icon 16×16, button 28×28, grip target 32×32, rail target 12 wide,
visible divider 1 wide. At font 15: 20×20 / 35×35 / 40×40. These are logical QML units,
not CSS or physical pixels. The 16:28:32 choice is a project sizing decision, not a
scientifically prescribed ratio. Relevant references:

- https://doc.qt.io/qt-6/qml-qtquick-controls-splitview.html — visual versus input size.
- https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html — web target
  size/spacing guidance; 24 CSS pixels cannot directly certify native QML sizes.
- https://www.w3.org/WAI/WCAG22/Understanding/dragging-movements.html — alternative
  pointer operations. Keyboard resizing is tested; a full click-only arbitrary-width
  alternative has not been implemented or certified.

Key dependency identities; MCP checked listed hashes before/after each render:

| Source | SHA-256 |
| --- | --- |
| app/SidebarHandle.qml | d4447d2fb0db6277d62753d09416a2ad68f63a8ad122e7a3509476b86fc6cc08 |
| app/Main.qml | 5a42581badc5c01d10d5e6666718f8a2e81501775c80c7d04542367463680721 |
| app/MessageRow.qml | 5beaa161245c5c2c6936e850357a5909301bfab207ba3cd3c16255898cb58b65 |
| app/ChatView.qml | 7ff76490783b0e30a0814cc95d03a84e3cfea7a85f7381e3e6e99bba600a8987 |
| shell/QuickView.qml | ca330177b5b8f530c62b690ca71246110c49f878de6f15dbe99379cd6e154a7e |
| shell/BarWidget.qml | f3c74e13c936fa57e2f59ff752a062cb04a2016bb789f976ebe015c3472fd92b |
| tests/visual/Sidebar.qml | aabf96b41641d8eee799dbbc2737e16b3320560486d0397f37428a3291629965 |
| tests/visual/Preview.qml | 62e431d86a8f08037e6be0a9ed8980908fc16a506d38e630791cd43c579d3e4c |
| tests/visual/FixtureApp.qml | 0cc8fc5b82c279f6861d6ad57907ffe430c9b49039705548fb65fe7d09aa5870 |
| inert qs/Commons/Style.qml | f5901092617ae62645597199270afd4dfb0644d596bdfff16e1834fca6ad7243 |
| renderer | 1cd55ee447bded3da8b3b7156861816b55b5acd6502a141f950136f9317e6348 |

Installed task-owned QML files match the worktree byte-for-byte. Existing unrelated
installed README sections were not overwritten; task-owned documentation was added.
No commit/push was made.

## Bounded acceptance

- Rendering: PASS for four final states; image inspection: PASS.
- Native design/self-review: PASS for measured handle and theme surfaces.
- Anti-slop: PASS for scoped purpose, specific copy and existing native tokens.
  Contrast across all themes and assistive technology: NOT VERIFIED.
- Interaction: PASS for listed inert consumer paths; not independent signoff.
- Bar consumer: BLOCKED by missing Quickshell.Hyprland plugin in the existing
  offscreen imports. Its radius source fix is present; no visual PASS claimed.
- Live reload/compositor placement, GPU masking/media, all theme combinations:
  NOT VERIFIED. Files installed without restarting desktop or application.
