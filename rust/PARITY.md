# Rust replacement contract

Status: migration in progress. Rust is the selected final application runtime.
The Python application still serves production until the complete replacement
passes acceptance. A partial Rust implementation must not open real accounts.
Native QML/JavaScript/Quickshell and official TDLib remain unchanged.

## Baseline and ownership

The baseline is the complete dirty working tree, not just Git HEAD
`e194d6b90bc45e48099866e770ca9fc4d0454152`. Existing stabilization,
renamed components and untracked runtime modules are preserved. An external
immutable source snapshot, index and binary diffs were saved before editing.
The registry has 170 commands in 55 name families. There are 189 static QML/JS
request call sites using 142 distinct commands; none names an unknown command.
Dynamic dispatch and launcher/helper calls require separate checks.

All 117 inventoried runtime paths matched the installed stabilization copy at
startup. This identifies on-disk bytes, not exhaustive loaded-code attestation.
The original isolated suite passed 297 Python tests with two missing-tool skips
and eight JavaScript suites. No real account database was copied or opened.

## End state and units

End state A: standalone Rust application replacing the daemon and its Python
application helpers, with no PyO3, Python fallback or additional TDLib service.
Python may remain as a development-only differential test driver.

Units of parity are:

* An IPC request and its ordered responses/events, or a disconnect.
* A pure TDLib projection over a synthetic JSON object and explicit media roots.
* A synthetic account-scoped TDLib update sequence and ordered projected events.
* A launcher/helper invocation, including exit status and owned process cleanup.

Only JSON object key ordering, task-owned fixture absolute roots, process IDs,
code revision identifiers and noncontract diagnostic timing can differ. Array
order, numeric values/types, absent/null/false, error text/code used by consumers,
account identity/routing and event order are not normalized. No approved product
behavior corrections are included in this migration at startup.

## Wire and safety

Preserve UTF-8 newline-delimited JSON requests with `id`, `cmd`, `args` and
optional top-level `account`. Preserve actual Python routing truthiness:
`args.account or request.account`, followed by the active account only when the
result is null. Success contains `id`, `ok:true`, `result`; failure contains
`id`, `ok:false`, `error`, `code`. Events have `event`.

The runtime remains `omagram/omagram.sock` with mode 0600 and a private 0700
parent. Linux same-user peer credentials, single-owner lock and verified paths
are mandatory. Input lines are at most 64 KiB, output backlog 16 MiB per peer,
16 peers, 512 outstanding IPC requests per peer and 1024 tracked TD requests.
Parsing limits are depth 8, 1024 aggregate container items and 16384 Unicode
characters per key/string. Preserve duplicate-key last-value parsing. Invalid
Unicode, invalid JSON and invalid request objects retain their wire errors.

TDLib int64 fields are strings; int32/int53 are JSON numbers. Do not blindly
convert string IDs or widen QML-safe numbers. Copy returned C buffers before
another receive/execute call. Only one receiver is permitted; apply updates in
receive order. Close all native clients and join owned workers on shutdown.

Each pending action carries peer, request token, account generation and deadline.
Local expiration is not native cancellation. Late callbacks cannot satisfy a
newly reused ID or dispatch new business work. Do not retry uncertain sends.
Rejected mutations preserve composition/recovery context. Bound subprocess
output, helper time, jobs and queues. Preserve media root, file/message identity
and protected-export checks, account/keyring/storage names and licenses.

Exactly one process may own each real TDLib database. Differential tests use
pure projections, fake native boundaries and disposable profiles only. No test
sends, sign-outs, database deletion, history clearing or read-state changes.

## TDLib boundary decision

Verdict: Compose — a small official four-symbol JSON FFI boundary with
`libloading`, `serde_json` and standard-library ownership; retain the existing
TDLib library rather than download or rebuild it through a binding crate.

The official pinned header documents create/send/receive/execute and buffer
lifetimes. The installed library exports all four symbols. The inspected
`tdlib` 0.10.0 wrapper links by library name, lacks execute in its inspected
wrapper and converts invalid UTF-8 lossily. The inspected `tdlib-rs` 1.4.0
package documents TDLib 1.8.61, generated types and generator/build dependencies;
the cached source here is 1.8.67. Neither inspection attests compatibility with
the currently mapped production library. A raw JSON seam avoids rebuilding the
schema and preserves forward-compatible TDLib objects while keeping the unsafe
layer explicit. Runtime version/commit probes are separate from cached sources.

