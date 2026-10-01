# Full codebase review plan

## Scope and evidence

Review the complete current Omagram worktree, manifests, launchers, Python backend,
QML window/shell components, JavaScript models, tests, documentation, assets and
vendored transport ownership. Include Ponytail simplicity review. No automatic
remediation, dependency installation, publication or live service changes during
the audit. Direct inspection is not independent review.

Initial mechanical inventory (2026-09-30, prior to this plan): 134 source/doc files,
36,507 lines excluding .git and vendor. app 35, bin 14, shell 8, tests 74, root 3.
These numbers prioritize reading; they are not findings. Binary assets, JSON data,
vendored sources and new files must be inventoried separately for full coverage.
Largest: omagramd 4,256; ChatView 3,280; daemon_test 2,674; QuickView 1,988;
Model.js 1,864; omagram_state.py 1,630; SettingsView 1,319; Main.qml 1,146 lines.

Before reading, record HEAD, dirty source hashes, installed/source drift and safe
test commands. Preserve user work, accounts and private logs. Track every reviewed,
excluded and unexamined file; capped searches do not establish complete coverage.

## Ordered passes

1. **Entrypoints and ownership:** manifest, launcher, install scripts, shell service,
   IPC, account sessions, workers/helpers, restart/quit, subscriptions and cleanup.
2. **Backend:** TDLib/state routing, stale replies, settings merge/migration, errors,
   security boundaries, media paths, resource limits and telemetry privacy.
3. **Window UI:** list/search/topics/accounts, composer/actions, file chooser,
   settings/auth, dialogs, keyboard/focus, reactive theme geometry and sizing.
4. **Shell UI and media:** compact/wide reply, bar popup lifecycle, viewer loading,
   photo/video/play/download routes, accessibility and actual-consumer imports.
5. **Models and hot paths:** algorithms, copies/sorts, repeated lookups, batching,
   I/O on UI/event threads, polling, decode/cache limits. Separate measured hotspots
   from hypotheses; do not optimize from LOC or syntax alone.
6. **Ponytail:** proven dead code/files/dependencies, duplicated implementations,
   speculative abstractions, commented-out code, needless configuration. Verify
   dynamic QML entrypoints and IPC consumers before labeling a symbol unused.
7. **Tests:** map behavior to tests; identify gaps, misleading fixtures, brittle
   source assertions and redundant implementation-mirroring checks. Exercise real
   control entrypoints and first-open/default state, not only methods with seeded
   state. Preserve useful regression checks; recommend additions/deletions with cost.
8. **Docs, comments and dependencies:** check explanatory value and accuracy,
   stale claims, licenses/attribution, supported Qt/native contracts and commands.

## Regression seeds

- FilePicker first open failed because StandardPaths returned a URL and code
  called .split on it. Seeded-folder tests concealed this.
- Qt 6.11.2 fallback OpenFiles behaved as single-file selection.
- MediaViewer's own Icon lacked a local import; full viewer loading needs a check.
- Constrained QML layout overflow passed interaction-only tests.
- QuickView conditional vertical anchors retained invalid placement when switching
  between stacked/inline tools; test directed size transitions and control hit bounds.
- Slow callback/UI timer outliers need handler attribution; passing batch tests
  and idle telemetry do not establish responsiveness or fix all stalls.
- Configured MCP wrapper/renderer mismatch blocks native visual acceptance.
- Plugin rescan left Omagram without a daemon socket; window and daemon were
  restored as managed user units. Review host/service ownership and restart
  transitions before claiming plugin reload is reliable.

## Findings and acceptance

For every finding provide file:line, severity, observed impact, caller/data flow,
counterevidence, reproduction/check, smallest correction and confidence/limits.
Group confirmed defects separately from speculative performance/design concerns.
Do not recommend splitting a file solely because it is long or deleting a comment
solely because it is verbose.

Deliver a coverage matrix, prioritized findings, test-gap/removal ledger, measured
performance evidence and an ordered remediation plan. Each remediation batch needs
explicit scope and executable acceptance criteria before implementation. The audit
does not authorize fixes or claim release readiness. Never publish personal logs/core.
