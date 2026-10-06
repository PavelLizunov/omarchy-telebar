# Telebar backend direction

Status: proposed Rust direction and implemented Python/QML corrective contracts, not a running Rust backend or stable-release acceptance. See CHANGELOG.md for the published checkpoint scope.

## Workspace and scope

Develop in the Telebar source checkout, not the installed plugin copy.
This independent checkout was copied from the published Telebar snapshot
`8fd406d4872d5fd56b29ebfb1afd2871bfad12a2`, including the original Git history
and existing local work. The old source checkout was removed after a fresh full
comparison and explicit owner confirmation. Active audit checks use this checkout;
historical copy/removal evidence remains outside Git.
Read the workspace and local `AGENTS.md` before changes. Do not run plugin
launchers from this checkout as tests: they can access the shared live profile.

The installed plugin is a separate deployment target. Neither this directory
nor its distinct name isolates account databases. There has been no installation,
service restart or database migration for this design. Publishing a source checkpoint does not deploy it.

## Decision: extend existing contracts

Keep QML/JavaScript inside the existing Omarchy/Quickshell host. The preferred
backend target is one separate Rust executable calling official TDLib through
its C JSON API. Preserve the current Unix-socket interface initially. Do not add
another service between Rust and TDLib, replace MTProto, or load TDLib into the
shared shell process.

The user prefers Rust. No measurements establish Python as the main bottleneck;
no performance improvement is claimed. Rust has no tracing garbage collector,
but QML JavaScript remains managed, and TDLib retains its own resource costs.
Bounded collections and correct lifecycle contracts are still necessary.

Alternatives: keeping Python is the lowest-risk immediate path to correcting
confirmed defects. C++/Qt or a Rust Qt bridge may become useful if measured
native-model/serialization costs justify them, but currently add packaging and
runtime coupling without demonstrated benefit. This is architectural advice,
not an independent source-audit result.

## Compatibility baseline

The source registry contains 168 commands with unique existing handlers.
Current owning sources are `bin/omagramd`, `bin/omagram_td.py`,
`app/OmagramClient.qml`, `shell/Service.qml`, and `bin/omagram`.

- Requests: newline-delimited UTF-8 JSON with `id`, `cmd`, `args`, and optional
  top-level `account`. Existing routing prefers `args.account`, then top-level
  `account`, then the selected account. Preserve precedence until a versioned
  contract explicitly changes it.
- Success: `{"id":1,"ok":true,"result":{}}`. Failure carries `ok:false`,
  `error`, and an existing optional/error code. Unsolicited updates use `event`.
- The current socket is `$XDG_RUNTIME_DIR/omagram/omagram.sock`. The daemon checks
  same-user peer credentials and uses private runtime/socket permissions.
- Existing input limit is 64 KiB; output backlog limit is 16 MiB per client;
  maximum connected clients is 16. These are baseline constants, not evidence
  that all internal queues are bounded. Output frame limits require separate
  sizing for history/snapshots; do not impose the input limit on outputs blindly.
- Preserve account/data/config/keyring identifiers, runtime names, window class,
  media membership and original attribution. Plugin branding alone does not
  isolate session storage.
- Inventory numeric fields before Rust conversion. Preserve safe int53 values;
  existing string-encoded wider IDs remain strings. Do not cast arbitrary int64
  values into QML JavaScript numbers or change all ID types indiscriminately.
- New protocol version/capability metadata must coexist with current `hello`
  consumers. Unsupported commands fail explicitly; partial candidates must not
  masquerade as complete replacements.

## Required ownership and correctness

One service owns each TDLib account database. Startup must acquire the verified
single-instance/database ownership guard before opening sessions. Select one
supported lifecycle policy; do not introduce competing QML and service-manager
owners. Shell/window startup, quit, disable, reconnect and parent-exit behavior
must have compatibility checks rather than an assumed systemd requirement.

Each pending operation has an owner, account/session generation, finite
admission budget, deadline and terminal cleanup. Suppress stale state mutation,
not resource finalization. A disconnected peer may not receive a reply: require
one internal terminal outcome and at most one delivered terminal response.
Local expiry is not TDLib cancellation. Unknown send outcomes require
reconciliation, not automatic retries that can duplicate messages.