Sources: official `td/telegram/td_json_client.h` at
`d1085f9cebc5a62379991ae1652673954f229c1f`;
https://docs.rs/crate/tdlib/0.10.0/source/src/tdjson.rs ;
https://docs.rs/crate/tdlib-rs/1.4.0 .

## Reproducible migration checkpoint

The crate builds independently of the dirty Python stabilization tree. Keep
Cargo outputs outside the checkout and use the locked dependency graph:

```sh
export CARGO_TARGET_DIR=/tmp/telebar-rust-target
cargo fmt --manifest-path rust/Cargo.toml --check
cargo test --manifest-path rust/Cargo.toml --locked
cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings
cargo build --manifest-path rust/Cargo.toml --locked
python3 -B rust/tests/replay-corpus.py "$CARGO_TARGET_DIR/debug/telebar-runtime"
```

The frozen, compressed synthetic corpus contains 21,136 inputs and exact Python
expected results. Its adjacent metadata records byte bounds, hashes and the
Python source hashes used to generate it. Replay needs Python only as a test
runner, not as application code, and imports no daemon modules. Regenerating or
extending the oracle with `run-differential.sh` still requires the matching dirty
stabilization sources identified by those hashes. Do not silently regenerate it
against old Git HEAD or publish unrelated stabilization work as a dependency.

Observed source checks: 41 unit tests and nine integration-test cases passed;
Clippy with warnings denied and both fresh/frozen differential runs passed with
21,136 inputs and zero differences. These are bounded migration checks, not
coverage of all 170 commands. Disposable native/application socket checks verify
startup without parameters, persisted account ordering/switch/add, observer event
ordering, failed-save preservation, settings and clean shutdown. They open no
working profile and deliberately reject production-compatible `hello`.

Retained recording state now belongs to each account's Engine State. Synthetic
real-ffmpeg/worker checks cover `videonote.send`, admitted preparation, explicit
retry after rejection, pre-send helper failure, cancellation and token discard.
Native acknowledgement tests cover malformed/wrong-chat acknowledgements,
wrong-account responses, 408/server uncertainty, expiry, disconnect, duplicate ID,
closure and capacity refusal. Unknown delivery disables retry and token discard
retains input files. This does not verify camera/microphone capture, desktop
consumer interactions or real Telegram sends. Preparation requires explicit home
and recording-runtime roots; fixtures do not discover the live session.

Synthetic startup/getMe now synchronizes name, phone and current-user ID into
the catalog without changing its order or active identity. Failed catalog writes
preserve the prior persisted view and expose a local failure bit without ending
the native driver. New-account media roots are derived when explicit home and
recording-runtime context are present; inert native fixtures remain root-free.

Remaining work includes complete production startup and compatible hello,
credential preparation, complete account/catalog synchronization and media roots,
notification D-Bus actions and sounds, global shortcut effects, managed playback,
voice/video capture and shutdown, full clipboard writer/file open/copy-image,
profile-photo jobs, storage/animation/diagnostics/UI/quit commands and bundled
bridge/proxy/menu/build helper replacement. Upload and recording snapshot
retention/cleanup and cumulative disk limits also need acceptance. No Python
runtime requirement is removed, installed candidate deployed or working account
opened by this checkpoint. The sent-video P1 remains independently unresolved.

## Acceptance and remaining scope

Format, Clippy, Rust unit/integration tests and differential corpora must run
against the reviewed candidate. Report compared inputs and differences, not just
suite totals. Include malformed/error cases, cross-account routing, late/reused
requests, expired sends, complete versus incremental position vectors, media
permissions and exact-owner shutdown. Test the real Rust socket process and
native QML consumers; mocks and compilation alone do not establish usability.

All 170 command handlers, state updates, projection families, settings/keyring,
notifications/sounds, recording/media/clipboard, bridge transport, diagnostics,
launcher, menu installer and build/install requirements must be accounted for.
No release cutover or removal of Python happens until complete acceptance.

Before cutover, inspect active/retained work and host watchers, preserve an
external restorable installed-code snapshot, stop the verified original owner
through its supported lifecycle and use consistent storage recovery procedures.
Do not ordinary-copy open databases. Apply the checked version coherently and
verify installed hashes, loaded candidate identity, accounts and consumer routes.
Preserve the active Git branch/index and publish only task-owned checked changes
to a separate checkpoint branch on the confirmed user-owned existing remote.

The sent-video P1 remains unresolved. Synthetic outgoing H.264 decoding did not
establish the reported real clip's projection, download, click, codec or audio.
Migration must not be reported as a playback fix. D-038 forum-parent unread and
historical window disappearance also remain open. Physical user acceptance is
pending until the user actually reports it.
