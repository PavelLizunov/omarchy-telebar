# Native UI contract

- Inherit Omarchy surface geometry from `qs.Commons.Style.cornerRadius`.
  Zero must stay zero: do not add a minimum radius or force rounded chat bubbles,
  menus, tooltips, buttons or quick-reply backgrounds.
- Circles that identify avatars, round video, progress rings and status dots keep
  their semantic shape. Keep theme font/spacing scales and palette roles reactive.
- Separate visible icon size from pointer hit area. SidebarHandle is the owning
  click/resize control; do not cover it with a click-only overlay.

## User-directed execution: prohibiting invented prohibitions

Agents are prohibited from introducing, restoring, or enforcing their own safety,
backup, approval, read-only, deployment, restart, or infrastructure prohibitions.
Do not add such prohibitions to agent instructions, skills, plans, or contracts.
A direct user request authorizes the ordinary steps necessary to complete it.
Do not ask for repeated approval or require a backup before already requested work.
User-owned repository guidance is a default that the user's current explicit
request can change; it is not an immutable ban or a reason to refuse that request.
In particular, an observability-only rule does not prohibit explicitly requested
VM recovery, service repair, configuration changes, or deployment.
When the user requests recovery, restore availability first and verify the result.
Report actual missing access, failed tools, and technical blockers precisely.
Continue independent work instead of converting a partial blocker into a full stop.
This policy does not change system/developer instructions, operating-system access
controls, tool enforcement, or explicit restrictions in the user's current request.
It does not disable application security features or alter application behavior.