Preserve unsent text, captions, attachments and reply context on rejected sends.
Delayed results must not overwrite newer edits or another chat/topic/account.
All export paths require backend-authoritative permission and file/message
membership checks; the existing photo-only checker is not a universal media
export contract. Topic changes invalidate pending mention navigation. TDLib
parameter rejection must reach the authorization error state.

Keep credentials and private message text out of argv, diagnostic logs and test
fixtures. FFI must copy returned JSON within the documented pointer lifetime,
validate bytes and bounds, and serialize receiving as required by TDLib.
Shutdown and worker completion need an explicit stop/join/cleanup contract.

## Implementation and acceptance sequence

1. Finish the source audit and describe command/result/event contracts at the
   fixed base. Use synthetic fixtures for the six confirmed defects D-005
   through D-010. Corrective implementation remains a separate authorized batch.
2. Evaluate a minimal Rust transport/TDLib boundary against those contracts in
   isolation. Select standard threads/multiplexing versus an async runtime from
   actual I/O/shutdown needs; neither Tokio nor an actor framework is mandatory.
   Do not add dependency/code-generation infrastructure speculatively.
3. Compare a bounded vertical slice and ultimately all supported commands using
   inert differential checks. A slice is a migration mechanism, not a reduction
   of the required feature set. Builds/full suites require an approved worker/CI.
4. Measure finite workload-matched idle CPU, memory, request processing and UI
   delays separately from network and TDLib latency. Agree bounds before claims.
5. Before any separately authorized candidate deployment, verify old-owner exit,
   consistent closed-database backups, same pinned TDLib and reversible switching
   on disposable data. Same-version use alone does not prove rollback safety;
   never ordinary-copy an open database or run both backends against it.

QML preview remains offscreen with reviewed actual consumers and inert models;
Rust is not required to render those fixtures. Render/image/design/interaction,
backend contract, live-account and performance evidence are separate gates.

## Executable correctness contracts: local P1 candidate

The working checkout now includes a bounded Python/QML corrective candidate, not
an installed release or Rust implementation. Published-base audit reproductions
remain immutable; use the candidate checks below for corrected expectations.

`file.save` requires `chatId`, `messageId` and `fileId` before file lookup. The
backend checks the returned file ID, message identity, normalized original media
membership and explicit `can_be_saved: true` from TDLib before copying. Missing,
unknown or denied permission fails closed. Photo, document, video, audio, voice,
video-note, animation and sticker saves share this check. Photo clipboard export
retains its stricter photo-only contract, including rich-post membership/full fetch.
Legacy context-free socket callers now receive an error; in-tree callers supply
context. This is an intentional permission tightening, not unchanged wire behavior.

Main text/file sending retains text, caption, attachments and reply metadata until
success. One request may be pending per composer. Failure releases that admission
without clearing composition. Success clears only the same account/chat/topic/
thread and unchanged composition revision. Chat/topic/account lifecycle changes
invalidate completion; edits, including edit-and-revert, prevent stale clearing.
No automatic retry occurs. The existing three-minute client callback expiry remains
the timeout policy; expiry does not prove Telegram cancelled an accepted send.
Message editing and recording-send retry semantics are outside this P1 batch.

Bounded executable acceptance:

```sh
python3 -B tests/export_contract_test.py
python3 -B tests/daemon_test.py MessageActions.test_files_open_save_and_clipboard_images MessageActions.test_viewer_photo_exports
python3 -B tests/rich_test.py RichDaemon.test_rich_photo_export_checks_membership_and_permissions
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_send_contract.qml -import tests/visual/imports -import tests/visual/imports/inert
```

The bundled inert adapters include versioned Commons provenance and licensing;
see tests/visual/imports/README.md for unsupported host behavior.
`SendContractConsumer.qml` renders actual ChatView pending/rejected states with
inert requests. Evidence is under the external audit directory's `source-fixes-p1/`.
These checks do not establish live Telegram permissions, server draft behavior,
independent security review or release readiness.

