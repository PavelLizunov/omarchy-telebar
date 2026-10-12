# Telebar Rust runtime candidate

This is the standalone replacement in progress, not the installed application.
Native QML/JavaScript, Quickshell and official TDLib are retained. The working
Python backend continues to own real accounts until the full migration gate and
safe cutover pass. `session::CUTOVER_READY` is false: production daemon/window
launch is deliberately refused. Do not enable it merely because a build passes.

The behavioral and storage contract, observed evidence and remaining acceptance
are maintained in [PARITY.md](PARITY.md). The sent-video incident is independently
unresolved. Synthetic decode/proxy/socket tests are not Telegram or hardware
acceptance.

## Build and isolated checks

Rust 1.88 or newer, system OpenSSL headers/libraries and `pkg-config` are needed.
Cargo.lock is the reviewed graph. Keep generated files outside this checkout:

```sh
export CARGO_TARGET_DIR=/tmp/telebar-rust-target
cargo fmt --manifest-path rust/Cargo.toml --check
cargo test --manifest-path rust/Cargo.toml --locked --offline
cargo clippy --manifest-path rust/Cargo.toml --locked --offline --all-targets -- -D warnings
cargo build --manifest-path rust/Cargo.toml --locked --offline
bash rust/tests/run-differential.sh "$CARGO_TARGET_DIR/debug/telebar-runtime"
python3 -B rust/tests/replay-corpus.py "$CARGO_TARGET_DIR/debug/telebar-runtime"
bash rust/tests/run-helper-parity.sh "$CARGO_TARGET_DIR/debug/telebar-runtime"
bash rust/tests/run-proxy-process.sh "$CARGO_TARGET_DIR/debug/telebar-runtime"
bash rust/tests/run-consumer-socket.sh "$CARGO_TARGET_DIR/debug/telebar-runtime" /absolute/verified/lib/libtdjson.so
```

Offline checks need the locked crates already fetched. Python is only the test
oracle/driver, never a Rust application dependency. Tests with real TDLib load
an explicit verified library and use pre-parameter/synthetic clients. They must
not open working profiles, contact credentials or duplicate native mutations.

`tests/ClientConsumer.qml` loads the actual TelebarClient and Quickshell Socket
through the installed qml-preview static adapter. Supply `fixtureSocket` before
creation, a task-owned `--inert-consumer` process and `readyProperty: ready`.
Inspect the PNG and metadata separately; this validates hello/settings callbacks
and ordering, not Main layout or compositor behavior. The optional QtTest socket
runner requires dynamic Quickshell QML plugins, absent on stock Arch; that
missing import is not a test pass or permission to open a production panel.

System helper ownership checks are enforced in normal application code. The
isolated Wayland lib-test build has a `cfg(test)`-only immutable hash admission
for helpers whose root UID is unmapped by Bubblewrap. Its wrapper verifies host
ownership and hashes before mounting the manifest. There is no release override.

## Generated input lifetime and finite storage

The owner journals exclusively created upload/recording copies before writing
bytes and durably marks them submitted before the actual native dispatch. A
startup after an abrupt exit reclaims only valid unsent journals with matching
owned inodes. Missing/malformed journals, replacement files, legacy media and
submitted/uncertain outcomes remain untouched. Album group transfer is not native
submission. Failure to retain an input refuses that request, not the service.
Dispatch validates the consumer-visible absolute root/path against the retained
inode; converter/probe I/O uses admitted fd paths. Cleanup atomically holds an
entry before checking its inode and never deletes a substituted file. A restore
collision preserves the replacement in a private quarantine rather than erasing
it. This is Linux-specific bounded ownership, not isolation from malicious code
already running with the same user's access.

Each account's sent directory and the runtime media directory admit at most
4096 top-level entries and 32 GiB, including existing legacy/uncertain inputs,
with 512 MiB free-space headroom, including 16 KiB reserved for durable/atomic
journal writes. A descriptor-owned lock covers reservation
through copy/capture/conversion completion. Oversize inventory, links and special
files refuse new work rather than evict existing media. Video conversion has a
512 MiB output cap and an exclusive output fd. These are finite admission limits,
not evidence of physical peak storage measurement or safe sent-file deletion.

Telegram may reference local inputs for subsequent playback. Accepted messages
and timeout/disconnect do not authorize age-based deletion. Raw recordings and
unknown profile-photo inputs are preserved across restart; only stale clipboard
scratch has an age policy. Explicit confirmed recording discard still owns its
narrow validated files. Per-account retained-recording metadata is checkpointed before preparation and
native dispatch. Restart restores the original token, chat, reply/topic and send
options through the actual hello snapshot. Proven-unsent work requires explicit
re-preparation; uncertain dispatch restores `unknown` with retry disabled. Missing,
substituted or corrupt metadata never authorizes deletion/resending. Corrupt
checkpoints expose an unknown/dismiss-only guard, preserving all media. Explicit
resolution clears durable metadata so another restart does not resurrect it.
No recovery action sends automatically or contacts credentials/account databases.
Dismissal of unknown/corrupt metadata first durably prevents unsent-journal cleanup
from reclaiming the account's existing generated inputs, including the narrow
checkpoint-before-journal crash window. Failed retry persistence rolls the live
state back; failed capture checkpoint setup returns explicit retained recovery
and reports the stopped recorder rather than deleting raw input or stranding UI.

