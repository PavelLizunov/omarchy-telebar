# Five-step compact UI pass

Authorized scope: continue all five displayed steps, including the newly reported
quick-reply defects. User selected keeping the hotkey overlay centered. No theme
switch, desktop capture, service restart, actual send, camera or microphone access.

## Step outcomes

1. Sidebar click/drag and existing narrow controls remain covered by consumer tests.
   QuickView reuses Model.receipt for outgoing read/sent/sending/failed state. Text,
   attachments and recordings stay open after success. Errors retain input. Late
   responses after another chat/account/open generation do not clear the new draft.
   Window/overlay live reloading is NOT VERIFIED; installed source identity is checked.
2. Narrow pass inspected search, account menu, reply/edit, attachments, voice/video
   recording layout, emoji/sticker picker, settings edit/confirmation/connection/
   shortcuts and topic retry. Fixed missing emoji search/overflowing tone controls,
   unbounded account menu, field labels, settings heading/confirmation widths,
   hidden editor retaining Escape, quick editor click interception and long-draft
   scrolling. Other dialogs and every possible interaction are not fully certified.
3. Radius 0/8, fonts 12/15 and light/dark quick-reply palettes exercised without
   modifying the live theme. New receipt text contrast measured against composed
   outgoing fills and asserted >=4.5 for the two fixture palettes. Arbitrary theme
   contrast, hardware/GPU masks and assistive technology remain NOT VERIFIED.
4. The full bar consumer remains BLOCKED by the existing offscreen adapter's missing
   Quickshell.Hyprland plugin. The actual menu content is now BarMenu.qml, consumed
   by both BarWidget.qml and the fixture. Ten mouse/keyboard action activations and
   radius changes pass. PopupCard placement/focus/lifecycle remain NOT VERIFIED.
5. Existing diagnostics inspected read-only. No backend changes were justified by
   this observation alone. In the sampled last-hour window, daemon slow-handler/
   stall/missing-heartbeat events were absent, but UI outliers remain. Application
   installed files are compared byte-for-byte to reviewed sources; no restart.

## Verification

The following foreground commands use synthetic models and inert imports:

```sh
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_quick.qml -import /tmp/opencode/qml-mock
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_consumers.qml -import /tmp/opencode/qml-mock
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_sidebar.qml -import /tmp/opencode/qml-mock
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_bar_menu.qml -import /tmp/opencode/qml-mock
node tests/model-test.js
node tests/keymap-test.js
node tests/ui-batch-test.js
omarchy plugin validate .
git diff --check
```

Observed: quick 7 passes, consumers 9 passes, sidebar 5 passes, bar content 3 passes
(counts include lifecycle hooks); model 56, keymap 10, batching correctness passes.
Synthetic 435 updates measured 1341.51 ms sequential / 4.94 ms batched in that run,
not live desktop performance. No code independence claim: direct self-review only.

## Render / image inspection / native design / anti-slop

MCP healthcheck previously established 0.2.0, Qt 6.11.2, software/offscreen.
Explicit ru_RU locale, importPaths=/tmp/opencode/qml-mock, readyProperty=ready,
dependency hashes and DPR 1 (light narrow receipt capture DPR 2) were supplied.
PNG attachments were inspected. Final relevant captures:

