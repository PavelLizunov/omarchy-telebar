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

Observed source checks: 41 default unit tests and twenty-four default integration-test
cases passed; four verified-library tests passed explicitly
(startup ownership, three-client startup, dummy-audio Driver lifecycle and
synthetic clipboard-owner retirement), plus one isolated real Wayland test.
Run library tests serially with
`-- --ignored --test-threads=1 --skip clipboard_offer_is_verified_by_the_actual_isolated_consumer`;
the fifth test requires the separate reviewed namespace below.
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

`credentials.set` now validates the original envelope and prepares a redacted,
bounded cancellable Secret Service save worker. Values travel through stdin,
never argv; partial store failure is reported. Completion acknowledges the save
before session-owned startup, and retired/expired tokens or stale generations
cannot trigger parameters. Synthetic fake-store checks passed; no real user keyring was contacted. A separate
actual private GNOME Secret Service fixture now exercises root-owned secret-tool,
account-key lookup/store, asynchronous Driver startup, sequential encrypted
disposable database reopen and account.add. AF_UNIX-only service policy refuses
IPv4/IPv6 sockets. Review exposed the first wrapper's filesystem/Unix/device
containment gap: env clearing alone was not isolation. The corrected test-only
Landlock ABI9 launcher preserves root helper identity and denies host file,
pathname/abstract Unix socket, device and signal scopes; external existing
file/socket/inert-device controls pass. `/usr` and narrow non-secret seat/passwd
metadata remain read-only; only the private fixture tree is writable. User-manager
PrivateUsers=identity preflight was refused, so no root remapping/trust relaxation
or privileged fallback was used. This is native
helper/private service evidence, not the user's Secret Service or account acceptance. Explicit asynchronous startup preparation is now integrated into
the single native Driver with account/client/generation/serial/auth ownership.
Startup and user jobs share a two-worker admission ceiling; a bounded FIFO
queues up to ten account startups without retry loops, deduplicates current
attempts and drops stale account/auth/serial entries. Shutdown cancels and joins
both pools and retires the queue. Current native parameter attempts also suppress
repeated wait-parameters events. Queue time counts toward the 180-second owner
deadline. Explicit regressions cover successful synthetic parameter effects,
terminal-once, auth/serial/generation/client replacement, expiry and shutdown.
A real TDLib pre-parameter driver check used an invalid disposable storage root
for three native clients, so preparation failed before Secret Service and no
profile/parameters were opened. Complete successful startup and production context still require
acceptance. Shared keyring lookup/store budgets now
support an explicit deadline and cancellation. An injected database-key policy
check confirms that a stored noncanonical Base64 representation is preserved,
lookup failure or an existing nonempty database never generates a replacement,
and an exhausted deadline starts no helper.

Local `ui.open` now targets window peers only, preserves selected-account
identity and ordered open-before-terminal delivery, and retains one pending
open for 30 seconds. Only an admitted window hello consumes it. `ui.focus`
withdraws the exact account/chat notification identity, keeps peer ownership,
and emits an owned no-retry reaction-read effect when enabled. Notification
withdrawals still need the desktop transport. Peer retirement clears focus only
for its owner. `app.quit` broadcasts before acknowledging and the actual inert
application socket drains for 300 milliseconds before bounded native shutdown.
Synthetic routing/admission tests and actual socket quit passed; no desktop
window was opened or real read-state changed.

Explicit `sticker.lottie`, `storage.stats` and `storage.clear` native
continuations now dispatch bounded cache workers. The gzip helper receives only
admitted private bytes, not paths or shell text; input is at most 1 MiB, output
16 MiB and helper time ten seconds. Parsed JSON has depth/item/string bounds and
must be an object. Cache writes retain SHA-256 identity and mode 0600; finite
no-follow inventory trims toward the existing 256 MiB / 4096-file ceilings.
An overfull uninspectable directory refuses new writes rather than treating an
inventory failure as empty (a deliberate resource-safety refinement).
Synthetic tests cover actual gzip/worker/Engine completion, corrupt/nonobject
input, expansion overflow, cancellation, symlink refusal and preserved sent
files. Failed-hash memoization, exact scheduling/capacity errors and actual QML
Lottie rendering remain unverified; no production storage was cleared.

