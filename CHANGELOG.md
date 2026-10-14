# Changes

## Compact emoji layout (2026-10-14)

- Reserve a usable chat viewport while emoji are open in a small window.
- Let the picker shrink and cap a long editor only while emoji are open; keep
  the caret visible when the editor shrinks.
- Scroll category/tone controls instead of wrapping them over the emoji grid,
  and provide a pointer/keyboard close action without discarding the draft.

## Round video enlargement (2026-10-14)

- Open original round videos full-screen from the window's expand control or the
  quick-view row, without an external Telegram client or player.
- Share native video decoding, circular crop, 1–4× zoom, bounded pan, pause,
  reset and close controls. Keep window inline playback separate.
- Await existing quick playback retirement; ignore refused or stale responses.
- Cover actual Main/Quick parent routes, original decoded/rendered frames,
  pointer/keyboard zoom and media close/download contracts. This does not settle
  the separate reported outgoing-video playback incident or perceived quality.

## Native main cleanup (2026-10-14)

- Publish the checked Rust implementation and current native UI on main.
- Remove retired interpreter code, comparison drivers, dependent test wrappers
  and obsolete guidance from the current source tree. Prior checkpoints remain
  in Git history; no tags or release assets are rewritten.
- Replace UI-generation and actual-consumer preview tooling with Node.js.
- Replay the frozen 21,136-plus-18-input projection corpora directly in Rust.
- Use native/JavaScript CI and actual packaged ELF/socket/helper checks.
- Preserve application behavior, accounts, licenses, native safety guards and
  the separately installed Qt recovery. No backend/shared-shell restart.

## 1.3.0-rc.1 (2026-10-14)

Native runtime prerelease with compatible launcher/daemon/helper entry points,
original paper-plane mark, narrow-window sidebar expansion, actionable compact
window opener and content-bound UI packaging. The user confirmed window opening.

[Release and native archive](https://github.com/PavelLizunov/omarchy-telebar/releases/tag/v1.3.0-rc.1)
retains its original source/acceptance scope. This main cleanup does not move its
tag or replace its files. New main is source-only until built and packaged.

## Remaining limits

The reported outgoing sent-video playback P1, forum-parent unread mismatch and
historical window disappearance remain unresolved. Full 170-command working-account
coverage, perceived media quality, working network/reverse rollback, clean-stock
portability and independent security review are incomplete. Runtime diagnostics
retain `migration_complete=false`. Source publication is not stable certification.
