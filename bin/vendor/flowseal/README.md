# Adapted transport provenance

Algorithms used by `rust/src/proxy_crypto.rs` originate in
https://github.com/Flowseal/tg-ws-proxy at
`caa949bee0873d2b95dfb4fbeb1b7868b0ee3843`.
MIT, copyright 2026 Flowseal; the exact original license is retained here.

No vendored executable implementation, tray, updater, GUI or server pool remains
in this directory. Telebar's native relay owns its private configuration pipe,
bounded handshake/packet buffers and lifetime. OpenSSL provides AES/TLS and system
trust. Only fixed Telegram destinations are admitted; no third-party SNI fronting.
