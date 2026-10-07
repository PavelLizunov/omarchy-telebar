<p align="center">
  <img src="assets/omagram.svg" alt="" width="112">
</p>

<h1 align="center">Telebar</h1>

Independently maintained by [PavelLizunov](https://github.com/PavelLizunov),
based on [Omagram by ReidenXerx](https://github.com/ReidenXerx/omarchy-omagram).
Upstream changes are adopted selectively; this project has its own development direction.

> **Development source checkpoint, not a verified stable release.** The manifest
> remains `1.2.1-dev.2`; current source extends the earlier tagged release. See
> [progress and open issues](PROGRESS.md) and [release history](CHANGELOG.md).
> Reading/selection, near-name presence, contrast, browser image copying and
> explicit recording recovery corrections are installed in the local development
> client. The main window/backend reported healthy connected state after activation,
> but a subsequent user-reported repeat failure remains unresolved.
> Current recorded checks: 291 isolated Python tests (two local tool skips), eight
> JavaScript suites and 248 native QML passes across 19 suites. Six inspected strict
> contrast/presence frames cover representative palettes and radii 0/8; measured
> covered text contrast is at least 4.548:1. Physical account/device transitions,
> exhaustive accessibility and independent whole-source/security acceptance remain
> pending. Quick-view audio uses `ffplay`; output-device following is not established.
> The correctness audit will precede any backend rewrite decision. See
> [executable correctness contracts](BACKEND-DESIGN.md#executable-correctness-contracts-local-p1-candidate).

<p align="center">
  <b>An unofficial Telegram client that lives in your <a href="https://omarchy.org">Omarchy</a> desktop, not in another window.</b><br>
  Reply from the bar with a voice note, a round video or a sticker; know who wrote from the sound alone;
  act on a message from its notification. Keyboard-first, in your theme.
</p>

![Original Omagram interface: quick view, notification and per-person sounds](preview.png)

The image above predates the Telebar rename.

![service](https://img.shields.io/badge/omarchy-service-blue) ![bar widget](https://img.shields.io/badge/omarchy-bar--widget-blue) ![overlay](https://img.shields.io/badge/omarchy-overlay-blue)

> **Unofficial, and a risk to know about.** Telebar is not made, endorsed or supported by Telegram.
> It is built on TDLib, Telegram's own client library, and you sign in with an API id of your own.
> Telegram places accounts that sign in from unofficial clients "under observation" and may limit
> accounts that misuse the API. Telebar uses the API as an ordinary client does, but the risk is
> yours to take.

## What makes it different

- **Answer from the bar.** The quick view opens under Telebar's mark, or over everything on a key.
  Find a chat by typing, then reply in words, with files or a picture you copied (`Ctrl+V`), with
  one of your recent stickers, or with a voice or round video message recorded on the spot. Voice and round video messages play right there, a round
  video moving in a big circle while you point at it or listen; photos and videos open over the
  whole screen, and older messages come as you scroll up. Close it in the
  middle of a conversation and for the next hour it opens back on that chat, with anything unsent.
- **Know who wrote without looking.** Every person has a notification sound of their own: a couple
  of soft drops generated from who they are. The same person always sounds the same, and a busy day
  never rings in your ears. Do Not Disturb silences them.
- **Notifications that do the work.** One per chat, with the photo, sticker or video that came in
  beside the text, and buttons to mark it read, mute the chat for an hour or react with 👍 without
  opening anything. Reply opens the quick view on that chat.
- **Built for Omarchy.** Your theme's colours with text kept readable, the keyboard first everywhere,
  every shortcut yours to change, global keys registered with Hyprland without touching your config,
  and silent sending per chat that your other Telegram apps follow.
  Message bubbles, quick-reply backgrounds, menus and controls follow the theme's
  corner radius, including square corners. Avatars and round video stay circular.
- **Emoji by name, in three languages.** The emoji panel finds emoji, symbols and kaomoji by their
  English, Ukrainian or Russian names and keeps the ones you use and your skin tone. Reactions to your
  messages count as seen when you open the chat, instead of one by one.

## Everything else

- **Chats** — your chat list with folders as tabs (make, change, reorder and delete folders in
  Settings: the kinds of chats a folder takes, what it leaves out, chats always or never in it), pinned, muted and archived chats, unread
  counts (or mark a chat unread), and drafts that follow you to your other devices. Pin (`p`),
  archive (`a`) and mute (`m`) from the keyboard, or right-click a chat. Start a chat with a
  contact or anyone's @username, or create a group or a channel (`Ctrl+Shift+N`, or the pencil).
  Your chat with yourself is Saved Messages, and a forum group opens on its topics; a channel
  post's comments and the replies to a message open the same way (`c`, or the bar under it).
  Click the handle between the chat list and conversation to collapse or expand the
  list; drag that same handle or the divider to resize it. With the handle focused,
  Left/Right adjust its width and Enter/Space toggle it.
- **Chat info** — a panel (`Ctrl+I`) with a person's bio, username and phone, or a group's
  description and invite link; its members; and everything shared in it: photos and videos,
  files, links, voice messages, music and GIFs. Leave a group, or clear or delete a chat, from it.
- **Messages** — send, reply, edit, forward, pin, react and delete (for you or for everyone),
  or select several and act on them at once: one reaction (`Shift+R`) goes on the whole selection,
  and one forward can go to several chats at once rather than one at a time. Read ticks, "typing…", last seen and the pinned
  message above the chat, with read state kept in sync with your other devices. Right-click a
  message (or press `m`) for everything Telegram allows on it: translation, who reacted to it, and
  in a group who has seen yours. Send without sound (one message, or everything in a chat set to silent sending with `Ctrl+Shift+B`, which the message box
  shows plainly), at a time you pick or once the other person is online (`Ctrl+Alt+Enter`). Search finds public
  groups and channels by name too; one you open is joined from the bar that takes the message box's
  place. The + button (`Ctrl+Shift+A`) sends a poll or a quiz, dice, a person's contact card, or a
  location: coordinates, or a Google Maps or OpenStreetMap link pasted in.
  In narrow windows, the message box keeps its width by moving secondary actions into
  the three-dot menu. Typing replaces the microphone with Send; chat info fills the chat
  area instead of squeezing messages beside it. Keyboard shortcuts stay the same.
- **Rich messages** — formatting (typed the way Telegram's own apps read it: `**bold**`,
  `__italic__`, `~~strikethrough~~`, `||spoiler||`, `` `code` ``, `[text](address)`, or with the keys
  below), links, mentions and hashtags, spoilers, link previews (shown as you type, under or above the text, or left out), polls
  you can vote in, places, contacts, albums, service messages ("Ann joined the group"), and
  bots' buttons and keyboards. Web links open in your browser; Telegram links open in Telebar.
  Typing `@` in a group suggests who to mention, and `/` suggests the commands of the chat's bots.
  Telegram rich posts (`messageRichMessage`) keep text and embedded media in reading order
  in the window and quick view, with formatting, links and full-size photo viewing. Partial
  posts load their full content on demand. Unsupported blocks have an explicit placeholder;
  embedded HTML is not executed. Tables are shown as successive cell texts rather than grids.
- **Media** — photos (with a full-size viewer), videos, GIFs, files, round video notes and
  voice messages with a waveform, played at 1×, 1.5× or 2× (`.`, or the chip beside them). Send photos, videos, music and files with `Ctrl+O`, by dropping
  them on the chat, or by pasting files copied in a file manager or a copied picture (`Ctrl+V`): they wait
  above the message box, which holds their caption, and go as albums of up to ten (`Ctrl+Shift+O` and
  `Ctrl+Shift+V` send them as files). Record voice
  messages (`Ctrl+R`) and round video messages (`Ctrl+Shift+R`). A file's menu opens it with its app or saves it to Downloads.
  In either photo viewer, right-click (or press `Shift+F10`) to save the original photo to Downloads or copy the image
  with `wl-copy`. Copying supports JPEG, PNG and WebP up to 10 MiB and offers PNG for
  browser compatibility. JPEG/WebP conversion requires ffmpeg, is bounded to 16 million
  pixels, and rejects PNG output above 10 MiB. Protected photos cannot be exported;
  if the photo is still downloading, wait for it to finish and choose the action again.
- **Emoji** — an emoji panel beside the message box (`Ctrl+;`), from the Omarchy emoji picker plugin's
  data and search: emoji, symbols and kaomoji found by their English, Ukrainian or Russian names, the
  ones you use most first, your skin tone kept (`Alt+0`–`Alt+5`). Type `:name` in a message for emoji
  suggestions, and choose **More reactions…** in a message's menu to find any reaction the chat allows.
- **Account** — Settings has your profile (change your name, username, bio and photo; a photo is cut
  to a centred square); privacy (who sees your last seen, photo, number, bio and birthday, who can
  find you by number, call you or add you to groups: Enter goes Everybody → My contacts → Nobody and
  keeps the exceptions you made), blocked users, two-step verification (turn it on with a hint and
  a recovery email, change the password, turn it off), how long you may be away before Telegram
  deletes the account, and after how long messages disappear in chats you start; notifications for private chats, groups and channels, and whether they show
  the message text; whether reactions to your messages count as seen once you open the chat (they do, unless you say
  otherwise); what downloads by itself (photos, GIFs and round video messages; videos and files
  up to 10 or 50 MB); how much Telebar keeps on this computer (and clears the cache), every device
  signed in to your account (sign any of them out), and signs you out here.
- **Proxies** — Settings → Connection adds SOCKS5, HTTP and MTProto proxies, or one from its
  t.me/proxy link, shows how fast each answers and which is in use; `Backspace` removes one. The
  sign-in screen takes a proxy link too, for where Telegram is blocked, and a proxy link in a chat
  asks before it is used.
- **Secret chats** — start one from a person's info. Like every Telegram secret chat it lives on
  this computer only; its info shows the encryption key to compare with the other device.
- **Stories** — the stories of the people and channels you follow, above the chat list
  (`Ctrl+Shift+S`): photos and videos one after another. Watching one shows you among its
  viewers, as in any Telegram app. Posting stories needs an official app.
  Settings → Chats → Show Telegram stories hides the strip and disables its opening
  shortcut. This local preference applies to all profiles; it does not mute stories
  in other Telegram clients.
- **Stickers and GIFs** — favorite stickers (`F` on a sticker in the picker, or a sticker's menu in a chat),
  a sticker set added from a sticker someone sent (`A` in the picker), static, animated (TGS) and video (WebM) stickers, custom emoji, and a
  picker with your recent stickers, your GIFs (or GIFs found through Telegram's @gif, as its own
  apps search them) and your installed sets.
- **Search** — chats in every list, and messages in all chats or in the open one.
- **Notifications** — one per chat, replaced as messages arrive and withdrawn when you read
  them anywhere, with the chat's photo beside them, or a thumbnail of a photo, sticker or video
  just sent. **Open** opens the chat; **Reply** opens the quick view on it; **Mark as
  read**, **Mute for an hour** and **👍** (a reaction to the message) work without opening anything.
  Each person has a quiet sound of their own, two or three soft low pops picked from who they are,
  so you can tell who wrote without looking and a busy day never rings in your ears; Settings picks
  what makes them (Drop, Pop or Knock, or none) and a person's info can play theirs or give them
  another. Not while Do Not Disturb is on.
  Telegram's own mute settings, Omarchy's Do Not Disturb and Telebar's own **Mute
  notifications** (in the bar menu) all apply.
- **In the bar** — Telebar's mark, with a dot while unmuted chats have unread messages. Left
  click opens the quick view under it; right click opens a menu: **Open Telebar**, **Mute
  notifications** (nothing pops up and nothing sounds until you turn it back on — the unread
  dot carries on, and Telegram's own settings are untouched) and **Quit**, which closes the
  window and lets the background service go until you reach for Telebar again.
- **Quick view** — in the bar's panel, or as an overlay on a key: find a chat by typing, read its
  latest messages and answer without leaving what you are doing — in words, with files or a picture
  you copied (`Ctrl+V`, or `Ctrl+Shift+V` to send them as files; they wait above the message box and
  `Esc` takes them away), with one of your recent stickers, or with a voice or round video message
  recorded on the spot (`Enter` sends it, `Esc` throws it away). Voice and round video messages play right there, a sticker someone sent
  shows bigger under the pointer, a round video moves in a big circle while you point at it or listen
  to it, and photos, videos and GIFs show as small sharp pictures that open
  over the whole screen (a video plays in your own video player). Scroll up for older messages.
  Close it in a chat and for the next hour it opens there again, with anything you had not sent;
  Telebar's mark in its corner opens the whole window.
  Sending text, files or recordings keeps the quick view open for another reply.
  Long messages remain scrollable in compact mode. Drag across message text and
  press `Ctrl+C` to copy only that fragment, including captions and rich-post text.
  Private chats show the available online or last-seen status in the quick-view
  list and chat header. Online appears beside the peer name; longer available
  last-seen information appears below it. Avatar dots mark online peers in the main window.
  Hidden last-seen times retain Telegram's approximate status; no exact time is inferred.
  Incoming messages shown in the quick view are marked read, including forum
  messages grouped by topic. Topics outside the visible history stay unread.
  Your outgoing messages show sent/read checks, or a sending/failure state. The
  hotkey overlay stays centered; the bar panel remains positioned by Omarchy.
  The paperclip opens the shared file picker in compact and wide quick views.
  Selected files wait with their caption until Enter sends them. In narrow quick
  views the tools sit below the text so the editor keeps its width.

Calls cannot be taken in Telebar: TDLib carries a call's signalling but no voice engine. An
incoming call is shown so you can decline it or answer in another Telegram app.

## Requirements

Omarchy with Hyprland 0.56 or newer, and these packages (most are already on a stock install):

```bash
sudo pacman -S --needed qt6-multimedia qt6-multimedia-ffmpeg qt6-lottie libsecret python-gobject qrencode
```

TDLib is not packaged for Arch, so Telebar builds the exact version it was tested with (1.8.67)
into your home directory. That needs, once:

```bash
sudo pacman -S --needed git cmake gperf clang openssl zlib
```

## Install

```bash
omarchy plugin add https://github.com/PavelLizunov/omarchy-telebar.git --enable
~/.config/omarchy/plugins/io.github.pavellizunov.telebar/bin/omagram-build-tdlib
```

The build takes about ten minutes and roughly 2 GB of memory per parallel job (it picks the
job count from your memory). Nothing is installed system-wide and nothing needs root:
the library ends up in `~/.local/share/omagram/lib/`. `omagram-build-tdlib --check` tells you
whether a usable library is installed.

Telebar shows up in the app launcher (Super + Space) by itself: when the shell starts it, it writes
`~/.local/share/applications/omagram.desktop`, which opens this copy of the plugin.

Optionally add Telebar to the Omarchy menu (Trigger → Telebar):

```bash
~/.config/omarchy/plugins/io.github.pavellizunov.telebar/bin/omagram-menu-install
```

## Sign in

1. Create your own API id at [my.telegram.org](https://my.telegram.org) → *API development
   tools*. Telegram requires every client to use its own id; Telebar does not ship one.
2. Open Telebar — search for it in the app launcher (Super + Space), from the menu, by
   right-clicking the bar icon, or with
   `/usr/bin/python3 ~/.config/omarchy/plugins/io.github.pavellizunov.telebar/bin/omagram`.
3. Enter the API id and hash, then your phone number, the code Telegram sends you, and your
   two-step verification password if you have one. Or choose **Use a QR code instead** and scan
   it with Telegram on your phone (Settings → Devices → Link Desktop Device); drawing the code
    needs `qrencode`. The sign-in screen shows progress while the code is prepared;
    if preparation is unavailable, use your phone number. QR encoding runs outside
    the service loop and an expired result cannot replace a newer sign-in step.

The API id and hash go straight into your keyring; they are never written to a file.

### Multiple accounts (fork)

Click the profile initial beside the search field and choose **Add Account…** to sign in
with another phone number or QR code. If Telegram requests a two-step verification
password after scanning the QR code, enter that account's cloud password. **Back to
another account** leaves the sign-in screen without signing out your other accounts.

Select a profile in the same menu or in the bar icon's right-click menu. The window and
quick view share the selected profile; all signed-in accounts continue receiving updates.
The bar indicator sums unmuted unread messages from the main chat lists. Up to ten local
profiles are supported. Notifications and their actions are tied to the originating account.

The existing `default` profile keeps its original database and keyring entry. New profiles
use `~/.local/share/omagram/accounts/<id>/` and separate encryption keys. Application API
credentials are shared. **Log out** lets TDLib close its session; it does not delete files
or encryption keys behind TDLib's back. The local profile remains available for sign-in.

Telebar uses its own plugin ID, `io.github.pavellizunov.telebar`. It retains the
`omagram` executable names, window class, socket, configuration/data directories and
keyring identifiers for compatibility with existing accounts. The legacy managed
desktop entry can be refreshed to show Telebar without replacing an unrelated entry.

**Do not run Telebar and Omagram together against the same session directories.**
Stop and disable the old installation before deploying the replacement; a different
plugin ID does not isolate the database. Deployment and any desktop/menu migration
are separate from editing this checkout. Existing Omagram menu entries need explicit
removal with the old installer before adding Telebar's entries.

### Forum groups and topics

Opening a forum group in the main window shows its topics. Select a topic to open its
messages; the back arrow returns to the topic list. Topics and messages are fetched in
pages of 50, rather than loading the group's entire history at once. Scroll the topic
list to load further pages. A failed topic request shows the error and a retry button.
The quick view remains a compact chat view; use the main window to choose forum topics.

### Window layout and stories

Drag the separator between the chat list and the conversation to resize the list. Its saved
width is 72–1200 pixels and is capped at 45% of the current window width, leaving most of
the window for the conversation. The default is 300 pixels. Drag below 180 pixels and
release to collapse it to a 72-pixel avatar rail with unread badges. Double-click the
separator to collapse/expand. Search expands the rail; the profile button still switches
accounts and provides access to Settings in compact mode.
The panel-arrow button at the top of the separator also collapses/expands with one click
(or Enter/Space when focused). The divider is a one-pixel line with a wider invisible
drag target, so it need not be visually thick to remain easy to grab.

Stories are downloaded into TDLib's private `database/stories/` directory within the selected
account. The viewer reports download/display errors and offers **Retry story**. Closing the
viewer stops viewing that story; the downloaded files remain managed by TDLib.

Silent animated stickers, GIFs and round-video previews only animate while their message is
in the visible viewport and the Telebar window has focus. Explicit round-video audio playback
can continue independently.

### Built-in connection bridge

Settings → Connection → **Built-in MTProto → WebSocket bridge** enables the bundled
transport for the selected account. **Test the built-in bridge without switching** asks
TDLib to ping it without changing the current account connection. A local loopback process
is started on demand; no tray application, system package or root service is needed.
Secrets travel through a private pipe and TDLib, never through command-line arguments
or the UI/diagnostic log. Only proxy IDs and previous connection IDs are saved in
`~/.config/omagram/bridge.json`. Disabling restores the preceding proxy if it still exists,
otherwise direct access. The bridge exits with its owning daemon; unexpected exits are
retried at most once per 30 seconds. A test-only idle process is stopped after three minutes.

The MIT transport algorithms are adapted from Flowseal `tg-ws-proxy` at the revision recorded
in `bin/vendor/flowseal/README.md`. This bundled profile uses Telegram WebSocket endpoints
and the upstream DC2/DC4 WebSocket entrypoint with TLS hostname verification. No public
Cloudflare relay pool, remote update job, fake-TLS server or third-party SNI fronting is
enabled. Availability and benefit depend on the network; it is not a guarantee of bypass
or higher throughput. See `THIRD_PARTY_NOTICES.md` for attribution.

### Interface icons and media bounds

Attachment and profile-photo pickers use the shared Qt Quick `FilePicker.qml` and
Qt's directory model instead of the GTK native chooser, with no helper dependency.
Click files or press Space to select; Open confirms, Escape cancels. The path field
accepts absolute local folders, including mounted drives; Up navigates to the parent.
This avoids the GTK places-sidebar path implicated in a USB
volume-change crash on this installation. The original memory fault's owning
component and physical USB regression are not yet established. Multi-file
attachments and the profile-photo image filter retain their existing behavior.

Service controls use the original 24-unit SVG drawings in `app/Icons.js`, rendered through
`Icon.qml`. Existing action glyph codes are aliases, not a font dependency for drawing.
The inherited Omagram Ring mark and user-provided emoji/sticker artwork retain their identity.
Icons pulse briefly on a state change; loading spinners run only while visible. SVGs are
rasterized by Qt at a stable small size and shared by its image cache.
The simplified drawings use 2.4-unit rounded strokes. Play/pause and the sidebar arrow
use original matched QML contours with a finite 160 ms transition; they do not reparse
SVG on every animation frame. Message bubbles inherit the theme's corner radius, with visible
spacing and a faint boundary, with distinct incoming/outgoing fills. Quick replies
likewise separate individual messages. These are functional separation cues, not shadows
or continuously animated decoration.

**Quit** in the bar menu closes the app window and gracefully stops the daemon and its
owned proxy. It leaves the installed bar widget in place, so clicking it can start
Telebar again. It does not sign out your Telegram accounts. If you want the widget
removed from the bar, use Omarchy's plugin/widget controls separately.

Full-screen photos have bounded asynchronous decode sizes and do not occupy the UI-image
cache. In-message image sizes are rounded into 128-pixel buckets to avoid decoding again
for every single-pixel resize. Voice media is opened only when playback is requested.
Incoming UI events are handled in batches of 32 or six milliseconds, and chat updates are
coalesced over 100 milliseconds before rebuilding/sorting lists. These limits preserve
responsiveness under synchronization bursts; a single expensive QML handler can still
exceed the budget and is recorded by local diagnostics.

### Diagnosing pauses

Quickshell keeps per-instance UI logs. Find the current instance with `quickshell list --all`
and read its log with `quickshell log --pid <PID> --tail 100 --no-color`. If launched as a user
systemd service, the same process output is also available through `journalctl --user -u <unit>`.
The daemon also records local JSONL diagnostics in
`~/.local/state/omagram/diagnostics/trace.jsonl`, with four rotated backups. Each file is
capped at 4 MiB (20 MiB total), permissions are `0600`, and the containing directory is
`0700`. There is no external telemetry endpoint. TDLib's verbose content log stays disabled.

Records include IPC/TDLib command names, request IDs and elapsed times; slow handlers;
event/task queue depth, backpressure, pending request ages, background jobs and socket
backlogs; process RSS/swap, CPU time, page faults, I/O counters and system memory pressure.
Every five seconds the window and shell report UI heartbeat delays, handler/response times,
pending counts and numeric cache/layout sizes. The focused window also samples QML
animation-tick intervals. These are **not GPU or compositor frame timings**; sampling
intentionally keeps an animation callback active while the focused window is visible.

An independent watchdog records location-only Python thread stacks if the daemon stops
progressing for 2.5 seconds, at most once per 15 seconds. It reports missing UI heartbeats
and their recovery. It cannot distinguish a compositor/GPU stall from a blocked UI thread
on its own, and cannot produce native TDLib/Qt stacks. A stopped TDLib receive thread is
reported to the UI rather than failing silently. Ordinary UI callbacks are capped at 512
and expire after three minutes; an expired send is not automatically retried, since it
may already have reached Telegram. The daemon independently expires pending IPC and
tracked TDLib requests after three minutes. It admits at most 512 pending commands per
connection and 1024 tracked TDLib requests overall. Reusing an outstanding request ID
closes the ambiguous connection. Expiry retires local callbacks, not Telegram's operation;
late results cannot satisfy a newer command that reused the ID.

No message bodies, account names/numbers, file paths, passwords, login codes, API hashes,
request arguments, exception messages or frame-local variables are recorded. Operation
names and timestamps still reveal usage patterns: do not publish the log unreviewed.
Disk writes run on a dedicated bounded queue. If the filesystem fails, diagnosis records
can be dropped; status exposes logger queue/drop/error counts. Logging is best-effort,
not a durable audit trail. The code hash in the startup record identifies the instrumented
backend and selected UI sources.

Current counters can be queried over the private local socket using `diagnostics.status`.
Use `diagnostics.ui` only for numeric UI reports; arbitrary strings are discarded and
reports are rate-limited per connection. Diagnostic fields do not contain chat contents.

The daemon processes at most 128 TDLib updates or eight milliseconds of update work before
returning to IPC handling. A single slow handler can still exceed that budget. Clipboard reads
run in bounded background jobs, so a slow clipboard owner does not block the service loop.

## Keys

These are the defaults. **Every one of them can be changed in Settings** — the gear in the chat
list, or `Ctrl+,`: choose an action, press Enter and then the new keys (A adds a key, Backspace
removes one, R resets it). Settings shows when two actions would fight over the same keys, and its
own keys never change, so a bad choice can always be undone. Your choices are kept in
`~/.config/omagram/settings.json`.

**Window**

| key | action |
|---|---|
| `Ctrl+K` / `Ctrl+F` | search chats and messages |
| `Ctrl+Shift+F` | search in the open chat |
| `Alt+↑` / `Alt+↓` | previous / next chat |
| `Ctrl+PgUp` / `Ctrl+PgDn`, `Ctrl+[` / `Ctrl+]` | previous / next folder tab |
| `Ctrl+1` / `Ctrl+2` / `Ctrl+3` | chat list / messages / composer |
| `Ctrl+;` or `Ctrl+.` | the emoji panel: emoji, symbols and kaomoji by name |
| `Ctrl+M` | jump to the next message that mentions you |
| `Ctrl+Shift+E` | jump to the next reaction to your messages you have not seen |
| `Ctrl+Shift+J` | go to a date in the chat (today, yesterday, 1 Sep, 01.09.2026) |
| `Ctrl+Shift+M` | mute or unmute the open chat |
| `Ctrl+Shift+D` | set messages in the open chat to disappear after a day, a week or a month |
| `Ctrl+Shift+B` | silent sending in the open chat, on or off: Telegram's own setting, so your other apps follow it |
| `Ctrl+Shift+P` | go to the pinned message |
| `Ctrl+I` | the chat's info (`Tab` switches its tabs, `Enter` opens, `Esc` closes) |
| `Ctrl+Shift+N` | start a chat, a group or a channel (`Enter` opens or adds, `Ctrl+Enter` goes on) |
| `Alt+←` | from a forum's topic back to its topics, from comments back to their post |
| `Ctrl+,` | settings |

**Chat list**

| key | action |
|---|---|
| `↑` `↓` or `j` `k`, `g` / `G` | move, first / last |
| `Enter`, `l` or `→` | open the chat (or the message found) |
| `/` | search |
| `[` / `]` | previous / next tab |
| `p` | pin or unpin |
| `a` | archive or unarchive |
| `m` | mute or unmute |
| `Menu` or `Shift+F10` | the chat's menu (so does a right click) |
| `Tab` | go to the open chat |

**Messages**

| key | action |
|---|---|
| `↑` `↓` or `j` `k` | select a message |
| `Enter` or `o` | download or open its media |
| `Space` | play or pause |
| `r` / `e` / `y` | reply / edit yours / copy |
| `Shift+R` | react: find one by name, on the message or on everything selected |
| `f` / `p` / `s` | forward / pin or unpin / save its file to Downloads |
| `Shift+Y` | copy a link to the message |
| `c` | the post's comments, or the replies to the message |
| `.` | voice and video messages at 1×, 1.5× or 2× |
| `x` | select or unselect (so does Ctrl+click); `Shift+R`, `f`, `y` and `d` then act on everything selected |
| `m`, `Menu` or `Shift+F10` | the message's menu (so does a right click) |
| `d` or `Delete` | delete (press again to confirm) |
| `Esc` or `i` | clear the selection, or back to the composer |

**Composer**

| key | action |
|---|---|
| `Enter` / `Shift+Enter` | send / new line |
| `Ctrl+Shift+Enter` | send without sound |
| `Ctrl+B` / `Ctrl+Shift+I` | **bold** / __italic__ around the selection (again takes it off) |
| `Ctrl+Shift+X` / `Ctrl+E` / `Ctrl+Shift+H` | ~~strikethrough~~ / `code` / \|\|spoiler\|\| |
| `Ctrl+L` | a link: the selection becomes its text, then type the address |
| `Ctrl+Shift+L` | the link preview: under the text, above it, or none |
| `Ctrl+Alt+Enter` | send later or when they are online; scheduled messages are listed there too |
| `↑` in an empty composer | edit your last message |
| `Esc` | cancel a reply or edit |
| `Ctrl+O` / `Ctrl+Shift+O` | attach photos / send files uncompressed |
| `Ctrl+Shift+A` | a poll, dice, a contact card or a location |
| `Ctrl+V` / `Ctrl+Shift+V` | with files or a picture copied: attach them / send them as files |
| `Ctrl+S` | stickers (arrows or `hjkl`, `Tab` switches sets, `Enter` sends) |
| `Ctrl+R` | record a voice message (`Enter` sends, `Esc` cancels) |
| `Ctrl+Shift+R` | record a round video message (`Enter` starts, then sends) |

**Menus and questions** — in a menu `↑` `↓` or `j` `k` choose, `Enter` picks, `Esc` closes, and
`1`–`8` pick a quick reaction. When the bar above the message box asks something (deleting,
joining a group, opening a file that could run a program), `Enter` answers yes and `Esc` no. In
the forward dialog, type to find a chat, `↑` `↓` or `Ctrl+N` `Ctrl+P` choose and `Enter` forwards;
`Tab` or `Ctrl+Space` ticks as many chats as you like and `Enter` then forwards to all of them.

**From anywhere** — pick keys for the quick view (as an overlay or in the bar's panel) and for
opening Telebar in Settings → *Shortcuts that work anywhere*. Telebar registers them with Hyprland
while it runs, never writes them into your Hyprland config, leaves combinations you already use
alone (Settings shows them as taken), and only ever removes bindings it made. Or bind the commands
yourself:

```bash
omarchy-shell shell toggle io.github.pavellizunov.telebar '{}'   # the quick view as an overlay
omarchy-shell io.github.pavellizunov.telebar.panel toggle        # the quick view in the bar's panel
```

In the quick view: type to search, `↑` `↓` or `Ctrl+J` `Ctrl+K` to choose, `Enter` to answer and
`Enter` again to send. In a chat, `Ctrl+R` records a voice message and `Ctrl+Shift+R` a round
video message (`Enter` sends it, `Esc` throws it away), `Ctrl+S` opens your recent stickers,
`Ctrl+V` pastes files or a picture you copied (`Ctrl+Shift+V` as files),
`Ctrl+P` plays the newest voice or round video message (again to stop; a round video shows big
while it plays), `Ctrl+O` opens the chat
in the window and `Esc` goes back. Over a photo or video: `←` `→` step through them, `Enter` plays
a video in your video player, `o` opens the chat in the window and `Esc` closes.

## How it is put together

- **`bin/omagramd`** — the service. It holds the Telegram session through TDLib and serves
  the window, the bar and the overlay over a Unix socket. Omarchy's shell keeps it running;
  the window starts it too if needed, and only one instance ever runs.
- **`bin/omagram`** — opens or focuses the window, a separate Quickshell process with its own
  Hyprland class `omagram`, so it tiles and takes window rules like any application.
  `omagram --chat <id>` opens it at a chat, and `omagram --desktop-entry` keeps its entry in the
  app launcher up to date (the shell service runs it whenever it starts).
- **`shell/`** — the parts that live inside Omarchy's shell: the service entry, the bar widget,
  and the quick view it shows in its panel and in the overlay.

## Privacy and security

- **Your data stays on your machine**, in `~/.local/share/omagram` (TDLib's database, encrypted
  with a key kept in your keyring, downloaded files, and in `sent/` the voice and video messages you
  send, so your own messages play from them) and `~/.cache/omagram` (the
  TDLib build, unpacked animated stickers, and the small silent animations that round videos move
  with in the quick view). Telebar sends nothing anywhere except to Telegram.
- **Telebar in the app launcher.** The shell writes `~/.local/share/applications/omagram.desktop`.
  It rewrites that file only while it is Telebar's own (marked `X-Omagram-Managed`) and out of
  date, never replaces a file there that it did not write, and keeps `NoDisplay=true` if you set
  it to take Telebar off the launcher.
- **Secrets are never in files, command lines or logs.** The API id, hash and database key
  move through `secret-tool` on stdin and stdout. TDLib's own log is off, because at higher
  verbosity it records message text.
- **Only you can talk to the service.** Its socket is `0600` in your runtime directory, and it
  checks every connection's user id.
- **The microphone and camera are used only while you record.** A voice or round video message
  records from the moment you start it until you send it or throw it away, in the window or in
  the quick view, which shows a bar the whole time. If preparation is busy, stopping is rejected
  without consuming the recording; retry once capacity returns. An outgoing video allocation
  failure also leaves the recording available to stop again. A discarded recording is deleted
  when its admitted finalization completes. After preparation failure or a confirmed Telegram
  rejection, one recording per account is kept for explicit retry or discard, with its original
  chat, topic/thread, reply and send options. The window and quick view expose the same controls.
  Timeout or uncertain delivery disables retry; dismissing that state does not delete a file
  TDLib might still use. This recovery state lasts for the running service, not across restarts.
- **Telegram content is shown as text.** Names and previews are rendered as plain text, message
  formatting is escaped before it is drawn, and notification bodies are escaped, because
  Omarchy's notifications render markup and links. Links lead only to web and mail addresses
  (opened in your browser) or inside Telebar; joining a group, starting a bot and opening a file
  that could run a program (a script, an executable, a `.desktop` file, a web page) ask first.
- **Bounded and checked.** Every request is validated field by field; network strings, lists
  and animated stickers are size-capped; files you send must be regular, readable files of at
  most 2 GB outside Telebar's own database; helpers run by absolute path as argument lists,
  never through a shell.
- **The library is only loaded if it is safe to.** `libtdjson.so` is used only if it is a
  regular file you own that nobody else can write, in directories you own.

`bin/plugin_safety.py` is a shared safety library vendored unchanged into each of these plugins.

```bash
python3 -B tests/lifecycle_contract_test.py # pending limits, expiry and stale-resource cleanup
python3 -B tests/auth_contract_test.py      # bounded startup errors and attempt/account isolation
python3 -B tests/export_contract_test.py    # message-backed file-export permission
python3 tests/state_test.py     # TDLib objects → what the UI sees, hostile values
python3 tests/rich_test.py      # rich posts, full-content requests and photo permission checks
python3 tests/daemon_test.py    # the service on a sandboxed socket with a fake TDLib
python3 tests/notify_test.py    # notifications with a fake bus
python3 tests/media_test.py     # preparing voice and video messages
python3 tests/settings_test.py  # settings and global shortcuts, with Hyprland faked
python3 tests/install_test.py   # the menu entries, the window's runtime root, the launcher entry
python3 tests/rename_test.py    # Telebar identity, attribution and legacy account/runtime compatibility
node tests/model-test.js        # the window's list, message and menu logic
node tests/keymap-test.js       # shortcuts: parsing, matching, clashes
node tests/accounts-ui-test.js  # account event filters, stale callbacks and snapshots
node tests/topics-ui-test.js    # forum load errors, stale replies and pagination progress
node tests/stories-ui-test.js   # story requests/errors and bounded chat-list width
python3 -m unittest discover -s tests -p event_loop_test.py  # queue fairness under synthetic updates
python3 -m unittest discover -s tests -p diagnostics_test.py # watchdog, rotation, privacy and unsafe files
python3 -m unittest discover -s tests -p bridge_test.py      # crypto, packet bounds, loopback process lifetime
node tests/icons-test.js        # original SVG and glyph aliases
node tests/ui-batch-test.js     # batched chat correctness; synthetic timing is not a desktop benchmark
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/lib/qt6/bin/qmltestrunner -input tests/visual/tst_morph.qml
```

`.github/workflows/inert-checks.yml` defines per-push/pull-request Python and
JavaScript checks inside a network-isolated read-only worker. Hosted results are
available in [GitHub Actions](https://github.com/PavelLizunov/omarchy-telebar/actions);
local suite results do not establish hosted exact-commit acceptance. Large-file boundary fixtures assert
production constants before substituting small test thresholds; worker limits
are not increased. `tests/inert_suite.py` requires that isolated environment.

`tests/visual/README.md` describes bundled inert consumer imports, reviewed screens and the
limitations of the configured QML-preview MCP. Rendered PNGs do not prove desktop
placement, all theme contrasts, media playback or every interactive route.

## Remove

```bash
bin/omagram-menu-install remove                 # if you added the menu entries
omarchy plugin remove io.github.pavellizunov.telebar
rm ~/.local/share/applications/omagram.desktop  # its entry in the app launcher
rm -rf ~/.local/share/omagram ~/.cache/omagram  # the session, downloads and the TDLib build
secret-tool clear service omagram               # the API id, hash and database key
```

Removing the data does not end the session on Telegram's side: to do that, terminate it from
Settings → Devices in another Telegram app.

## Support

Report Telebar issues in [this repository](https://github.com/PavelLizunov/omarchy-telebar/issues).
The original Omagram author's [Donatello page](https://donatello.to/DuduPhudu) supports
upstream development, not Telebar's maintainer.

## License

MIT. Original Omagram copyright © 2026 ReidenXerx; the original notice is preserved
in [LICENSE](LICENSE). Telebar is independently maintained by PavelLizunov.
Third-party notices remain in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

TDLib is © Aliaksei Levin and Arseny Smirnov, under the Boost Software License 1.0;
Telebar downloads and builds it on your machine and does not redistribute it. Telebar is an
independent project and is not affiliated with Telegram.
