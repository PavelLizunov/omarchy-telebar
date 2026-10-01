# Attach opening and media regression pass

Scope: FilePicker first-open root cause, compact QuickView picker entrypoint,
constrained geometry and MediaViewer local icon imports. Full codebase audit is
planned separately in AUDIT_PLAN.md; user clarified Ponytail simplicity review.

## Observed defects and corrections

- Production window log reported FilePicker.qml:29 TypeError: Property split of
  object file:///home/slovn is not a function. QtCore StandardPaths returns a URL.
  Earlier tests seeded currentFolder and never exercised this first-open branch.
  New actual paperclip test failed before correction with the same error and picker
  visible=false. The shared function now passes the URL directly to navigate.
- QuickView had clipboard-file sending but no paperclip/file chooser. It now reuses
  FilePicker; selection stages attachments, Enter invokes existing sendFiles. Picker
  closes on account/chat reset and leaving the view. No external helper dependency.
- Narrow quick controls retain editor width by putting the tool row below it.
- First inspected compact picker capture overflowed horizontally. Layout minimum
  widths and bounded button widths corrected it; child bounds are asserted in tests.
- Shell.MediaViewer referenced Icon without importing the actual app component.
  Explicit App.Icon imports added; actual viewer construction, photo/video step,
  ready and delayed file.open routing and Escape are exercised.

## Verification and visual evidence

Foreground offscreen/software qmltestrunner, existing inert /tmp/opencode/qml-mock:

```sh
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_file_dialogs.qml -import /tmp/opencode/qml-mock
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_quick.qml -import /tmp/opencode/qml-mock
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_media_open.qml -import /tmp/opencode/qml-mock
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_attach_capture.qml -import /tmp/opencode/qml-mock
git diff --check
omarchy plugin validate .
```

Results: 8 file-picker passes, 8 quick-view passes, 3 media passes (including lifecycle
hooks), capture 3 passes. Qt DelegateModel cancel index warnings occur during folder
changes/clearing; no warning-free claim. Actual network send, player playback, camera
and live surface/compositor interaction are NOT VERIFIED.

MCP healthcheck succeeds but rendering remains BLOCKED by wrapper 0.2.0 / renderer
0.3.0 mismatch. No tool installation or desktop capture used. Supplemental QtTest
actual-consumer captures at DPR 1, environment locale, Qt 6.11.2/software, synthetic
dark palette and font 15 (quick) / 12 (media), were inspected through the harness:

| PNG under /tmp/opencode | Logical/pixel size | State | SHA-256 |
| --- | --- | --- | --- |
| omagram-quick-attach-picker-0.png | 350×500 | square picker | 5537329bb9156aa8188e2df69c3d8f2c03b780cc5fcf6bbba95b43fb58e11176 |
| omagram-quick-attach-picker-8.png | 350×500 | rounded picker | 869431040dfcc22a8d430980eb6311871708da1d1ec15343e7e929eca2bd5957 |
| omagram-full-attach-fixed.png | 900×650 | full window picker | eb348058f58d72f530799ccdb91428ca7d845f3aa943330f4c5a63d56837ca04 |
| omagram-media-open-fixed.png | 900×650 | photo in real shell viewer | 28280287bd341b25dd1260294722563c893e6112803616d24d2be0884db77997 |

Relevant installed/worktree source hashes match:

- app/FilePicker.qml: 43342bc3060154718dc859bb29a1d372ff89128e505f9087df91beb4a4876a6b
- shell/QuickView.qml: 6bf146abb8f8e0f7450f394f5622c8eb2e1e3f8ef55a62e3cfc3a0160c4b3669
- shell/MediaViewer.qml: a2102a46a7cbbe6a7b42008dc26a817e8992b6f7b3f19b83e904f31f6c4438b3

QuickView urgent role was added to its picker palette adapter after these captures;
normal-state appearance is unaffected. Exact current source hash above identifies
that adapter correction. Transitive imports not all enumerated.

Native design/self-review: fit and hierarchy PASS for inspected supplemental states;
anti-slop scoped purpose/tokens PASS, no decorative system added. MCP cycle BLOCKED.
Independence, all-theme contrast and assistive technology not verified.

## Application actions and limits

User authorized one Omagram restart and separately one global plugin rescan. Window
and daemon initially started with both accounts ready and showStories=false. After
authorized rescan the daemon socket disappeared; shell ping stayed ok. No claim of
successful plugin lifecycle reload. Restored owned managed units:

- omagram-daemon-recovery.service, PID 1260577;
- omagram-attach-window.service, PID 1261333.

Both active at final check, window Configuration Loaded, hello both accounts ready,
showStories=false. No desktop restart, account logout, live theme change, commit or
push. Compact source was reloaded through the host rescan; real compact click/send
has not been observed. Lifecycle reliability is a high-priority audit seed.
