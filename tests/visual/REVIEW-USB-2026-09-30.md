# USB crash workaround and story visibility

## Facts

Crash: Quickshell PID 1074884, SIGSEGV, 2026-09-30 19:28:59 MSK, command
`quickshell --no-duplicate --path /run/user/1000/omagram/app`. coredumpctl confirms
a stored core. Report: Quickshell 0.3.1, Qt 6.11.2, GTK 3.24.52, GLib 2.88.3,
jemalloc 5.3.1; gtk3 platform theme. Kernel exFAT event at 19:28:58.950554 precedes
crash registration at 19:28:59.280115 by 0.329561 seconds. No OOM entries in the
examined 19:20–19:35 kernel interval. One Quickshell core on this date in the listing.

Symbolized stack: GVfs drive-changed → GtkPlacesSidebar update_places → add_place
for Documents → g_signal_connect_data → g_bsearch_array_grow → g_realloc → jemalloc.
The raised libc signal frames belong to crash handling, not the original allocation
site. The originating corruption/ownership defect is not established. No core was
extracted, copied or published during this task. Existing reports contain private
paths; no upstream publication was performed.

## Adopted implementation

Verdict: Compose existing Qt.labs.folderlistmodel, project Button/Icon/WheelScroll
and native Style tokens into one shared FilePicker. The two original FileDialogs
were replaced. No external chooser or new package dependency. User explicitly
rejected Zenity. DontUseNativeDialog was tested first: Qt 6.11.2 fallback discarded
the first selection after Ctrl-clicking a second file; upstream versioned source
confirms single selectedFile handling. That approach was not shipped.

Picker navigation is local/read-only, file names are PlainText, selected URLs route
through existing URL conversion and backend local_upload validation. Backend still
rejects nonregular/unreadable/empty/oversized files and all account database paths.
No shell interpolation, automatic upload, mount/unmount or deletion was added.
Own UI surfaces inherit Style.cornerRadius; semantic icon shapes are retained.
Direct differential security self-review found no new demonstrated trust-boundary
bypass in this scope; not an independent security certification.

Story setting: showStories boolean, default true, strict validation, merged partial
settings preserve shortcuts/global keys and other preferences. Main.storyChats is
empty and openStories returns when disabled; an open story is finished. This is a
local visibility preference, not a ban on TDLib story metadata synchronization.

## Verification

- QtTest actual-consumer picker: 6 passes including lifecycle hooks. Tested with
  QT_QPA_PLATFORM=offscreen, QT_QUICK_BACKEND=software, QT_QPA_PLATFORMTHEME=gtk3,
  QT_QUICK_CONTROLS_STYLE=Fusion, existing inert imports. Two file mouse selection,
  accept routing, photo filtering/inert profile.setPhoto, Escape cancel, reopen,
  path validation and Up, radius 0→8→0. Qt emits a DelegateModel::cancel index warning
  when a scrolled photo model clears on closing; acceptance/path tests pass. Not
  presented as warning-free runtime or USB recurrence proof.
- Consumer tests: 10 passes, including story setting toggle.
- Python settings: 12 passes; daemon: 105 passes, including story persistence and
  partial-settings preservation; stories-ui JS passes; 54 icon drawings pass.
- `git diff --check`, `omarchy plugin validate .`: exit 0.
- Physical USB reconnect after dialog close: NOT VERIFIED. No mounted-media changes.

MCP rendering: BLOCKED. Wrapper 0.2.0 / renderer 0.3.0, render returns
Renderer/wrapper version mismatch. Tool config/binaries were not changed by this task.
Supplemental QtTest actual-consumer PNGs were read via the harness, software/DPR 1:

| PNG in /tmp/opencode | Logical/pixels | State | SHA-256 |
| --- | --- | --- | --- |
| omagram-filepicker-radius-0.png | 900×650 | attachment picker, radius 0 | 079cbcbe2e387a1f18453738e723f1ffc2a74da731ae6bc17fa37d9dc7f31fbe |
| omagram-filepicker-radius-8.png | 900×650 | attachment picker, radius 8 | 275767d9592544cf9b4179682a5176dd51e5188da2f62130da1faad8a7097ab9 |
| omagram-filepicker-photo.png | 444×600 | narrow photo empty state | 7826e660aebd07b70a28c5383f196fcbd06eef0a155b5ce659f5174b8d08cbd3 |
| omagram-stories-hidden.png | 552×650 | stories setting Hidden | ca48934269516b49d5efaed3793f24740ea9fddea83705e85782b31c5c171389 |

QtTest locale was environment default (not explicitly controlled). Font 12 for picker,
15 for settings; synthetic dark fixture palette. Native design/anti-slop direct
review: surfaces fit shown widths, path clips internally, explicit confirm/cancel,
no decoration or new design system. Image inspection PASS for supplemental captures;
required MCP cycle remains BLOCKED. All-theme contrast/accessibility consumer/GPU
and production dialog interaction remain NOT VERIFIED.

Candidate SHA-256 (installed task files checked byte-for-byte):

| Source | SHA-256 |
| --- | --- |
| app/FilePicker.qml | 79f4b3b1b7d2f7b2194f424cb14b45fa77f101e78daf59dbc3d6494353b703df |
| app/ChatView.qml | dd020ef1d8e9f32aae936d5aad62e2e8de8da41847311c3ac4307b0df0bbfa80 |
| app/SettingsView.qml | 21a2fe209b92cb7d8bc64f1137fa536bb0c797c8ad19b579aefdd6e4ebe4f8bb |
| app/Main.qml | 6ed23c0c267b08c8212a59991acd5ef489b9c9e4f14a7fc2fb08a38af668b18a |
| app/Icons.js | 55320d72035fce64120b562bfdc745a17c267f3c2a6f0ebd135784abb56ad0a2 |
| bin/omagram_settings.py | 18bfe6076762ac0de03ea30a2ca0e3cbee93e81826f6a913df41456646e5d8fe |
| bin/omagramd | 6ca225f0b970c9866e9b161e6069043aec454a6418c3727240797213f01cff7c |

## Application result

User authorized one Omagram-only restart. Graceful app.quit acknowledged; original
window/daemon ended. showStories=false saved with all other preferences compared
unchanged. Started managed omagram-usb-workaround.service, MainPID 1173651. Startup
reports Configuration Loaded; hello confirms showStories=false and both accounts
ready. No desktop/shell/theme restart. No account/logout/database removal.

Telegram database loss was not demonstrated by this UI crash; retained authorizations
are observed after restart. An unsaved UI-only draft may have been lost at the crash;
not recoverability-certified. Kernel separately warned the USB exFAT volume had not
been cleanly unmounted; this is not proof Omagram damaged it, no fsck was performed.

Bug ownership: native GTK/Qt/GVfs/Quickshell integration is implicated, allocator
ownership remains unresolved. Not a confirmed Omarchy core bug. Omagram now avoids
the demonstrated GTK chooser path; physical regression is still required.
