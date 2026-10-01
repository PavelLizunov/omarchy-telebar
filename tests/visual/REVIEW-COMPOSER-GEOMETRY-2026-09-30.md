# Quick composer geometry regression

User screenshot: tools overlapped the top border of the empty inline composer.
Earlier acceptance tested a narrow stacked layout, not directed width transitions.

## Reproduction and correction

Actual QuickConsumer / QuickView test, font 12, transition 350→534: attach hit area
y=-40, height=30, editor height=36. The assertion failed before correction.
Conditional bottom/verticalCenter anchors were replaced by an explicit y binding
owned by composerBox. Minimum inline editor height includes tools.implicitHeight
plus scaled padding. The stacked layout retains separate space below text.

Command:

```sh
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_quick.qml -import /tmp/opencode/qml-mock
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_composer_geometry_capture.qml -import /tmp/opencode/qml-mock
```

Quick tests: 9 passes including lifecycle hooks. Geometry covers fonts 12/14/15/18,
width transitions 350→534→700→350→534, empty and multiline input: 40 settled checks
of each of four controls. Hit areas stay inside the editor with >=2 units clearance
and do not overlap the text viewport. Existing Attach/send/cancel tests still pass.
Qt DelegateModel warning on folder reset remains; not a warning-free claim.

## Visual evidence

MCP healthcheck: wrapper 0.2.0 / renderer 0.3.0; attempted reviewed-consumer render
failed Renderer/wrapper version mismatch. Required MCP visual acceptance BLOCKED.
Supplemental QtTest captures use actual QuickView, inert models/imports, software,
Qt 6.11.2, environment locale, DPR 1, default fixture palette and font 14.
All six PNGs inspected through the harness. No live desktop capture/theme change.

Under /tmp/opencode:

| PNG | Logical/pixel size | SHA-256 |
| --- | --- | --- |
| omagram-geometry-narrow-r0.png | 350×680 | b3143302f68d3d6d406beb9b53a59157adab698dab1c02bed5df079547c765f9 |
| omagram-geometry-narrow-r8.png | 350×680 | e569bdc92e5282c4a0482ba6475e9f8a974b2b817d73f5e32a357c025705ca6b |
| omagram-geometry-wide-r0.png | 534×680 | bce8baa51d81c28ae6f1d78a6b6be5b31eae48f966c97ea488425dde2192c0ec |
| omagram-geometry-wide-r8.png | 534×680 | b3b01affc7b3006706a09bb00037349bfaf5d7946922954ae3e79276de59543e |
| omagram-geometry-draft-r0.png | 534×680 | 337333b643b00779717d3ef546f3f4a33a224d28d7bd9a99eb38e4e28b6206dc |
| omagram-geometry-draft-r8.png | 534×680 | 0cc06ca600e05970983f055e9756a60cff59b104b3ab83b30f351ac6c0a33982 |

Measured at width 534/font 14: editor 54 high, tools y=9.5, height=35, equal 9.5
clearance above/below. Radius 0/8 retained. Image inspection and native design
self-review PASS for these supplemental states: readable density, aligned controls,
stable theme roles. Anti-slop scoped purpose PASS: functional spacing only, no new
decoration. All-theme contrast, assistive tech and independent review NOT VERIFIED.

Source hashes:

- shell/QuickView.qml: fe588a93399502b45d6582023af9e7838a878e9a9d81c74a7a3a8219ddcb1bcb
- QuickConsumer.qml: b68b2de7e15f1e45a43a2bbcee6a98a5416bef06e2db25c14cc63e7799fec2bc
- FixtureApp.qml: 9cb32b81b18e2c69d308f5df8d599e51f5726da31eca801ce1070f18c21f9142
- tst_quick.qml: 6495bfa3ced830772c3c367951b6febd9eae0d8094e7479dacafd54cec923e85

Installed QuickView matches the candidate byte-for-byte. Validation/diff checks
exit 0. User separately authorized one normal Omarchy shell restart to apply this
fix; command completed, new shell PID 1328735, ping ok. Existing window retained,
hello both accounts ready and showStories=false. Restart is application deployment,
not visual evidence: live panel geometry remains user-observation-required.