## Mention and parameter-startup contracts: local candidate

Mention navigation is invalidated at chat reset, topic/thread transition and
account leave. Invalidation clears pending message/busy state and advances the
request serial. Returning to the same topic does not revive an old completion;
forum-topic and reply-thread contexts with the same numeric ID remain distinct.
Only the current context may focus and read the resolved mention once.
Reaction navigation uses the same lifecycle invalidation paths with a separate
request serial, captured topic/thread/account context and exactly-once completion.
Its read request retains topic/thread identity; a stale empty result cannot clear
unread reactions in a replacement context.

Each parameter-startup attempt records the owning account object, TD client ID
and an attempt serial. Success retains the existing bridge-ready behavior only
for that attempt. Rejection sets a bounded authorization error only while the
same attempt and authorization-state object are current. The reason includes a
bounded numeric code and generic corrective guidance, never the raw TD error
message or submitted parameters. Another account, a replacement client, a newer
attempt or a progressed authorization step must not be overwritten. The existing
explicit credentials submission can retry; there is no automatic restart/logout.

```sh
python3 -B tests/auth_contract_test.py
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_mention_contract.qml -import tests/visual/imports -import tests/visual/imports/inert
```

Evidence: external `source-fixes-context/`. The backend event and IPC snapshot are
checked with synthetic TDLib replies. Main.qml already renders auth.reason as plain
text on its error status view; the complete Main.qml shell/window is not rendered
by the offscreen item fixture. MentionContextConsumer renders actual ChatView after
a topic transition. Live Telegram read counters and production sign-in remain
unverified. The next section describes the subsequently implemented local shared-lifecycle
candidate for D-005 and D-008; no live deployment is implied.

## Shared request and background lifecycle: local candidate

Tracked TDLib requests are capped at 1024. Each IPC connection admits at most 512
pending commands; reuse of an outstanding request ID closes that ambiguous connection.
The daemon event loop independently expires both TD and IPC entries after 180 seconds.
A timeout retires local callback state, not the operation inside Telegram. Errors say
that the operation may still complete and instruct checking before retrying; no automatic
retry is performed. Monotonic deadlines do not rely on QML heartbeat timing.

IPC work carries a daemon-issued request token across nested TD and background callbacks.
A terminal reply consumes that token. A late response or background result cannot submit
follow-up business work, emit a second response, or satisfy a newly reused request ID.
Original account-object identity and TD client generation are checked separately.

Background and resource-owning TD operations have a separate stale cleanup callback.
It releases bridge admission, removes only generated recording/profile/clipboard output,
and never invokes business completion on a replacement session. Account closure retires
owned IPC work and pending callbacks. Bridge metadata and original user upload files are
preserved. Existing helper work runs to its bounded completion; this is not native TDLib
request cancellation or a guarantee that an already submitted send was cancelled.

```sh
python3 -B tests/lifecycle_contract_test.py
python3 -B tests/event_loop_test.py
```

Evidence: external `source-fixes-lifecycle/`. Direct source and deterministic inert tests
cover timeout, overload, transport exception, duplicate/late response, original account,
replacement account object, nested callback context, reused IDs, profile/voice/video
cleanup and bridge enable/disable lock release. A bounded read-only Gemini acceptance
analysis exposed missing IPC retirement and callback-token coverage in an intermediate
candidate; the coordinator confirmed and corrected these with executable regressions.
It is not a full independent source/security audit or live acceptance.

## Evidence and next step

Detailed audit logs, private workspace migration records and consultation evidence
stay outside Git. CHANGELOG.md records the public checkpoint and outstanding
acceptance gaps. Tool-free architectural advice did not independently inspect
this repository or authorize implementation.

The isolated corrective suite now passes with unchanged worker resource limits.
Bundled visual imports remove the temporary external adapter dependency; actual
consumer QtTest and MCP evidence remain separate. The inert CI workflow runs on pushes and pull requests;
its definition alone is not a hosted exact-commit result. Continue remaining command-family/source-range
coverage and final differential review without another routine approval prompt.
No Rust code has been added and no live candidate acceptance is claimed.
