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