`videonote.animation` now uses a native download continuation and cancellable
worker with an owned stable source snapshot (64 MiB maximum). It emits bounded
silent 240-pixel / 20-fps animated WebP through ffmpeg, preserves the original
path/size/nanosecond-mtime SHA-256 cache identity, refreshes cached demand time
and cleans its exact source/partial/empty container. Output has a 16 MiB disk
cap plus complete RIFF-size validation; note-cache trimming keeps 128 MiB / 512
finished pictures. Real synthetic ffmpeg conversion/cache/cancellation checks
passed. No actual reported clip, physical UI playback or camera input was tested.
Crash-left snapshot GC and exhaustive cache-limit acceptance remain incomplete.

`videonote.discard` now refuses any path owned by a retained send across
accounts before deleting an admitted raw recording. Only a regular owned direct
child of the explicit `omagram/rec` root is eligible; symlinks and unrelated
paths are refused. Retained/unknown outcomes still require token discard and
preserve potentially consumed inputs. Synthetic deletion/preservation checks
passed; no live recording was deleted.

`process::Managed` now provides pollable foreground helpers with bounded input,
output/lifetime, explicit interrupt/terminate grace and exact unreaped process-group
cleanup. Existing finite helpers reuse it. Actual cat/yes/sleep, cancel/timeout/drop
and exited-leader descendant tests passed. Driver `media.play`/`media.stop` now own
admitted selection serials, downloaded-file continuation, a foreground ffplay
owner, account generation, no-retry content-open effect and bounded teardown.
The player retains the admitted source descriptor through a parent `/proc` path,
so path replacement cannot substitute media. A synthetic ffplay test used only
SDL dummy audio/video drivers; no desktop, real audio or Telegram read state was
accessed. Driver tests additionally cover stale-selection refusal, retained
previous owner, admitted stop ordering/reap and generation-change cleanup without
dispatching synthetic native IDs. Actual sound/device consumer, normal-end/quit
coverage and complete capture/clipboard ownership still need acceptance.

`file.copyImage` now validates original photo membership and server save
permission before a bounded no-follow PNG worker. Prepared image bytes reach a
still-admitted token and a Driver-owned foreground `wl-copy` child, never an
unmanaged selection fork. Pending writes have a ten-second readiness budget;
retirement/replacement/shutdown kills and reaps only the exact child. After stdin
delivery, one bounded `wl-paste` read verifies the SHA-256 of the actual selection
before success; no retries. A 100-ms grace only gates the probe, never success.
Successful selections have no arbitrary expiry: their foreground process belongs
to the Driver until selection replacement, helper exit or shutdown. Readiness is
still finite. Synthetic checks cover permission/bytes, redacted
offers and exited-reader/disconnect retirement. A real Driver test additionally
used headless labwc/pixman inside a private network/home/device/bus-free namespace:
actual foreground wl-copy, verification, independent read after acknowledgement
and peer disconnect, mismatch rejection and exact helper/reader shutdown passed.
No session clipboard was read or replaced. No physical desktop/UI acceptance.

Reproduce the isolated check with an exact current `cargo test --lib --no-run`
executable, the verified TDLib and a reviewed labwc package root:

```sh
bash rust/tests/run-clipboard-wayland.sh <lib-test-binary> <verified-tdlib> <labwc-root>
```

The observed fixture was built outside Git from signature-verified packages from
the configured Arch mirror: labwc 0.20.2-1, wlroots0.20 0.20.2-1,
libliftoff 0.5.0-1 and libsfdo 0.1.4-1. The script does not install packages or
alter the live compositor. Review new fixture dependencies before reuse.

