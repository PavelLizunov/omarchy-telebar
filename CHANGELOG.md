# Changes

## 1.3.0-rc.4 (2026-10-25)

Follow-up to the requested implementation/security review. The rc.3 release
and its assets remain unchanged.

- Fix the same hidden-source repaint defect in the recording preview. Synthetic
  native Radeon checks reproduce zero natural swaps before the repair and
  sustained presentation after it; hardware capture acceptance remains separate.
- Pin every upload path ancestor without following replacement symlinks. A
  directory swap after canonicalization could otherwise bypass the database
  exclusion. A deterministic synthetic interleaving fails before and passes
  after the repair; this is not a claim of observed real-world exploitation.
- Retain ordinary chosen-file and resolved-symlink behavior, plus the existing
  descriptor/inode checks. Add a bounded concurrent directory-swap regression.

[Release and native archive](https://github.com/PavelLizunov/omarchy-telebar/releases/tag/v1.3.0-rc.4)
contain the follow-up fixes. The scope is rendering and upload admission, not
whole-client security certification or physical device acceptance.

## 1.3.0-rc.3 (2026-10-25)

- Fix inline round videos that decode without naturally repainting: keep the
  video source in the scene graph and apply the circle through its layer effect.
- Package one content-bound UI generation for service, bar and overlay; verify
  all selected visual files, not only the base app directory.
- Require natural presentation before image grabs in playback regressions;
  image capture itself could previously hide stalled rendering.
- Retain the direct-output original-video viewer, zoom, pan, pause and reopen.

[Release and native archive](https://github.com/PavelLizunov/omarchy-telebar/releases/tag/v1.3.0-rc.3)
provide the prebuilt x86-64 plugin and checksums. Prior scoped checks include
native VM and Radeon playback, old/fixed controls, changing painted frames and
virtual audio. Original private-clip and physical audio/synchronization acceptance
remain pending. This is a prerelease, not whole-client or security certification.
Existing release tags and assets remain unchanged.

## 1.3.0-rc.2 (2026-10-14)

Corrected native prerelease with the compact emoji layout fix, confirmed working
by the user. Includes the previously published original round-video viewer and
native-only source/tooling cleanup. The application remains Rust/QML/JavaScript.

[Release and native archive](https://github.com/PavelLizunov/omarchy-telebar/releases/tag/v1.3.0-rc.2)
provide the updated x86-64 package and checksums. Existing `v1.3.0-rc.1` assets
and tag remain unchanged. No additional tests were run for this publication at
the user's request; prior scoped checks and user acceptance are retained.

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