```sh
cargo test --manifest-path rust/Cargo.toml --locked --offline --test input_recovery
cargo test --manifest-path rust/Cargo.toml --locked --offline --test recording_checkpoint --test recording_recovery_socket
TELEBAR_TEST_INERT_ABI=/absolute/fixture/lib/libtdjson.so \
  cargo test --manifest-path rust/Cargo.toml --locked --offline --test input_driver -- --ignored
```

## Implemented helper entry points

The candidate implements `--launch`, `--host-window`, `--daemon`, `--menu`,
`--build-tdlib` and the owned `--proxy` child mode. Launcher/daemon cutover is
blocked until acceptance; launcher names in the installed plugin still invoke
Python. Menu/build entries must not be used on real user configuration as a test.
The build helper preserves the official pinned TDLib commit and a capped private
build log. A complete TDLib rebuild is not performed just to test this port.

The bundled transport keeps the Flowseal algorithm attribution. AES/TLS use
system OpenSSL through maintained bindings; WebSocket framing/handshakes use
Tungstenite. Only Telegram WebSocket hosts and the original DC2/DC4 IP fallback
are allowed. Proxy configuration is passed through a private stdin ownership
pipe, not argv. Socket bounds, finite handshake/write budgets and owned teardown
are checked independently of live Telegram connectivity.

## Native package and host handoff

`--package <absolute-source> <new-absolute-output>` assembles an inert candidate
outside the checkout. It refuses existing output, unsafe source links, unowned
or writable inputs, missing entry points and Python QML/JavaScript/menu invocations.
Per-root file types are allowlisted; Python files/bytecode are rejected before
output creation. Output creation and writes use the same pinned parent/child
storage descriptors, so a renamed/replaced pathname cannot redirect the package. It
copies only QML/JavaScript, assets, licenses, manifest/menu, one ELF runtime and
five privileged-mode Bash shims. No Python application file or fallback is
included. Source hashes and the compile-time cutover state are recorded in
`runtime-candidate.json`. This command never installs/enables the plugin.

```sh
cargo build --manifest-path rust/Cargo.toml --release --locked --offline
"$CARGO_TARGET_DIR/release/telebar-runtime" --package "$PWD" /absolute/new/native-candidate
bash rust/tests/run-package-process.sh /absolute/new/native-candidate
omarchy plugin validate /absolute/new/native-candidate
bash rust/tests/run-consumer-socket.sh "$CARGO_TARGET_DIR/debug/telebar-runtime" --no-library
```

QML and the menu invoke the compatible executable entry points directly. The
native launcher uses the existing local session bus and `StartTransientUnit`,
with `Type=exec`, fail-on-existing ownership, no restart policy, eight-second
stop grace, control-group cleanup and loader/interpreter environment removal.
It waits for the exact start job result instead of equating job submission with
execution. Root-tool validation is not weakened for recovery user namespaces:
the user manager owns the host process. A finite no-native fixture verifies
this real handoff, duplicate refusal and cleanup without account/keyring work.
The live check is explicit and ignored in the default suite.

`--inert-no-library <private-directory>` exposes local hello/settings and native
query rejection on the disposable socket without loading TDLib or enabling
startup. `tests/NoLibraryConsumer.qml` verifies the actual product client stays
connected. `tests/MainConsumer.qml` also loads the complete actual Main with
`autoStart: false`, an explicit socket and `surfaceVisible: false`, and reparents
its real content subtree for offscreen inspection. Tested noLibrary and
needCredentials do not establish ready chats, compositor or live authorization
acceptance. Use the reviewed native Commons tokens with inert local Process/
FileView imports, never replace the actual native Socket with a mock.

`tests/inert_td.c` is a fixed synthetic four-symbol JSON-ABI consumer fixture,
not official TDLib. Build it in a new private `lib/` directory with
`gcc -std=gnu11 -Wall -Wextra -Werror -shared -fPIC rust/tests/inert_td.c -ljson-c -o /absolute/fixture/lib/libtdjson.so`
and pass that explicit library to `--inert-consumer`. It returns one chat/three
synthetic messages, refuses parameters/mutations and has no network/files/device
access. `MainConsumer.qml` with `expectedAuth: ready` checks actual list/history
rendering. This does not authorize profile access or replace official native
acceptance; json-c and a C compiler are development-only fixture dependencies.