`diagnostics.ui` and `diagnostics.status` now use original Engine admission,
per-peer one-second rate limits and a bounded numeric/boolean UI allowlist. Real
socket peer PIDs supply numeric process counters; no argv/environment/memory is
read. Private trace logging uses one 2048-record queue, a writer and a separate
atomic-heartbeat watchdog; no TDLib/account state is shared with either thread.
The writer pins its 0700 directory, refuses symlinks/hard links/special files,
keeps 0600 logs and rotates 4 MiB with four backups. Logging failure is isolated
from the session. Actual disposable application socket tests validate reports,
rate refusal, status/counter identity and no caller-string logging. Finite tests
exercise real rotation, symlink preservation and a genuinely idle owner caught
by the independent watchdog after 2.5 seconds. Rust thread-stack capture is NOT
implemented (`stack_capture_supported:false`); native receive age/oldest TD
metrics and complete request/response diagnostic events remain unfinished.
Unknown metrics are null, not invented zeros. No physical UI/profile acceptance.

Personal sound previews now use the existing deterministic WAV owner with
no-follow 0600 cache entries, a finite 350-entry inventory and original
300-entry WAV pruning. `sounds.play` acknowledges after validation; sound
preparation/playback stays best-effort and cannot undo settings persistence.
Admitted settings/sound changes queue at most two previews, captured account
client/generation is checked before a background completion can spawn, and all
user/startup/sound work shares the two-worker cap. Driver owns one foreground
`pw-play --media-role Notification` with ten-second budget and keeps its input
descriptor alive. Shutdown joins/cancels sound jobs and reaps the exact player.
Synthetic tests check exact legacy WAV bytes, pre-cancel/no filesystem work,
symlink preservation, malformed cache refusal and 300-file pruning. A verified
TDLib-only Driver fixture checks admitted setting/preview scheduling, stale
account completion and shutdown without dispatching synthetic native IDs or
opening real storage. The private PipeWire fixture creates only a null sink,
explicitly configures/links its consumer ports, and exercises actual `pw-play`
completion/reap in a device/home/network/session-bus-free namespace. Physical
sound and notification-triggered sound policy remain unverified/unimplemented.

`file.open` validates requested native file identity/completion and media-root
membership, then opens a no-follow descriptor. Original IPC ownership/current
generation/expiry is checked before `xdg-open` starts; at most eight foreground
openers with 60-second deadlines are retained/reaped. Actual xdg-open is tested
using a disposable MIME registry and `/usr/bin/cat` handler, with a dummy
nonexistent display address: original bytes survive path replacement, while
symlinks and wrong/incomplete native files are refused. This does not attest
real desktop application/portal handoff or long-lived app use.

`profile.setPhoto` admits an explicitly selected upload outside account
databases, snapshots bounded bytes for pipe-only ffmpeg conversion, and writes
one private square JPEG under the explicit recording runtime. Decoder URLs and
unrelated local files are refused by the `pipe` protocol allowlist. A shared
input lease cleans unsubmitted/stale/cancelled preparations, successful replies
and confirmed 4xx rejection. The lease is marked submitted at Driver native
dispatch, not planning. Timeout/disconnect/408/5xx retain submitted input because
local retirement cannot cancel TDLib use. Synthetic tests exercise actual ffmpeg,
8x8 square output, private input cleanup, no-spawn pre-cancel, refused URL
playlist and original Engine admission/native success/error/expiry outcomes.
No real profile was changed; cumulative/crash-left retained-input GC remains open.

Recorder ownership now handles admitted `voice.start/stop` and
`videonote.record/stop` using one explicit Driver capture context. Production
argv stays fixed; fixture-only synthetic lavfi inputs are not exposed over IPC.
Output is exclusively created mode 0600 under the compatible account sent or
runtime recording root and ffmpeg writes the retained parent's descriptors;
round-video output container is explicit, output capped at 512 MiB, microphone
and camera helpers are limited to 605/65 seconds. Stop validates account,
worker capacity and send/reply/topic/options before consuming capture. SIGINT
has five-second grace, exact group reap precedes decoding, and send=true reserves
retained state before stop/preparation workers. Current-session retirement and
shutdown cancel/reap unsent captures and delete exact partial/preview outputs;
retained preparation preserves its input on cancellation/failure for explicit
retry. Synthetic lavfi tests exercise actual voice/video/preview preparation,
stopped discard and exact helper reap with no devices/native sends. Driver
fixture checks invalid-account/options preservation, admitted stop/idle and
stale-generation cleanup. Hardware capture/session environment, original stale
recording GC and live recording UI acceptance remain open.

