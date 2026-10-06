# Checkpoints

## 1.2.1-dev.2 (2026-10-07)

Final checkpoint for this batch; supersedes the preliminary dev.1 tag.
Application and QML fixture bytes are unchanged. Ubuntu hosted checks now stage
input outside the private runner home and use privilege only for namespace setup.
The payload runs with the runner uid/gid and no effective capabilities. The runner
asserts an isolated network and read-only workspace before importing the suite.
Resource, network and filesystem restrictions remain intact.

[Hosted run 37536497815](https://github.com/PavelLizunov/omarchy-telebar/actions/runs/37536497815)
passed on commit `910fbb82d5f674b4543d1fe38ac1944604a8156d`: three identity checks,
279 Python checks (two skipped) and eight JavaScript suites. Final version metadata
is checked again on the published dev.2 commit. Local omissions and live/audit
limitations below remain; hosted success is not installed-client acceptance.
The initial dev.1 and first setup-correction CI failures are retained in Actions.

## 1.2.1-dev.1 (2026-10-07)

Development checkpoint of the current Python/QML implementation. This is not a
stable release, completed repository audit, or acceptance of the installed client.
No Rust backend, TDLib upgrade, account migration or live deployment is included.

### Included corrections

- Require message identity, original-media membership and explicit TDLib export
  permission before saving files or copying viewer photos.
- Retain rejected text/file compositions and edits. Accept completion only for
  the original account, chat, topic/thread and unchanged composition revision.
- Preserve voice and auxiliary-send reply metadata on rejection; successful
  reply-only completion leaves ordinary text, attachments and edits intact.
- Retire stale mention/reaction navigation and delayed edit-Markdown callbacks.
- Surface bounded startup-parameter failures without raw credentials/errors.
- Bound tracked TDLib/IPC requests, expire them independently, protect reused IDs
  and finalize owned resources without applying stale business results.
- Return failures, not successful empty results, after malformed callbacks.
- Reject malformed numeric/opaque media boundaries; preserve valid Unicode text
  and paths. Clipboard cleanup deletes generated output, never selected uploads.
- Enforce recording-finalization capacity before consuming recording ownership;
  allocate video output before interrupting its recorder.
- Include inert regression suites, versioned/licensed QML imports and a
  network-isolated Python/JavaScript GitHub Actions workflow.

### Verification evidence

A fresh export of the staged checkpoint executed 279 Python checks with zero
failures/errors, two missing-tool skips (`qrencode`, `desktop-file-validate`) and
eight JavaScript suites in the reviewed network-isolated, read-only worker.
Checkpoint metadata does not change the checked application or test bytes. The offscreen Qt evidence
contains 164 passes across 17 suites, zero failures/skips/warnings, and sixteen
personally inspected radius-0/8 captures. Rendering, geometry and interaction
assertions are fixture evidence, not live desktop or hardware acceptance.

Selected immutable corrective deltas received bounded advisory reviews. Full
independent source/security review and complete personal source coverage remain
unfinished. Hosted CI must be checked against the exact published commit; a
workflow file or local pass is not hosted acceptance.

### Known limitations

- The installed plugin remains unchanged. The reported rich-post display failure
  is not resolved by publishing this source checkpoint.
- Video-note original-context routing, recording cancellation/account transitions
  and recorded-file retry after Telegram rejection need further audit.
- Sticker/GIF reply handling and other asynchronous sibling consumers still need
  coverage; the main-composer fixes do not certify every send route.
- Live microphone/camera, account/database transitions, quick-view audio output
  following, compositor behavior, exhaustive contrast and accessibility are not
  verified.
- Fresh-environment portability, the complete audit and phone-first README
  restructuring remain outstanding. No performance or Rust benefit is claimed.

### Package and compatibility

The checkpoint package contains QML, Python helpers, assets, manifests and notices.
It does not bundle or rebuild TDLib; the existing pinned TDLib setup remains
required. Account storage, keyring identifiers, runtime names and original licenses
are preserved. Legacy context-free `file.save` callers now fail closed and must
supply `chatId`, `messageId` and `fileId`.

The archive is a source/plugin package, not a standalone compiled executable.
Creating it does not install, enable or restart the plugin. Do not run Telebar and
Omagram together against the same profile.
