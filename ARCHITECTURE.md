# Native ownership

Telebar has one Rust application owner, native QML/JavaScript consumers and official
TDLib's four-symbol JSON interface. The generated package has five compatible Bash
entry shims. The source checkout is built before installation.

## Runtime

- `rust/src/main.rs`, `session.rs`, `launcher.rs`, `host_service.rs` and
  `socket_owner.rs`: bounded IPC loop, explicit session context, native window/
  backend starts through the user manager, exact process/job and lock ownership.
- `application.rs`, `native_driver.rs`, `engine.rs`, `authorization.rs` and
  `lifecycle.rs`: account-scoped requests, ordered responses/events, deadlines,
  worker completion and native client close. Uncertain mutation is not retried.
- `td.rs`, `keyring.rs`, `storage.rs` and `paths.rs`: copied JSON-ABI buffers,
  session keyring, anchored owned files and compatible account/database identities.
- `capture.rs`, `recording_jobs.rs`, `recording_checkpoint.rs`, `upload_lease.rs`,
  `files.rs`, `playback.rs` and `sound_playback.rs`: bounded native helpers,
  retained media/checkpoints, exact inode cleanup and playback input lifetime.
- `proxy.rs`, `proxy_crypto.rs` and `bridge.rs`: adapted Flowseal transport,
  OpenSSL/TLS, bounded relay and owner-lifetime pipe; fixed Telegram destinations.
- `package.rs`: allowlisted native assembly, hashes, licenses, executable and shims.

## Interface

`app/TelebarClient.qml` owns the compatible socket consumer and callbacks.
`app/Main.qml` composes the separate application window. Extracted `Main*.js`,
`Chat*.js` and `Settings*.js` own the existing UI actions. `Model.js` owns shared
projection/layout logic; `SidebarHandle.qml` owns click, keyboard and drag resize.

`shell/TelebarService.qml` connects the shared shell to the independent backend.
`BarWidget.qml`, `Panel.qml`, `TelebarQuickView.qml` and `TelebarQuickReply.qml`
compose the mark, compact panel and overlay. The real parent handles window
opening; pending composition remains owned by its current surface. A retained
QML service and cached same-URL types are separate from backend readiness.

`scripts/native-ui-generation.mjs` gives the checked visual closure content-bound
URLs without moving the backend executable. `scripts/prepare-preview.mjs` builds
actual-consumer offscreen fixtures with explicitly inert I/O and window boundaries.
Neither tool installs or reloads anything. Node.js is build/test tooling only.

## Verification boundary

[Runtime contract](rust/PARITY.md), [native checks](rust/README.md) and
[visual checks](tests/visual/README.md) identify current invariants and limits.
Native defaults, frozen corpora, QML fixtures, package bytes, installed ELF and
physical acceptance are distinct. Existing account readiness is not exhaustive
command, media-quality, network, reverse-rollback or independent security proof.
Historical checkpoints remain recoverable from Git history, not current guidance.