Verdict: Compose for desktop notification transport. Maintained `zbus` 5.19
(minimum Rust 1.87) owns D-Bus encoding/authentication/signals, rather than custom
FFI or putting message text into `gdbus` argv. Direct async-io/channel/futures-lite
use supports one private owner thread with bounded 64-command/event queues,
three-second initialization and two-second calls, no retry loops, explicit local
Unix address and shutdown flag/join. Notification replacement is serialized in
transport even before the account owner consumes acknowledgements. Policy
preserves quiet/focus/silent/latest-message/account title/no-preview rules,
original message IDs for read/mute/react buttons, 1.2-second global/four-second
person sound gaps, Omarchy's `.local/state/omarchy/notifications.json` DND and
200-image/250-inventory tiny JPEG cache. Driver pins account generation and
notification serial before accepting display/action/sound; stale native replies
cannot make a new-generation button active. Read/mute/react are allowlisted
internal no-retry native requests. Open/reply reach existing window routes or
pending-open state; shell summon/new-window launcher fallback remains incomplete.
Actual isolated bus/server checks show escaped previews, replacement, action/
closed signals and finite teardown; actual Driver fixture checks reply route and
read query without dispatching synthetic client IDs. Synthetic policy checks
cover quiet/focus/silent/hidden/account text, buttons, sound gaps/DND and picture
membership. Working notification server, sound devices and profiles untouched.

The session owner, native launcher, global shortcut worker, bundled proxy/bridge,
menu and TDLib-build entry points now exist in source. `CUTOVER_READY` remains
false, so daemon/window entry points refuse working-profile access. The isolated
native package now contains no Python runtime/helper/fallback. The working
installed application is unchanged Python; no Rust cutover has occurred.

Global shortcut work is single-flight across accounts and shares the aggregate
two-worker limit. Later settings mutations wait before persistence; current
responses precede settings events. Completion pins the original IPC token,
not a reusable wire ID; disconnect/expiry cannot save deferred settings or
start a stale helper. Captured binding insertion order and unavailable/failed/
taken/off statuses are preserved. Initial saved globals and shortcuts.apply use
the same owner. Session/catalog and compatible hello are exercised in disposable
pre-parameter mode, never credentials/database opening.

Native launcher parity covers arguments, account-safe runtime paths, private
links/generated shell, desktop-entry identity and Hidden/NoDisplay preservation,
reuse of an existing backend and host-managed fixed-argv starts. Production
startup supplies explicit capture/keyring/clipboard/notification/global contexts
only after the single socket lock. The implemented main loop has no production
120-second expiry; inert modes stay finite. It is not cutover accepted.

Verdict: Compose for bridge transport: `openssl` 0.10.81 supplies AES-256-CTR
and system TLS, `tokio-tungstenite` 0.29.0 supplies maintained WebSocket framing
and handshake checks, and Tokio 1.53.2 owns at most 32 connection tasks in the
separate child process. Original Telegram hosts/DC fallback and Flowseal MIT
attribution remain. Finite config/handshake/write deadlines and owner EOF/signal
shutdown are tested. Native bridge enable/disable continuations preserve previous
proxy identity, persist only current metadata and release stale admission.
The official parameter/database path reaches the pre-phone authorization state
on two disposable no-network accounts. New account.add receives the same durable
recording checkpoint owner before native client creation/catalog persistence.
Saved-account restoration is queued after bridge-ready parameters. A confirmed
owned proxy-child loss queues one recovery after 30 seconds; an unready first
account cannot starve later ready profiles. Native mutation uncertainty is never
automatically retried. Synthetic owner/deadline checks pass; real recovery and
Telegram TLS/relay connectivity, account proxy restoration and full-duplex
behavior remain unverified. The actual duplex pump now has finite fragmented
cipher-pressure and idle ping/pong checks; the same native TLS/upgrade helper
passes local trust/hostname negative controls and binary transport. No remote
TLS/relay or working-account success is inferred. A finite 295-input helper corpus has zero differences
for AES, obfuscation, packet splitting and managed menu text. It is not network
acceptance. The real proxy process survives 32 inert loopback connections and
exits on its private owner pipe closing in a host-network-free namespace.

