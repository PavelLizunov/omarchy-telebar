# Native runtime contract

The current implementation is Rust with native QML/JavaScript, Quickshell and
single-owner official TDLib. Account names/keys, `omagram` storage/runtime paths
and command/IPC identities remain compatible. Launch readiness is enabled;
`migration_complete=false` records incomplete manual and exhaustive acceptance.

## Protocol and account ownership

UTF-8 newline-delimited requests carry `id`, `cmd`, `args` and optional `account`.
Account routing preserves the established null/falsy handling and active-account
fallback. Success is `id`, `ok:true`, `result`; failure includes `error` and `code`.
Ordered events use `event`. TDLib int64 remains strings, int32/int53 JSON numbers.
Only object-key order and noncontract timing may differ in synthetic comparisons;
array order, types, null/false/absence and consumer-visible errors stay exact.

The private socket is mode 0600 under a mode-0700 directory, with same-user peer
checks and one original-inode lock. Limits include 64 KiB input lines, 16 MiB output
backlog per peer, 16 peers, 512 pending IPC requests per peer and 1024 native queries.
Wire parsing bounds depth, container items and strings. Native C buffers are
copied before another FFI call; one receiver preserves TDLib update ordering.

Each pending operation owns its peer/request/account generation/deadline. Expiry
retires continuations, not an already-submitted Telegram mutation. Late replies
cannot satisfy a reused ID; uncertain sends are not retried. Clients and workers
close before the profile owner releases the lock. No second service opens those
same working databases.

## Paths, helpers and retained inputs

`Storage` anchors owned ordinary directories/files and no-follow identities.
Helpers use admitted absolute executables, discrete argv, bounded I/O/deadlines
and exact child ownership. The systemd user manager retains root-tool/keyring
trust even when the shell runs through a separate recovery namespace. Job-start
completion and loaded socket readiness are separate observations.

Generated inputs are journaled before writing and durably marked before native
submission. Missing, substituted, submitted, uncertain or playback-referenced
files are preserved. Confirmed unsent discard removes only its owned inode;
selected originals are not adopted. Recovery never auto-sends or auto-retries.
Checkpoint corruption is dismiss-only and does not authorize media deletion.

Per-account sent/runtime-media admission is finite: 4096 entries, 32 GiB and
512 MiB free headroom, with journal allowance and a 512 MiB conversion output cap.
New work refuses at capacity instead of evicting retained media. Exact resource
bounds are not physical peak-load certification or same-UID isolation.

## UI and lifecycle

Theme palette/font/spacing/radius remain reactive. Radius zero stays zero;
avatars, round video and unread dots retain their semantic circles. SidebarHandle
owns toggle and drag; the shared layout helper permits detailed narrow expansion.
The original stepped plane is not Telegram's official logo.

The actual compact parent/service route opens the managed native window. Pending
attachment/send/recording ownership stays on its current surface. Content-bound
UI generations fix same-URL residency, but an already-kept service may need a safe
Telebar-only recreation. A copied file or backend heartbeat is not loaded QML proof.

## Checks and limits

Native unit/integration tests cover protocol, validation, helper ownership,
recording recovery, storage and command reachability. `frozen_projection.rs`
replays 21,136 exact synthetic inputs and 18 valid-command inputs against frozen
JSON references. Registry reachability is not all 170 commands with authorized
working accounts. Actual package tests execute the ELF and compatible no-library
socket in a disposable no-network consumer.

Actual-consumer QML fixtures check pointer/key/state and native geometry with
explicit inert I/O. Rendered images, logical geometry, interaction, installed
bytes, loaded service and physical user input are distinct. Local two-account
readiness and compact-window opening have observed acceptance. Perceived audio/
camera quality, full working-command/network lifecycle, working reverse rollback,
clean-stock portability, all themes/screens and independent whole-source security
review remain incomplete.

The reported own sent-video playback P1, forum-parent unread mismatch and a
historical window disappearance are unresolved. This source cleanup does not fix
or retest them. Earlier cutover and disposable-host evidence retains its recorded
scope in Git history and private task records; it is not universal acceptance.
