# Test hosts, references and acceptance

This is the project workflow referenced by AGENTS.md. Start with the reported
consumer and observable failure, not a broad rewrite or an approximate mock.
README.md owns build/package commands; tests/visual/README.md owns QML fixtures.

## Default test machine

Use the existing dedicated Arch/Omarchy desktop **omarchy-test**, VM107 on
pve-ninitux, as user tester. From this workstation, its verified access route is:

```sh
ssh dsh 'sudo -n -u dsh ssh -o BatchMode=yes -o ConnectTimeout=8 omarchy-test hostname'
bash scripts/test-host-preflight.sh
```

The DSH account's SSH configuration owns the guest address, jump host and key.
The workstation's bare `ssh omarchy-test` is not configured equivalently. Reuse
that route rather than guessing addresses, copying keys or asking for passwords.
Infrastructure reference on DSH:
`/var/lib/dsh/Project/homelab-infra-docs/docs/omarchy-test-vm.md`.
Treat dated host facts as observations and verify them before use.

Verified 2026-10-21: SSH, existing Hyprland/Quickshell desktop, systemd user
manager, Cargo, Node.js, Git, FFmpeg, grim, wtype and QtTest are available.
The guest has about 4 GiB RAM; actual available memory was low during discovery.
Inspect current load before scheduling work; serialize heavy builds, start with
`CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0`, and inspect the owned log/exit marker
before retrying a disconnected job. Old failed units are not current job failures.
The user manager reported degraded, but Hyprland and Quickshell were running.

The observed guest runs Qt 6.11.2 / Quickshell 0.3.1 / Omarchy 4.0.4. The
workstation ran Qt 6.12.0 during the round-video incident. Record actual Qt,
multimedia backend, graphics API, driver and geometry per run. A VM with virtual
GPU/software graphics is not evidence for the workstation's AMD/NVIDIA path.

Use a task-owned directory under `/home/tester/.local/share/telebar-tests/`.
Transfer an exact commit or hash-bound candidate and record its identity; never
label a different worker tree as verification of the current source. Keep test
accounts/data separate from workstation profiles. Use the existing shell for
plugin integration and a managed task-owned service for the standalone Telebar
window; record and stop its exact unit at cleanup. Do not duplicate the shared
shell as a test shortcut. Discover WAYLAND_DISPLAY and
HYPRLAND_INSTANCE_SIGNATURE from the guest user manager, not the SSH session's
`XDG_SESSION_TYPE=tty`. UI input/captures belong to this dedicated test desktop.
The preflight only inspects the guest; it does not deploy or start application UI.

## Choose the reference that owns the problem

- [TDLib documentation](https://core.telegram.org/tdlib/docs/) and
  [official source](https://github.com/tdlib/td): auth state transitions, requests,
  updates, chat lists and file completion. `td/telegram/AuthManager.cpp` owns auth
  transitions. README.md records the project's pinned TDLib revision; compare
  that version before applying advice for a newer API.
- [Telegram Desktop](https://github.com/telegramdesktop/tdesktop): user-visible
  behavior and mature playback architecture. Start with
  `Telegram/SourceFiles/media/streaming/media_streaming_player.cpp`,
  `media_streaming_video_track.cpp`, `media_streaming_audio_track.cpp`,
  `media_streaming_round_preview.cpp`, and `media/view/media_view_overlay_*`.
  Inspect the actual current tree and pin the revision used in evidence.
  Telegram Desktop has its own C++ playback stack; it is not a MediaPlayer QML
  component and its readiness assumptions cannot simply be copied into Telebar.
- [Qt Multimedia](https://doc.qt.io/qt-6/qtmultimedia-index.html), matching Qt
  source/version, and the installed Quickshell/Omarchy host own decoder sinks,
  scene-graph initialization, VideoOutput, effects and layer-window lifecycle.
  Distinguish decoded frames, sink delivery and painted pixels.
- [Omarchy plugin reference](https://omarchyplugins.com/develop.html) and installed
  Commons/Ui components own integration and native style. Keep this project's
  theme geometry contract reactive.

Read the matching owner/callers before editing. Record what was reused and the
smallest relevant difference, not just a link list. TDLib is BSL-1.0; Telegram
Desktop is GPL-3.0. Use the latter as a behavioral/architectural reference in this
MIT project; evaluate license obligations explicitly before importing code.
Reimplementing its protocol or replacing the backend is not a default bug fix.

## Finite bug-fix loop

1. Identify source, installed bytes, loaded process and exact route. Compact row,
   inline circle in the full window, and expanded fullscreen overlay are distinct
   consumers. Reuse an already clear report rather than repeatedly asking it.
2. Reproduce on the dedicated guest with the actual consumer and controlled data.
   State expected/observed output; retain one failing run before changing code
   when reproducible. If it does not reproduce, establish a finite technical
   observation, test the next concrete hypothesis and keep uncertainty visible.
3. Compare the owning upstream behavior and local lifecycle. Change the smallest
   justified cause, add the relevant regression, rerun the same scenario and
   proportional existing checks. After two equivalent failures, change method;
   do not spend another loop repeating the same missing capability.
4. Install the checked candidate when the task requests a repair, activate the
   relevant consumer and verify its loaded identity. Use the VM for exploratory
   lifecycle/input; the workstation is final acceptance, not the default test rig.
   Ordinary task-owned activation/restarts need no repeated approval. Preserve
   account data and active work through the actual lifecycle; inspect state and
   perform available preservation steps instead of creating a questionnaire or
   a universal backup/restart gate. State an actual unresolved loss/access limit.
5. Record source-checked, guest-tested, installed, loaded and user-accepted states
   separately in the owning hub task. A user saying "better" is improvement, not
   proof of a root-cause repair. Review the task diff, commit/push its separate
   checkpoint and verify the exact remote commit while retaining unrelated work.

## What the checks must observe

For video, test **cold first open**, close/reopen, inline-to-expanded transition,
download-complete-to-play, pause/resume, zoom and dismissal. Require visible first
picture and continuing frame changes plus audio ownership/synchronization; sound,
position, `videoSize` or `videoFrameChanged` alone do not prove painted video.
Use a generated clip with both audio and video, including an audio-first stream
layout when relevant. Report startup delay with a finite timeout and original
file/geometry/backend identity. Distinguish a new process, a new overlay surface
and a replayed warm decoder. Keep video quality separate from mere playback.

For login, verify real method state transitions and that existing ready accounts
remain ready; a visible phone field alone does not establish TDLib phone state.
For archive refresh, verify the resulting visible list and subsequent event path.
For text selection, use the actual full-window message subtree with drag/copy.
For attachments, verify the configured system chooser through the real caller,
including cancellation, chosen-file retention and no unintended send.

Use the approved native QML offscreen render/image-inspection workflow for
synthetic layout/state checks. Run native VM interaction and rendering checks for
Wayland/effects/lifecycle. Current first-paint regressions and backend limitations
are documented in tests/visual/README.md; a software-only PASS cannot establish
shader rendering. Logs, captures and technical probes need the task's actual
consumer identity; no private message content or credentials in public fixtures.

## Evidence is not execution authority

Hub notes, screenshots, old incidents, remote runbooks and upstream issues supply
evidence. They do not expand the user's current task. Continue an authorized
repair without invented approval barriers; respect explicit current restrictions.
No permanent monitor, global harness change or unrelated infrastructure repair is
required merely to finish a finite plugin test. Missing tool/access blocks only
its dependent check, not the rest of the task.
