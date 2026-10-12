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
