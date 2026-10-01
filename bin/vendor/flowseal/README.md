# Vendored transport core

Source: https://github.com/Flowseal/tg-ws-proxy
Revision: caa949bee0873d2b95dfb4fbeb1b7868b0ee3843
License: MIT, copyright 2026 Flowseal (LICENSE in this directory).

Selected algorithms are adapted from `_aes.py`, `tg_ws_proxy.py`, `bridge.py` and
`raw_websocket.py`. No tray, installer, update checker, connection pool or GUI.
Omagram's runner supplies private credentials over stdin and owns process lifetime.
Local modifications: system OpenSSL AES-CTR and trust roots, bounded listener/handshake/
packet buffers, verified WebSocket upgrade response. The Omagram runner uses only Telegram
WebSocket hosts with normal TLS hostname checking. No third-party fallback or SNI fronting.