| PNG under /tmp/opencode | Logical size / DPR | State |
| --- | --- | --- |
| omagram-quick-receipts-dark-final.png | 500×650 / 1 | radius 0, font 15, outgoing states |
| omagram-quick-receipts-light-final.png | 350×500 / 2 | radius 8, font 15, light palette |
| omagram-quick-attachments-final.png | 350×500 / 1 | caption/files |
| omagram-quick-video-final.png | 350×500 / 1 | inert video recording |
| omagram-pass-final-emoji-rounded.png | 552×500 / 1 | radius 8, visible search/tones |
| omagram-pass-emoji-v2.png | 552×500 / 1 | radius 0, search/tones |
| omagram-pass-final-stickers.png | 552×500 / 1 | empty sticker state |
| omagram-pass-final-reply.png | 552×500 / 1 | reply bar |
| omagram-pass-settings-final-confirm.png | 420×500 / 1 | wrapped confirmation |
| omagram-pass-settings-edit-v3.png | 420×500 / 1 | profile editor |
| omagram-pass-shortcuts-v3.png | 420×500 / 1 | shortcuts navigation |
| omagram-pass-accounts-ten-v2.png | 350×500 / 1 | bounded scrolling account menu |
| omagram-bar-menu-diagnose.png | 300×250 / 1 | actual five-item menu content |
| omagram-pass-bar-final-rounded.png | 300×250 / 1 | menu content, radius 8 |

Rendering PASS for listed states, image inspection PASS. Native design PASS for
scoped fit, feedback, stable surfaces and reactive geometry; no web design imported.
Anti-slop PASS for specific action labels, existing icon/palette roles, no decoration
added. Full screen-reader, all-theme WCAG or independent acceptance: NOT VERIFIED.
Some snapshots are deliberately scrolled with partial edge rows; that is not overlap.
Emoji loading and sticker-empty captures only establish those states.

Final installed/source SHA-256 matches:

| File | SHA-256 |
| --- | --- |
| shell/QuickView.qml | dbfee6ce9473ee6dd848985156101b3db0059070369bf650729842503beff662 |
| shell/BarMenu.qml | b3c8279ab00f0a5fc2f942bb0e0a19df2c4da3a9c2004642a862e0ee8ae0db9d |
| shell/BarWidget.qml | 49dcb29216b35d213e433f0ee023b450c5339a40a9d79b9116ade50407da1e35 |
| app/ChatView.qml | dba0e3ef49611a1f9fb86c73db842486ede7f2596e5b59dd7e201c5cea31cedb |
| app/ChatList.qml | 072fcbe2e4e104232d649642d031615aacc0104e4ad78fa7330e7a1f7ff36dc2 |
| app/Field.qml | d2f44aec1f2c277e7d6da918da6e769f0fb36fe7bf4c887cb8cce768d84fa2a7 |
| app/SettingsView.qml | 7c7b012e13395f7a4e0001406ad8d3413591db03262bdf55a0dea04fb730ea8e |
| app/EmojiPanel.qml | 193258f63777aeb74ab9ca419bae0ce269ab989fb0bd98e8b15409509f190192 |

Final receipt images SHA-256: dark
`e02dc862d8946eb7c96d5dbb494fd5917a0e9cdfb4b550ff4877ff51eee64022`,
light `46a74a31a3433a3392513b4d21b7454175ae4c588e7a34ca46566e871423001d`.
MCP hashes identify listed dependencies only; unlisted transitive imports are not
claimed fully enumerated. No independent review, commit or publication.

## Diagnostics observation

Read-only parser examined 25,601 JSONL records, zero malformed, retained rotations.
At observation ending 2026-09-30 14:30:33 UTC, the preceding hour had zero daemon
slow_ipc_handler, slow_td_event, main_loop_stall or ui_heartbeat_missing events.
Subsequent surface-aware aggregation of roughly that hour: window timer max 173 ms
(p95 2), callback handler max 1015 ms (p95 1), frame-tick max about 1021 ms; shell
timer max 4755 ms (p95 80), handler max 162 ms (p95 1). These are UI diagnostic
counters, not compositor/GPU presentation timing and not evidence for the new code.
Recorded hello IPC responses across retained files max 40 ms (11 responses).

Outliers: shell timer 4755 ms at 13:38:58 UTC, 3117 ms at 14:11:43; window handler
1015 ms at 14:13:16, 405 ms at 14:13:21. No owning handler name is recorded by these
aggregate samples, so blaming a specific component or declaring the cause solved
would be unsupported. Next performance work needs an isolated handler attribution
measurement rather than speculative batching changes.
