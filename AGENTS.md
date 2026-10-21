# Telebar development

- Read [TESTING.md](TESTING.md) before choosing a test host or reference implementation.
- Default runtime target: dedicated `omarchy-test` VM107, reached through the
  existing DSH SSH route. Run `bash scripts/test-host-preflight.sh` to verify it.
- Use the workstation for editing/control and final installed-consumer checks;
  use the VM for exploratory windows, pointer input, captures and shell lifecycle.
- Read upstream behavior before inventing a workaround. TDLib owns protocol/auth;
  Telegram Desktop is a behavioral reference, not a drop-in QML player.
- A defect task includes diagnosis, the smallest justified repair and consumer
  verification. Routine tests/activation are part of the request, not new approvals.
  Report exact technical limits; a failed branch does not stop independent work.

# Native UI contract

- Inherit Omarchy surface geometry from `qs.Commons.Style.cornerRadius`.
  Zero must stay zero: do not add a minimum radius or force rounded chat bubbles,
  menus, tooltips, buttons or quick-reply backgrounds.
- Circles that identify avatars, round video, progress rings and status dots keep
  their semantic shape. Keep theme font/spacing scales and palette roles reactive.
- Separate visible icon size from pointer hit area. SidebarHandle is the owning
  click/resize control; do not cover it with a click-only overlay.
- Verify visible changes with actual-consumer inert QML fixtures, both radius 0
  and nonzero, plus the relevant interaction tests. Do not change the live theme
  or restart the desktop to obtain preview evidence.
