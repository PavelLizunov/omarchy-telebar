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
already running with the same user's access. Downloads export now applies the
same exact-inode cleanup owner, rejects source changes and refuses success when
the visible output/home directory has been replaced. Raw recording path discard
also removes only the admitted inode rather than reopening a substituted name.

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

## Official startup and private Secret Service checks

`tests/native_parameters.rs` exercises the official loaded JSON ABI through the
actual Driver on two disposable profiles. It sends synthetic parameters, creates
distinct encrypted database paths and reaches `phone`, then closes both clients.
The wrapper removes host home/bus/devices and network. This is actual native
parameter/database acceptance, not real Telegram authorization or existing-user
profile acceptance. Wrapper tests fail closed when required kernel isolation is
unavailable; a plain env-cleared host test is not equivalent containment.

`tests/keyring_service.rs` starts a finite private D-Bus and GNOME Secret Service
with synthetic credentials under a new temporary home. The real root-owned
`secret-tool` and normal asynchronous startup worker store/look up exact account
keys, sequentially reopen disposable databases and add a third account through
the actual catalog/Driver. Both profiles first reach `needCredentials`; normal
`credentials.set` requests use the real worker/keyring and acknowledge before
restarting only the routed profile. Each then reaches `phone`, and malformed
save input leaves the private stored credentials unchanged. Normal `account.logout`
then closes only the third unauthorized disposable client; the Driver recreates
its native client with a new generation, retains the same key/catalog identity
and reaches `phone` again. This never signs out a working account. A task-owned
user service enforces AF_UNIX-only sockets;
the test verifies that IPv4/IPv6 socket creation fails. An identity-preserving
Landlock ABI 9 launcher additionally denies file/device/pathname Unix sockets
outside the new private tree. Existing external sentinel/socket and inert device
negative controls prove denial, not merely absence. Read-only `/usr`, four inert
pseudo-devices and narrow non-secret passwd/machine/seat metadata are admitted;
no host bus/key directories are accessible. Both helpers are reaped.
Root/helper trust is not relaxed
for a user namespace, and no production route is added for these tests.

The private Wayland consumer also exercises normal `clipboard.image` and
`clipboard.files` Driver requests against the exact owned PNG offer and an
explicit foreground URI-list owner. It verifies private generated image bytes,
selected-path deduplication, missing-file/database refusal and unchanged originals.
This is separate from offer acknowledgement and does not touch the host clipboard.

```sh
cargo test --manifest-path rust/Cargo.toml --locked --offline --test native_parameters --no-run
bash rust/tests/run-native-parameters.sh /absolute/exact/native_parameters-test /absolute/verified/lib/libtdjson.so
cargo test --manifest-path rust/Cargo.toml --locked --offline --test keyring_service --no-run
gcc -std=gnu11 -Wall -Wextra -Werror -O2 rust/tests/keyring_sandbox.c -o /absolute/task-owned/keyring-sandbox
bash rust/tests/run-keyring-service.sh /absolute/exact/keyring_service-test /absolute/verified/lib/libtdjson.so /absolute/task-owned/keyring-sandbox
```

The proxy's actual TLS/WebSocket upgrade and duplex cipher pump are also tested
with a local self-signed fixture, explicit trust root, wrong-host/untrusted-root
negative controls, binary frames, idle ping/pong and fragmented pressure. Normal
production connections still use only admitted Telegram destinations and system
roots. Local transport verification does not establish remote Telegram reachability
or working-account bundled-bridge recovery.

## Send owners and public preauthorization transport

`tests/upload_groups.rs` runs real upload workers and Engine outcomes on mixed
photo/document groups. Accepted and uncertain siblings keep their own generated
inputs; a confirmed rejection cleans only its group, and unsent cancellation
keeps selected originals unchanged. `tests/send_driver.rs` adds actual Driver,
worker, synthetic JSON ABI and projected acknowledgement/update acceptance for
text, mixed albums, ordinary video, round video and profile-photo conversion.
An explicit `TELEBAR_SEND_FIXTURE` C build is the only fixture accepting these
local mutations; the default `inert_td.c` still refuses them. The fixture never
reads input paths, contacts Telegram or loads official TDLib. The send wrapper
reuses the root-identity-preserving Landlock/AF_UNIX confinement above. Final
media remains playable after native-owner close and unsent cleanup. The same
confined actual Driver also checks protected Downloads save/name collision,
explicit inert `xdg-open` lifecycle, sticker inflation, storage stats/clear,
round-video animation/cache, account listing and sound-request admission. Cache
clear preserves generated/sent media and selected originals; native replies are
still synthetic, not actual TDLib storage deletion. Normal bridge status and
invalid/unavailable set/test requests also preserve saved proxy metadata without
persisting a new bridge or attempting a native ping. These checks do not establish
real Telegram sends, host opener/audio output or physical recording/playback.

```sh
cargo test --manifest-path rust/Cargo.toml --locked --offline --test upload_groups
cargo test --manifest-path rust/Cargo.toml --locked --offline --test send_driver --no-run
gcc -std=gnu11 -Wall -Wextra -Werror -O2 -DTELEBAR_SEND_FIXTURE -shared -fPIC \
  rust/tests/inert_td.c $(pkg-config --cflags --libs json-c) -o /absolute/task-owned/libsendfixture.so
bash rust/tests/run-send-driver.sh /absolute/exact/send_driver-test \
  /absolute/task-owned/libsendfixture.so /absolute/task-owned/keyring-sandbox
```

