# Narrow chat review

Scope: ChatView.qml and MessageRow.qml in the dirty main worktree. Native Omarchy
tokens and original icons are preserved. Design read: utility chat client, calm native
layout; energy/rhythm/motion 1/1/1. Secondary actions move into existing ContextMenu
so the editor keeps typing space. Narrow message bubbles use the available width.

## Observed checks

- Red: installed pre-fix ChatView, width 462, Style.fontBaseSize 15: text viewport
  was 12 units; isolated acceptance assertion requiring 140 failed.
- Green: `QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_consumers.qml -import /tmp/opencode/qml-mock`:
  7 passes. Widths 330/462/820/462 retain the draft and at least 140 units of
  editor space. Send click produces an inert message.send request with the draft.
  Overflow keyboard selection opens stickers, Escape restores editor focus,
  header Space/Return opens info, narrow info fills the chat viewport.
- `node tests/icons-test.js`: 53 drawings pass.
- `git diff --check` and `omarchy plugin validate .`: exit 0.
- Installed ChatView.qml and MessageRow.qml match the reviewed worktree byte for
  byte. The same isolated installed-consumer assertion now passes with a 300-unit
  text viewport (3 passes including lifecycle hooks). The active application was
  not restarted; evidence establishes the installed files, not live reload.

## Render and inspection

MCP legacy 1.0.0, Qt 6.11.2, Omarchy 4.0.4, software backend, DPR 1, fixed 300 ms
delay. Healthcheck call returns Tool not found. Locale/readiness/measureObjects
arguments are unavailable; fixtures expose ready and use inert adapter imports.
Narrow fixture: explicit font size 15, monospace, synthetic Nord-like palette;
wide fixture: existing default palette, font size 12. Locale is renderer default,
not explicitly controlled. All following final images were read through the harness.

| Image under /tmp/opencode/ | Size | State | SHA-256 |
| --- | --- | --- | --- |
| omagram-narrow-final.png | 552×1020 | empty editor | 9f5bbb18f02d6e21f0b55687ac6504765fa7a5a0be431c9dbd6bd7626ff5727f |
| omagram-narrow-draft-final.png | 552×1020 | multiline draft/Send | ada00098c44e7089e42a62e7f65f6df67355be52748210838f969a4810574683 |
| omagram-narrow-menu-final.png | 552×600 | all message actions | 80dca5da08b615bc67b6efc9e4d37cca029fd9f36296b0cd3daea91d6f6c39d1 |
| omagram-narrow-info-final.png | 552×1020 | information overlay | 904d8507b3bfb0f9f4a9627b84e5a5da6cda5f703f8ef3cda2bfb28f8788775e |
| omagram-narrow-prompt-final.png | 420×650 | wrapped confirmation | 46979d4806aae1445e9ee337e891038d720244b0484d161d27683a75e6635b34 |
| omagram-narrow-selection-final.png | 420×650 | wrapping actions | bed9298e2f3fdac8b8807dc7e87d8dd7ade3dd814e7b3ee4aa71117e63df6a94 |
| omagram-narrow-attachments-final.png | 552×600 | caption/files | f764b9feb6c251e8a64a724e150c7c438ef5d1cc032473bc9f675c620c994b7c |
| omagram-narrow-recording-final.png | 552×600 | inert recording | 92b84b95759f30a52def29d85ef1c9414b7de78564c1be20a7c0b33ce58240eb |
| omagram-wide-final.png | 1100×760 | wide chat | 7eeea668e848f4473e55990f0295787d62922322a5717a3954f3d57d4431b14d |

Key dependency hashes (additional transitive host/assets are not fully enumerated):

| Dependency | SHA-256 |
| --- | --- |
| app/ChatView.qml | 2fb0701d074cb95cac5ad62c591a3b46136e841a95651c45aac664d7d85afedb |
| app/MessageRow.qml | 9332e41ece51cb3642793e6c03fa8c3166b07a50799986007ac5b561c65f72bf |
| tests/visual/FixtureApp.qml | 0b26a395b3ccd4548cbbf08a6f5e8a37fcbfb056a1b099f9e239e79c95e28b92 |
| tests/visual/Narrow.qml | c4c4a939fcf37d522284dae426292429192bc100a4e49fb4457a0d7645d1c8f5 |
| tests/visual/tst_consumers.qml | a04af8728fde60df201543ca1c63afadf7500cdab8d59c8c94f72b0685f1084c |
| /tmp/opencode/qml-mock/qs/Commons/Style.qml | f5901092617ae62645597199270afd4dfb0644d596bdfff16e1834fca6ad7243 |

## Bounded verdict

- Render: PASS for listed fixture states; healthcheck capability NOT VERIFIED.
- Image inspection: PASS; editor/actions fit, confirmations wrap, selection flows.
- Native design/self-review: PASS for these constrained layouts. Not independent review.
- Anti-slop purpose/copy: PASS; existing palette/icons, specific action labels, no
  decorative chrome. Full WCAG contrast across user themes NOT VERIFIED.
- Interaction: PASS for exercised resize, send, menu/stickers, Escape and info paths.
  Actual recording, camera, file chooser, network send and every overflow operation
  are NOT VERIFIED; these fixtures do not execute production operations.
- Live compositor/GPU, all themes/locales, other dialogs and bar: NOT VERIFIED.
