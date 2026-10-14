# Telebar

An unofficial Telegram client for Omarchy. Reply from the bar, use a native window,
and keep Omarchy's theme and keyboard controls. The application runtime and helper
commands are Rust; the interface is QML/JavaScript with Quickshell and official TDLib.

[Download the native prerelease](https://github.com/PavelLizunov/omarchy-telebar/releases)
 · [Architecture](ARCHITECTURE.md) · [Runtime contract](rust/PARITY.md)

Main contains the native source, tests and tooling. It is not an installation
package until built. The current release remains 1.3.0-rc.1, not stable-release
certification. Reported sent-video playback, forum-parent unread state and a
historical disappearing window remain unresolved. Full command, perceived media
quality, working-account reverse rollback and independent security acceptance are
incomplete. The installed compact-to-window opener has user acceptance.

## Requirements

Omarchy, Hyprland 0.56+, Quickshell and a running systemd user manager. Runtime tools:

```sh
sudo pacman -S --needed qt6-multimedia qt6-multimedia-ffmpeg qt6-lottie libsecret qrencode ffmpeg pipewire-audio wl-clipboard xdg-utils openssl
```

TDLib 1.8.67 is built into `~/.local/share/omagram/lib/` from pinned official commit
`d1085f9cebc5a62379991ae1652673954f229c1f`; it is not bundled. Its build needs Git,
CMake, gperf, clang, OpenSSL and zlib. Rust 1.88+, Cargo, OpenSSL development files
and pkg-config are build requirements. Node.js and gzip are development/test tools,
not application runtime requirements. Clean-stock portability and the separate
local Qt 6.12 host recovery are not certified by repository checks.

## Build and install

From main, select new output directories outside this checkout and the watched
plugin tree:

```sh
export CARGO_TARGET_DIR="$HOME/.cache/telebar-native-build"
cargo build --manifest-path rust/Cargo.toml --release --locked
"$CARGO_TARGET_DIR/release/telebar-runtime" --package "$PWD" "$HOME/telebar-native-base"
node scripts/native-ui-generation.mjs "$HOME/telebar-native-base" "$HOME/telebar-native-package"
omarchy plugin validate "$HOME/telebar-native-package"
```

A generated package contains one Rust ELF, five Bash entry shims and native UI.
Content-bound QML entry points avoid relying on same-URL cache freshness. The
package manifest records exact input hashes. Building or validating does not
install, enable or restart anything. Downloaded x86-64 native release archives
are already built; GitHub Source code archives are source, not plugin packages.

For a fresh installation with no Telebar or Omagram owner:

```sh
mkdir -p "$HOME/.config/omarchy/plugins"
test ! -e "$HOME/.config/omarchy/plugins/io.github.pavellizunov.telebar"
test ! -L "$HOME/.config/omarchy/plugins/io.github.pavellizunov.telebar"
mv -T -- "$HOME/telebar-native-package" "$HOME/.config/omarchy/plugins/io.github.pavellizunov.telebar"
omarchy-shell shell rescanPlugins
omarchy plugin enable io.github.pavellizunov.telebar
~/.config/omarchy/plugins/io.github.pavellizunov.telebar/bin/omagram-build-tdlib
```

For an update, save pending window/quick compositions. Stage code outside the
recursive plugin watcher and reconcile installed local changes; never copy live
databases or replace profiles, keys, settings or retained media. A backend change
requires the old owner to close clients and retire its process/socket before the
same profiles are reopened. Check the original exclusive lock, installed hashes,
loaded ELF, account readiness and UI connections. Plugin disable alone is not
proof of graceful TDLib shutdown.

A QML-only update retains the Rust owner and installed executable inode. An
already-kept old QML service may still require Telebar-only disable/enable after
its pending work is saved. Preserve original bar placement and separate host
recovery adaptations. Check `io.github.pavellizunov.telebar.action status` in the
existing shell: `launcherRuntime` must be `rust-native-v1`. A successful copy or
heartbeat alone is not loaded-generation evidence.

The plugin runs unsandboxed with your user permissions in the shared shell.
Network traffic goes to Telegram; helper commands, notifications, clipboard and
local media require the corresponding session access. No boot service is enabled.
Do not run Telebar and Omagram together against the same session directories.

## Use

Create your API ID/hash at [my.telegram.org](https://my.telegram.org). Open Telebar
from the app launcher or the bar menu, then sign in by phone or QR. Credentials
and database encryption keys use the session keyring. Existing `omagram` paths,
command names, window class, desktop entry and account keys remain compatible.

Left-click the plane to open the quick view; right-click for Open Telebar, mute
notifications or quit. Its Open Telebar label/mark opens the native window.
Attachments, sends or recordings pending in the compact panel stay owned there;
opening a window does not transfer those attachments. The unread dot is separate
from the mark. Quiet mode leaves unread indication intact.

Common window controls: Ctrl+K searches, Ctrl+, opens Settings, Ctrl+I shows chat
info, Ctrl+O attaches media, Ctrl+R records voice and Ctrl+Shift+R records round
video. Enter sends, Shift+Enter inserts a line, Escape cancels the current action.
Ctrl+O in the quick view opens its chat in the window. Shortcuts are configurable
in Settings; global bindings only remove entries owned by Telebar.

Chats, folders, topics, rich posts, photos, ordinary/round video, stickers and
recorded-message recovery use the existing native UI. Calls need another Telegram
app because TDLib provides signalling but no voice engine. The sidebar handle
owns both toggle and resize, including detailed-list expansion in narrow windows.
Theme radius zero stays zero; avatars and round video remain semantic circles.

## Checks

```sh
cargo fmt --manifest-path rust/Cargo.toml --check
cargo test --manifest-path rust/Cargo.toml --locked
cargo clippy --manifest-path rust/Cargo.toml --all-targets --locked -- -D warnings
node scripts/check-source.mjs
for test in tests/*-test.js tests/*-test.mjs; do node "$test"; done
bash scripts/check-native-package.sh "$HOME/telebar-native-package"
```

Native tests replay 21,136 frozen projection inputs plus 18 valid-command inputs,
with exact JSON values/types and array order. Host/device/native acceptance cases
are explicitly ignored by default; they are not hidden passes. Package checks use
an isolated no-library socket, native proxy and owned helpers, not working accounts
or Telegram mutations. [Visual fixtures](tests/visual/README.md) check actual QML
with inert I/O. Hosted [Actions](https://github.com/PavelLizunov/omarchy-telebar/actions)
run native and shared-UI contracts on the exact commit. No independent whole-source
review or all-screen/all-theme acceptance is implied.

## Privacy, diagnostics and removal

The backend socket is mode 0600 in a private runtime directory with same-user peer
checks and a single-owner lock. Database/catalog state lives under
`~/.local/share/omagram`; settings under `~/.config/omagram`. Generated recordings
and uncertain inputs are retained rather than silently resent or age-deleted.

Numeric local diagnostics rotate under `~/.local/state/omagram/diagnostics/`.
They omit message bodies, credentials and user paths. UI heartbeats are not GPU
frame timing or exact-source attestation. Use the managed unit logs and private
`diagnostics.status` route to investigate failures.

Quit first, then remove the plugin using Omarchy's controls. This does not sign out
an account on Telegram or delete its data/keys. Keep account storage unless you
explicitly intend to lose local sessions, downloads and secret chats. Terminate a
remote session in Telegram Settings → Devices when needed.

## Attribution

Independently maintained by [PavelLizunov](https://github.com/PavelLizunov), based on
[Omagram by ReidenXerx](https://github.com/ReidenXerx/omarchy-omagram). MIT; original
copyright remains in [LICENSE](LICENSE). [Third-party notices](THIRD_PARTY_NOTICES.md)
cover TDLib, Unicode data and the adapted Flowseal transport. This is not made,
endorsed or supported by Telegram. Unofficial-client use is subject to Telegram's
API/account restrictions; an API ID is not a guarantee against account limits.
