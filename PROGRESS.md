# Telebar development progress and open issues

Updated: 2026-10-07. This is a development checkpoint, not stable-release or full-audit acceptance. The manifest remains `1.2.1-dev.2`; this source checkpoint extends that earlier tagged release without creating a new release/tag.

## Current state

- **Source checked:** current Python/QML implementation, including the corrections below.
- **Installed:** the checked 82-file runtime candidate was applied to the local development installation. Source and installed runtime hashes were compared; the backend was kept alive for the latest UI-only update.
- **Loaded:** the main window loaded the installed candidate and reported a connected heartbeat. The existing shell reported a connected Telebar service after the supported plugin rescan. Both account sessions remained available. These are bounded lifecycle observations, not exhaustive interaction acceptance.
- **User confirmed:** copying an image and pasting it into a browser worked in the reported case.
- **Still pending:** physical acceptance of fragment selection, compact reading, presence/contrast and recording recovery. The owner subsequently reported another failure; its cause is not established (see below).

## Implemented since the previous published checkpoint

### Reading, selection and contrast

- Read-only native Qt text selection in message bodies, captions, translations and rich-post text, including compact QuickView. Drag to select a fragment and press `Ctrl+C`; the main message context menu offers selected-text copying.
- Removed compact mode's six-line clipping limit. Long messages use the existing history scroll viewport.
- Body, sender, timestamp, link and selected-text colors are derived against the actual message surface. Incoming sender names use readable theme accent; compact sender and time labels are separate.
- Online status sits beside the peer name; available last-seen information sits below it. The quick chat list and main avatar dots also expose presence. Current and queued chat updates retain account-scoped status events.
- Telegram privacy/approximate status is preserved. Missing status does not become a fabricated precise timestamp or an unsupported offline assertion. Existing online-expiry semantics are unchanged.
- Native theme font, spacing and radius remain reactive, including radius zero.

### Sending and recording recovery

- Stickers and saved/inline GIFs use the shared send ownership contract. Rejection retains reply metadata; eligible success clears only the unchanged original reply context.
- Main-window video capture retains the original account, chat, topic/thread and reply context. A delayed result cannot clear a newer draft or send. Cancelling during stopping revokes sending; other composer sends are blocked while the video overlay owns capture.
- One in-memory retained recording per account supports explicit retry/discard after preparation failure or confirmed Telegram rejection. Retry retains the original destination and content; there is no automatic resend.
- Timeout, uncertain delivery, server errors and generation change disable retry. Dismissing unknown delivery preserves media that TDLib may still use.
- The main window and QuickView share recovery controls. Retained slots **do not survive backend restart**.

### Browser-compatible image copying

- JPEG/WebP copying produces actual PNG bytes rather than relabelling the original MIME type. Existing export permission, message/file identity and job-admission guards remain.
- Conversion uses existing ffmpeg with bounded input/output, pixel, memory, CPU and wall-time limits and pipe-only protocol access. Existing PNG input is passed through.

## Verification at this checkpoint

- Isolated Python: **291 tests**, zero failures/errors, **two missing-tool skips** on the local host.
- JavaScript: **eight suites passed** in the isolated check.
- Native QML consumers: **248 passes across 19 suites**, zero failures/skips/warnings in the recorded full run.
- Six strict actual-consumer offscreen frames were inspected for the contrast/presence change: narrow main window, group chat, compact/wide QuickView, installed representative palette and light/dark variants, radii 0/8. Renderer: Qt 6.11.2, software/offscreen, DPR 1, `en_US`.
- Minimum measured contrast across covered message/name/time/selection/presence text: **4.548:1**. The previous timestamp implementation reproduced **4.104:1** on the installed palette and **4.456:1** on light. This is not a certificate for every theme, widget or accessibility consumer.
- Pointer drag, fragment clipboard readback, read-only text, links/context menu, wheel scrolling and last-line reachability use actual QML consumers with inert I/O. Presence tests exercise the actual Service/client event flow, including account and queued-update guards.
- Source/staged/installed plugin validation and whitespace checks passed at deployment. GitHub runs the Python/JavaScript contract workflow; native QML visual checks are local, not hosted CI.

