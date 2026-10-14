# Telebar Rust runtime

One standalone application runtime owns TDLib, IPC, launcher/helper operations and
accounts. Quickshell and native QML/JavaScript remain the interface. The compatible
executable names, storage, keyring and socket identities are preserved.

## Build and test

Rust 1.88+, Cargo, system OpenSSL development libraries and pkg-config are required.
Keep build outputs outside the checkout. The frozen projection tests use gzip;
shared-UI/tooling tests use Node.js. Neither is an application dependency.

```sh
export CARGO_TARGET_DIR="$HOME/.cache/telebar-native-build"
cargo fmt --manifest-path rust/Cargo.toml --check
cargo test --manifest-path rust/Cargo.toml --locked
cargo clippy --manifest-path rust/Cargo.toml --all-targets --locked -- -D warnings
cargo build --manifest-path rust/Cargo.toml --release --locked
"$CARGO_TARGET_DIR/release/telebar-runtime" --package "$PWD" "$HOME/telebar-native-base"
node scripts/native-ui-generation.mjs "$HOME/telebar-native-base" "$HOME/telebar-native-package"
bash scripts/check-native-package.sh "$HOME/telebar-native-package"
```

Add `--offline` to Cargo commands when locked dependencies are already fetched.
Native defaults retain ownership/validation/recording regressions and replay the
21,136-input frozen corpus plus 18 valid-command inputs without a language oracle.
JSON numbers remain in serde_json, avoiding a test-driver precision normalization.
These are finite synthetic references, not all 170 working-account routes.

The package/shim/socket test runs in a private read-only, no-network Bubblewrap
consumer with a disposable no-library owner. It checks real ELF execution, proxy
EOF teardown, startup-environment refusal, hello/settings ordering, diagnostics
redaction and normal quit. It never opens working profiles or sends parameters.

Host/device, private Secret Service, media/native ABI and working-network cases
are explicitly ignored unless their named disposable environments exist. Removed
fixture drivers do not imply those cases were rerun or accepted. Native wrappers
for preauthorization parameters, disposable proxy and notifications remain under
`tests/run-*.sh`; inspect the wrapper and exact fixture before invoking it. The
separate Qt recovery is not part of this crate.

## Package and application

`--package <absolute-source> <new-absolute-output>` admits regular owned native
files before creating output. The package contains UI, notices, one Rust ELF and
five Bash shims. Existing/linked/substituted outputs and unapproved file types are
refused. `runtime-candidate.json` records hashes and launch readiness.

`--launch`, `--host-window`, `--daemon`, `--menu`, `--build-tdlib` and `--proxy`
implement the compatible entry contracts. The user manager owns window/backend
processes; each job is submitted once and exact completion is observed. A busy or
uncertain result is not permission to resubmit a mutation. The original database
lock prevents simultaneous owners. Root-owned tool/keyring checks remain intact.

The compile-time launch gate is enabled. It is not an acceptance flag: diagnostics
retain `migration_complete=false`. Local installation has two original ready
accounts and accepted compact window opening, but complete manual/audio/network/
reverse-rollback/security certification is not claimed. See [PARITY.md](PARITY.md)
and the root [installation guide](../README.md).