`tests/run-voice-driver.sh` separately exercises the normal `voice.start` and
`voice.stop` Driver routes against a private synthetic PipeWire/Pulse source.
The actual Capture owner uses unchanged production ffmpeg Pulse input, session
configuration, Opus conversion, duration/waveform and retained-recording dispatch.
Duplicate and wrong-account stops are refused without replacing the recorder;
explicit unsent stop and shutdown reap the observed child and remove only their
raw inputs. One synthetic native send leaves playback bytes intact after close
and unsent recovery. The graph uses finite Managed owners inside the existing
identity-preserving confinement; root tool checks remain unchanged. The same
fixture now unifies normal `media.play`/`media.stop` and `sounds.play` requests
with actual ffplay and pw-play streams linked to its private null sink. It verifies
player/sound reaping and unchanged recording bytes, not audible device output.
Sound launch supplies explicit `--playback`: verified canonical resolution turns
`pw-play` into `pw-cat`, whose mode must not depend on the original executable
name. The earlier direct-name null-sink test did not exercise that production
launch contract; the normal Driver output check now does. This tests
silent synthetic audio, not a physical microphone, audible output or Telegram.
Normal Driver polling also retires spontaneous recorder completion or its finite
helper deadline. It keeps the original raw input and persists `prepareFailed`
for explicit retry/discard, with no automatic preparation or send. Changed
input/root identities become `unknown`, retaining the pinned original identity
and refusing retry; neither replacement is deleted. A checkpoint write failure
still exposes truthful live recovery. Synthetic early-exit/substitution/failure
checks verify the stopped transition and retained bytes. Explicit recording
retry opens the checkpoint's pinned original inode before worker admission;
pathname replacement cannot supply different bytes and be hidden by restoring
the original before native dispatch.

```sh
bash rust/tests/run-voice-driver.sh /absolute/exact/send_driver-test \
  /absolute/task-owned/libsendfixture.so /absolute/task-owned/keyring-sandbox
```

`tests/run-bridge-driver.sh` checks normal `bridge.test` and status through the
actual Driver, worker and one owned Rust proxy in a private network namespace.
The explicit C fixture alone supplies local ping success/refusal. Busy/refused
requests release admission and preserve child and metadata identity; explicit
subsequent commands reuse the child without automatic retries. No proxy enable
or persistence occurs. This is route/lifecycle evidence, not remote latency.

```sh
bash rust/tests/run-bridge-driver.sh /absolute/exact/send_driver-test \
  /absolute/task-owned/libsendfixture.so /absolute/exact/telebar-runtime
```

`tests/run-proxy-remote.sh` is a separate explicit public transport check, not part
of inert/default tests. It permits host networking only inside a private namespace
and sends one unauthenticated MTProto `req_pq_multi` through the actual Rust proxy,
normal system trust and fixed DC2 targets. A matching `resPQ` nonce, response bounds
and owner shutdown establish public preauthorization relay acceptance. No account,
authorization key, credential, database, read-state or Telegram message is used.
One client/query, zero retries; the native four fixed target attempts retain their
64-second overall limit. Mount both `/etc/ssl` and its existing Arch extracted CA
symlink destination; do not disable certificate verification for an incomplete
fixture trust store. Working-account bridge recovery remains a separate check.

```sh
bash rust/tests/run-proxy-remote.sh /absolute/exact/telebar-runtime
```

An explicit ignored lib-test observes actual owned proxy-child exit, advances one
owner clock across the 30-second recovery deadline, and checks saved proxy/prior
identity and non-starvation through Engine continuations. It uses loopback child
readiness plus synthetic replies, never a native proxy mutation or Telegram:

```sh
TELEBAR_TEST_PROXY_BINARY=/absolute/exact/telebar-runtime \
  cargo test --manifest-path rust/Cargo.toml --locked --offline --lib \
  native_driver::tests::actual_proxy_child_loss_and_saved_bridge_recovery_preserve_native_identity \
  -- --ignored --exact
```

## Supplementary valid commands and official proxy persistence

The frozen baseline remains 21,136 inputs. A separate 18-input supplement covers
valid success and native rejection for the six formerly error-only projection
routes: bot start, folder create/edit/reorder, proxy types, secret close and thread
history. Its error oracle follows `Daemon.handle_td` before success transforms;
that opt-in correction does not rewrite historical baseline expectations. Engine
checks independently cover wrong-account replies, terminal error codes, duplicate
callbacks and expired/reused IDs. The pinned official schema and Python consumer
both use `getMessageThreadHistory.message_id`; the native planner now preserves
that field rather than sending the unsupported `message_thread_id`.

```sh
cargo test --manifest-path rust/Cargo.toml --locked --offline --test valid_commands
bash rust/tests/run-differential.sh /absolute/exact/telebar-runtime --valid-command-corpus
python3 -B rust/tests/replay-corpus.py /absolute/exact/telebar-runtime --valid-command-corpus
```

`tests/native_proxy.rs` separately creates two unauthorized disposable encrypted
TDLib profiles without network, adds a prior loopback proxy, enables the actual
owned Rust bridge, closes/reopens the same temporary databases with injected keys
and restores saved bridge IDs through official `getProxies` and `editProxy`.
Disabling restores each original prior proxy and reaps the child. Both accounts
remain at `phone`; no authorization, messages or working-profile access occurs.
This is actual native proxy persistence, not working-account/network acceptance.

```sh
cargo test --manifest-path rust/Cargo.toml --locked --offline --test native_proxy --no-run
bash rust/tests/run-native-proxy.sh /absolute/exact/native_proxy-test \
  /absolute/verified/lib/libtdjson.so /absolute/exact/telebar-runtime
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