Generated upload inputs now carry leases: failure/cancellation before native
dispatch removes exact snapshots; submitted/408/5xx inputs survive unknown use.
Album groups own separate leases so one rejection cannot remove other groups'
submitted inputs. Recording preparation errors clean generated copies while
preserving the retained original. Generated recording leases now move through
the actual worker result into the retained owner and are marked submitted only
at Driver dispatch. Stale/drop/capacity failures clean generated snapshots;
unknown dispatched inputs survive owner retirement. Synthetic capture and
real Engine completion/retirement tests cover this boundary. Exclusive copies
now have a durable inode journal before byte copying; submission is durable before
TDLib dispatch. Startup reclaims only proven-unsent journaled inodes. Missing,
malformed, substituted and uncertain entries remain. Per-account sent/runtime
input admission is capped at 4096 entries/32 GiB with 512 MiB free headroom;
lock ownership spans copy/capture/conversion, including legacy/uncertain inventory.
No accepted/uncertain sent input is evicted just because a success update arrives:
TDLib and native playback may still reference it. Exact recovery, cap, lock,
parent/input replacement and Driver journal-refusal regressions pass in isolation.
A bounded same-family reviewer found consumer-visible path substitution, capture
replacement cleanup and omitted journal budget. Corrections validate the actual
absolute root/path before dispatch, use fd-only converter/probe paths, hold then
check cleanup entries without overwrite, and reserve 16 KiB journal headroom.
Exact capture/root/controlled-unlink and near-cap/Driver controls pass; newly
created upload-container reachability is fsynced before journal copying. This is
coordinator-verified closure of cited findings, not cross-family acceptance or a
power-loss proof. Retained UI/crash metadata is now durably checkpointed and
restored per account before unsent cleanup. Original destination/options survive;
uncertain/corrupt/substituted input restores unknown with retry disabled. Recovery
never deserializes native content into automatic work. Real abrupt test-owner
termination, socket restart, hello UI state, explicit discard/dismiss and durable
resolution pass on inert media. Complete Main recovery states render against the
actual Rust/native socket at radius 0/8 and are inspected; real media use,
power-loss and physical/compositor interaction remain unverified. Same-family
review found the checkpoint-before-journal unknown-dismiss crash window and two
checkpoint-failure lifecycle gaps. Shared corrections durably preserve unknown
inputs before dismissal, roll retry back before its error, and retain stopped
capture recovery through an explicit action/idle event. Synthetic exact crash-
window, failed-write, capture-open/save and native Driver recovery checks pass;
reviewer execution, cross-family independence and full migration are not claimed.

Fixed system helpers now require root-owned non-writable /usr executables and
safe ancestors; application helpers require owned non-writable files. The
isolated Wayland test-only build pins host-verified helper hashes where root UID
is unmapped. No release/application environment override weakens these checks.
Actual offscreen Quickshell Socket/TelebarClient acceptance passed hello shape,
settings response-before-event and window-only fields against the Rust socket.
The initial stock QtTest attempt lacked dynamic Quickshell plugins; the verified
native static qml-preview adapter supplied the actual consumer instead. The
viewed diagnostic PNG does not prove Main layout, hardware or compositor use.

