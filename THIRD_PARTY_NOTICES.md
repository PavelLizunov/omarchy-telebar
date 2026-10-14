# Third-party notices

Telebar is based on Omagram by ReidenXerx. Original MIT copyright and permission
remain unchanged in `LICENSE`.

## TDLib

Official TDLib is built locally from https://github.com/tdlib/td at
`d1085f9cebc5a62379991ae1652673954f229c1f` (1.8.67). It is not redistributed here.
Boost Software License 1.0, copyright Aliaksei Levin and Arseny Smirnov.

## Emoji, symbols and kaomoji

`app/emoji/` originates in ReidenXerx's Omarchy emoji picker, MIT:
https://github.com/ReidenXerx/omarchy-emoji-picker. Its emoji data uses Unicode
Emoji 16.0 and CLDR en/uk/ru annotations at `1aaabe99aa65`, Unicode License V3.
`LICENSES/Unicode-3.0.txt`, copyright 1991-2026 Unicode, Inc.

Kaomoji originate in https://github.com/w33ble/emoticon-data at `92b6211ec2a9`, MIT,
`LICENSES/emoticon-data-MIT.txt`, copyright 2014 Joe Fleming. Local tag/search
translations do not change the original kaomoji.

## MTProto WebSocket algorithms

`rust/src/proxy_crypto.rs` and the native relay adapt algorithms from
https://github.com/Flowseal/tg-ws-proxy at
`caa949bee0873d2b95dfb4fbeb1b7868b0ee3843`, MIT, copyright 2026 Flowseal.
The exact license remains at `bin/vendor/flowseal/LICENSE`; generated packages
place it at `LICENSES/Flowseal-MIT.txt`. No upstream tray, updater or server list
is bundled. Native TLS uses system trust and fixed Telegram destinations.

## Native dependencies and interface drawings

Cargo.lock pins the runtime graph. OpenSSL bindings use system OpenSSL 3;
current OpenSSL and the Rust openssl crate are Apache-2.0. Tokio/Tungstenite are
MIT; native-tls and other dual-licensed crates permit MIT/Apache-2.0 as applicable.
Distributed native release archives include dependency version/license texts under
`LICENSES/rust-dependencies/`. A successful build is not an independent legal or
vulnerability audit; consult each dependency's metadata and upstream license.

`app/Icons.js`, the paper-plane contour and local interface drawings are original
MIT artwork for this fork, not Telegram's official logo or Lucide/Feather assets.
User-provided media and Unicode emoji remain their original artwork.