Private deployment records, logs, photographs, account data and core dumps are not part of this repository. A later source change requires refreshed affected checks; historical counts are not a blanket future PASS.

## Open issues and acceptance gaps

### Reported repeat failure — open, cause unknown

The owner reported “it crashed again” after the UI update. At the subsequent bounded inspection, the main Telebar window, backend and shared shell were alive; the main window and shell had connected heartbeats, and the backend receiver was alive with no pending requests/jobs reported. No matching Telebar window/backend/shared-shell core dump was identified in the inspected time range. Other process dumps in that range do not establish a Telebar crash.

This does **not** refute the reported symptom: a panel disappearance, rendering/input failure, transient reload or a different process remains possible. The exact failing surface, timestamp, trigger and mechanism are unresolved. Do not mark this fixed or infer a cause from the portal warning. Next diagnosis should correlate the reported surface/action with its exact process and time, then reproduce safely and inspect matching logs/core when available.

### Live warnings and shared reload scope

- The main window logs a host-portal app-ID registration warning.
- Shared-shell logs contain duplicate panel IPC-handler registration warnings, including Telebar, and a Telebar UI timer delay during rescan.
- A plugin rescan reloads shared widgets/panels; an unchanged shell PID does not prove unrelated transient UI survived. The unchanged keepLoaded Telebar Service and daemon were retained for the latest UI-only update.

These warnings were preserved, not treated as a clean-live-log PASS or assigned an unproven crash mechanism.

### Recording, media and lifecycle

- Real camera/microphone capture, device loss, account removal, cancellation and physical retained-recording retry/discard are not exhaustively verified.
- Retained recovery is in memory only; crash/restart persistence and all-account retained-slot diagnostics are not implemented.
- Exact transient drafts, attachments and edit buffers across earlier restarts/reloads were not verified. Current development policy permits affected Telebar lifecycle operations even during in-app work; persistent profiles/accounts/databases remain protected.
- Quick-view audio still uses `ffplay`; output-device following is not established.
- Timeout means unknown delivery, not cancellation or permission to resend.

### Reading, presence and rich content

- Physical installed pointer/keyboard behavior and live presence transitions need user acceptance.
- Online-expiry/privacy behavior follows the existing model; exhaustive live-account semantics are not established.
- Long sender names, every theme/font scale, accessibility readers, GPU/compositor behavior and all unrelated controls are not exhaustively verified.
- The previously reported rich-post case still needs specific installed acceptance. Earlier rich-post/layout and Telegram-link/audio paths remain under review.
- One transcribed requested term remains ambiguous; no feature was invented from it.

### Audit and portability

- Complete nontruncated personal source coverage and independent whole-source/security signoff remain unfinished. Selected contract reviews and test passes do not establish a completed audit.
- Local `qrencode` and `desktop-file-validate` checks are skipped when their tools are missing; `actionlint` is unavailable locally. Hosted tools may differ.
- Fresh-environment native QML portability and lossless phone-first README restructuring remain open.
- Python-to-Rust migration is deferred until correctness/security review. No Rust backend exists; native QML/Quickshell and official TDLib remain the architecture.

## Development operation and next steps

The owner authorizes checked Telebar application, reloads and restarts without repeated approval during development, including while using the app. See [the lifecycle policy](BACKEND-DESIGN.md#development-lifecycle-policy). Prefer the smallest affected scope, preserve persistent data and verify recovery. This is not authority to restart unrelated applications, terminate the compositor/session, reset profiles or publish future changes without a direct request.

1. Diagnose the reported repeat failure with exact surface/action/time evidence.
2. Obtain installed-user acceptance of reading, contrast and near-name presence.
3. Verify physical recording/media recovery and account/device transitions.
4. Continue the semantic/security audit and reproducibility work before considering a backend rewrite or stable release.

Publication of this checkpoint records the current implementation and its gaps; it does not close the items above.