Remaining work: exact completed-command/helper acceptance, safe native package/
launcher/QML cutover, production account/catalog/notification/bridge behavior,
resource retention/GC/disk policy, startup and absent-library degradation,
complete cache/animation acceptance, device/media/clipboard/opener
acceptance, critical-path review and actual loaded application identity.
Missing/unloadable TDLib now retains local hello/settings and reports noLibrary
instead of ending the IPC owner. Native queries for client zero return the normal
Telegram-is-not-available error, not peer loss. Global/local worker completion
still uses account generation without requiring a native client. Actual isolated
socket and product QML-client checks pass, including asynchronous settings,
response-before-event, rejected queries, account switch and clean shutdown.
Native production identity/language and receive/request counters are explicit.
Finite stale clipboard-paste startup cleanup protects named retained inputs and
never traverses symlinks or age-deletes recordings/sent inputs/databases. Submitted-input
cumulative/crash-left cleanup remains a separate unfinished policy. The
sent-video P1 remains independently unresolved.

Native candidate assembly copies only the admitted QML/JS/assets/licenses and
one ELF plus five fixed Bash entry shims. It refuses unsafe or existing output
and records exact input hashes; it never enables or installs a plugin. A
separate-context same-family reviewer found file-type admission and pathname
output-creation gaps. Per-root file allowlists, JS consumer validation and pinned
exclusive output creation now close those owning mechanisms; planted Python/JS
and deterministic parent-replacement regressions pass. This is direct corrected
verification, not cross-family independence or review of all application owners.
The actual packaged proxy shim/owner pipe and BASH_ENV refusal pass in a home/bus/
network/device-free namespace. Direct QML/menu executable calls remove the
interpreter dependency without changing the compatible entry names.

Host-managed starts now use native zbus on the existing local session bus rather
than execute systemd-run inside the recovery namespace. Exact StartTransientUnit
properties keep Type=exec, fail-on-existing, no automatic restart, loader/interpreter
sanitization and control-group cleanup. The exact JobRemoved result establishes
exec/start acceptance. A finite no-native owner was launched from a mapped user
namespace and inspected from the host: native image and host namespace matched,
duplicate start was refused without changing PID, shutdown removed its socket.
The working Python backend stayed PID 188718. Native trust checks remain strict.

The bounded failed-sticker FIFO memo and every-32-success cache trimming now
match the original policy, with startup trimming on the final owner. Legacy
making-WebP cleanup is separated from completed-note LRU accounting; fresh
partials and symlinks are preserved by the executable regression. Animated
note cache paths/frames still pass synthetic ffmpeg checks. Resource/concurrency
and crash-left cumulative cleanup acceptance remains incomplete.

Mechanical corpus inventory: 123 commands in the command projection corpus plus
nine local preference commands: 132 of 170 registry commands. The remaining 38
are lifecycle/helper routes and need separate behavioral evidence, not guessed
coverage from matching names. An explicit 170-route empty-argument reachability
check now rejects missing handlers and peer loss with no native/device dispatch;
validation reachability is not complete command behavior. This is not full
migration acceptance.

The complete actual Main now accepts explicit socket/autoStart/unshown-surface
context with unchanged production defaults. Its exact native content subtree was
rendered against an inert Rust owner for noLibrary and needCredentials at radius
0 and 8. Images were inspected; native palette/density and existing status/setup
copy were retained. A fixed synthetic four-symbol C ABI fixture additionally
feeds one chat and three history messages through the real Rust socket/FFI,
projection, actual Main/ChatList/ChatView at radius 0 and 8; both PNGs were
inspected. This fixture is explicitly not official TDLib and refuses parameters
and native mutations; it contains no credentials/files/device/network operations.
The only recorded warning was the deliberately unavailable Hyprland connection;
report-policy acceptance does not establish compositor placement/focus. Real
ready accounts/media and physical interactions remain unverified.

Dependency advisory check: cargo-audit 0.22.2 from the signature-verified Arch
package inspected the locked graph against RustSec database commit
3d2bd193db927dd715d34f92fe7b9242a18299dc: zero listed vulnerabilities or warnings.
Yanked-crate checks were explicitly disabled; this is advisory evidence, not a
proof of dependency safety or a complete license audit. No OS package was installed.

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
