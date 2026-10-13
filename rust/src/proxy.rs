//! Owned loopback MTProto/WebSocket bridge. Only Telegram destinations; normal
//! TLS host verification/system roots. No account credentials, updater or GUI.
use crate::proxy_crypto::{self as crypto, Crypto, MAX_PACKET, Splitter};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{
    io::{self, Read, Write},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinSet,
    time::timeout,
};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, client::IntoClientRequest, protocol::WebSocketConfig},
};
fn failure() -> io::Error {
    io::Error::other("bridge connection failed")
}
#[derive(Clone, Default)]
struct Counts(std::sync::Arc<Metrics>);
#[derive(Default)]
struct Metrics {
    connections: std::sync::atomic::AtomicU64,
    active: std::sync::atomic::AtomicU64,
    failures: std::sync::atomic::AtomicU64,
    up_bytes: std::sync::atomic::AtomicU64,
    down_bytes: std::sync::atomic::AtomicU64,
}
impl Counts {
    fn value(&self) -> Value {
        use std::sync::atomic::Ordering::Relaxed;
        json!({"event":"stats","connections":self.0.connections.load(Relaxed),"active":self.0.active.load(Relaxed),"failures":self.0.failures.load(Relaxed),"up_bytes":self.0.up_bytes.load(Relaxed),"down_bytes":self.0.down_bytes.load(Relaxed)})
    }
}
struct Active(Counts);
impl Drop for Active {
    fn drop(&mut self) {
        self.0
            .0
            .active
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}
pub fn targets(dc: i16) -> Vec<(String, Option<String>)> {
    let id = if dc.unsigned_abs() == 203 {
        2
    } else {
        dc.unsigned_abs()
    };
    let mut hosts = vec![
        format!("kws{id}.web.telegram.org"),
        format!("kws{id}-1.web.telegram.org"),
    ];
    if dc < 0 {
        hosts.reverse();
    }
    let mut targets = vec![];
    if matches!(dc.unsigned_abs(), 2 | 4) {
        targets.extend(
            hosts
                .iter()
                .map(|host| (host.clone(), Some("149.154.167.220".into()))),
        );
    }
    targets.extend(hosts.into_iter().map(|host| (host, None)));
    targets
}
/// Bound decrypted upgrade bytes before Tungstenite's handshake accumulator.
/// After a successful upgrade the wrapper becomes an ordinary TLS stream.
struct UpgradeStream<S> {
    inner: S,
    remaining: Option<usize>,
    scratch: Vec<u8>,
}
impl<S: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for UpgradeStream<S> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        output: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        if self.remaining == Some(0) {
            return std::task::Poll::Ready(Err(io::Error::other("WebSocket headers exceed limit")));
        }
        let size = output
            .remaining()
            .min(self.remaining.unwrap_or(output.remaining()));
        let this = self.as_mut().get_mut();
        this.scratch.resize(size, 0);
        let mut buffer = tokio::io::ReadBuf::new(&mut this.scratch);
        match std::pin::Pin::new(&mut this.inner).poll_read(cx, &mut buffer) {
            std::task::Poll::Ready(Ok(())) => {
                let size = buffer.filled().len();
                output.put_slice(buffer.filled());
                if let Some(remaining) = &mut this.remaining {
                    *remaining -= size;
                }
                std::task::Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}
impl<S: tokio::io::AsyncWrite + Unpin> tokio::io::AsyncWrite for UpgradeStream<S> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bytes: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, bytes)
    }
    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
async fn websocket(
    dc: i16,
) -> io::Result<WebSocketStream<UpgradeStream<tokio_native_tls::TlsStream<TcpStream>>>> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(64);
    for (host, address) in targets(dc) {
        let attempt = async {
            let stream = TcpStream::connect((address.as_deref().unwrap_or(&host), 443)).await?;
            tls_websocket(
                stream,
                &host,
                native_tls::TlsConnector::new().map_err(|_| failure())?,
            )
            .await
        };
        let budget = deadline
            .saturating_duration_since(tokio::time::Instant::now())
            .min(Duration::from_secs(16));
        if budget.is_zero() {
            break;
        }
        if let Ok(Ok(ws)) = timeout(budget, attempt).await {
            return Ok(ws);
        }
    }
    Err(failure())
}
async fn tls_websocket(
    stream: TcpStream,
    host: &str,
    connector: native_tls::TlsConnector,
) -> io::Result<WebSocketStream<UpgradeStream<tokio_native_tls::TlsStream<TcpStream>>>> {
    let mut request = format!("wss://{host}/apiws")
        .into_client_request()
        .map_err(|_| failure())?;
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        "binary".parse().expect("fixed header"),
    );
    let config = WebSocketConfig::default()
        .read_buffer_size(16384)
        .write_buffer_size(0)
        .max_write_buffer_size(MAX_PACKET + 65536)
        .max_message_size(Some(MAX_PACKET))
        .max_frame_size(Some(MAX_PACKET));
    let tls = tokio_native_tls::TlsConnector::from(connector)
        .connect(host, stream)
        .await
        .map_err(|_| failure())?;
    let stream = UpgradeStream {
        inner: tls,
        remaining: Some(16384),
        scratch: Vec::new(),
    };
    let (mut ws, _) = tokio_tungstenite::client_async_with_config(request, stream, Some(config))
        .await
        .map_err(|_| failure())?;
    ws.get_mut().remaining = None;
    Ok(ws)
}
async fn relay(stream: TcpStream, secret: [u8; 16], counts: Counts) -> io::Result<()> {
    let (mut reader, writer) = stream.into_split();
    let mut handshake = [0; 64];
    timeout(Duration::from_secs(10), reader.read_exact(&mut handshake))
        .await
        .map_err(|_| failure())??;
    let (dc, tag) = crypto::client_handshake(&handshake, &secret)?;
    let mut ws = websocket(dc).await?;
    let relay = crypto::relay_handshake(tag, dc)?;
    let crypto = Crypto::new(&handshake, &secret, &relay)?;
    let splitter = Splitter::new(&relay, tag)?;
    timeout(
        Duration::from_secs(30),
        ws.send(Message::Binary(relay.to_vec().into())),
    )
    .await
    .map_err(|_| failure())?
    .map_err(|_| failure())?;
    relay_frames(reader, writer, ws, crypto, splitter, counts).await
}
async fn relay_frames<S>(
    mut reader: tokio::net::tcp::OwnedReadHalf,
    mut writer: tokio::net::tcp::OwnedWriteHalf,
    ws: WebSocketStream<S>,
    crypto: Crypto,
    mut splitter: Splitter,
    counts: Counts,
) -> io::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut sink, mut source) = ws.split();
    let (mut client_dec, mut telegram_enc, mut client_enc, mut telegram_dec) = (
        crypto.client_dec,
        crypto.telegram_enc,
        crypto.client_enc,
        crypto.telegram_dec,
    );
    let (control, mut controls) = tokio::sync::mpsc::channel::<()>(8);
    let up = async {
        let mut buffer = [0; 65536];
        loop {
            let size = tokio::select! {
                read = reader.read(&mut buffer) => read?,
                pulse = controls.recv() => {
                    if pulse.is_none() { return Ok(()); }
                    // Tungstenite queues automatic pong frames while reading.
                    timeout(Duration::from_secs(30), sink.flush()).await.map_err(|_| failure())?.map_err(|_| failure())?;
                    continue;
                }
            };
            if size == 0 {
                return Ok::<_, io::Error>(());
            }
            counts
                .0
                .up_bytes
                .fetch_add(size as u64, std::sync::atomic::Ordering::Relaxed);
            for packet in
                splitter.split(&telegram_enc.update(&client_dec.update(&buffer[..size])?)?)?
            {
                timeout(
                    Duration::from_secs(30),
                    sink.send(Message::Binary(packet.into())),
                )
                .await
                .map_err(|_| failure())?
                .map_err(|_| failure())?;
            }
        }
    };
    let down = async {
        while let Some(frame) = source.next().await {
            let frame = frame.map_err(|_| failure())?;
            let data = match frame {
                Message::Binary(data) => data,
                Message::Close(_) => return Ok::<_, io::Error>(()),
                Message::Ping(_) => {
                    control.try_send(()).map_err(|_| failure())?;
                    continue;
                }
                Message::Pong(_) => continue,
                _ => return Err(failure()),
            };
            counts
                .0
                .down_bytes
                .fetch_add(data.len() as u64, std::sync::atomic::Ordering::Relaxed);
            let bytes = client_enc.update(&telegram_dec.update(&data)?)?;
            timeout(Duration::from_secs(30), writer.write_all(&bytes))
                .await
                .map_err(|_| failure())??;
        }
        Ok(())
    };
    tokio::select! {result=up=>result,result=down=>result}
}
async fn serve(
    listener: std::net::TcpListener,
    secret: [u8; 16],
    mut stop: watch::Receiver<bool>,
) -> io::Result<()> {
    listener.set_nonblocking(true)?;
    let listener = TcpListener::from_std(listener)?;
    let counts = Counts::default();
    let mut tasks = JoinSet::new();
    let mut report = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(10),
        Duration::from_secs(10),
    );
    loop {
        tokio::select! {
            changed=stop.changed()=>{if changed.is_err()||*stop.borrow(){break;}},
            accepted=listener.accept()=>{let(stream,_)=accepted?;if tasks.len()>=32{drop(stream);continue;}let counters=counts.clone();counters.0.connections.fetch_add(1,std::sync::atomic::Ordering::Relaxed);counters.0.active.fetch_add(1,std::sync::atomic::Ordering::Relaxed);tasks.spawn(async move{let _active=Active(counters.clone());if relay(stream,secret,counters.clone()).await.is_err(){counters.0.failures.fetch_add(1,std::sync::atomic::Ordering::Relaxed);println!("{}",json!({"event":"connection_error","exception_type":"ConnectionError"}));}});},
            _=tasks.join_next(),if !tasks.is_empty()=>{},
            _=report.tick()=>{println!("{}",counts.value());},
        }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    Ok(())
}
fn decode_secret(value: &str) -> io::Result<[u8; 16]> {
    if value.len() != 32 {
        return Err(failure());
    }
    let mut out = [0; 16];
    for (i, part) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(part).map_err(|_| failure())?, 16)
            .map_err(|_| failure())?;
    }
    Ok(out)
}
/// stdin remains an ownership pipe. A nonblocking read retires all connections
/// on EOF; no detached thread survives shutdown and no signal retry is used.
pub fn run(stopping: &std::sync::atomic::AtomicBool) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let mut input = std::io::stdin();
    let fd = input.as_raw_fd();
    // SAFETY: stdin is this process's owned descriptor; no borrowed pointers.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut line = Vec::new();
    let mut byte = [0];
    while line.len() <= 1024 {
        if stopping.load(std::sync::atomic::Ordering::Relaxed)
            || std::time::Instant::now() >= deadline
        {
            return Err(failure());
        }
        match input.read(&mut byte) {
            Ok(0) => return Err(failure()),
            Ok(_) => line.push(byte[0]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            Err(e) => return Err(e),
        }
        if byte[0] == b'\n' {
            break;
        }
    }
    if !line.ends_with(b"\n") || line.len() > 1024 {
        return Err(failure());
    }
    let config = crate::wire::bounded_json(&line, 2, 4, 64).map_err(|_| failure())?;
    let secret = decode_secret(config["secret"].as_str().ok_or_else(failure)?)?;
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let port = listener.local_addr()?.port();
    println!("{}", json!({"event":"ready","port":port}));
    std::io::stdout().flush()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(4)
        .enable_all()
        .build()?;
    let result=runtime.block_on(async{
        let(sender,receiver)=watch::channel(false);let mut service=tokio::spawn(serve(listener,secret,receiver));let mut timer=tokio::time::interval(Duration::from_millis(20));
        loop{tokio::select!{done=&mut service=>break done.map_err(|_|failure())?,_=timer.tick()=>{let eof=match input.read(&mut byte){Ok(0)=>true,Ok(_)=>false,Err(e)if e.kind()==io::ErrorKind::WouldBlock=>false,Err(_)=>true};if eof||stopping.load(std::sync::atomic::Ordering::Relaxed){let _=sender.send(true);break service.await.map_err(|_|failure())?;}}}}
    });
    runtime.shutdown_timeout(Duration::from_secs(2));
    result
}
// The listener and finite connection tasks are independently usable in tests;
// test inputs must remain loopback/inert and never open a real Telegram profile.
pub fn fixture_lifetime(
    secret: [u8; 16],
) -> io::Result<(
    u16,
    watch::Sender<bool>,
    std::thread::JoinHandle<io::Result<()>>,
)> {
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let port = listener.local_addr()?.port();
    let (sender, receiver) = watch::channel(false);
    let thread = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(serve(listener, secret, receiver))
    });
    Ok((port, sender, thread))
}
#[cfg(test)]
mod tests {
    use super::*;
    // Tungstenite's public callback contract fixes the large HTTP error type.
    #[allow(clippy::result_large_err)]
    fn binary_fixture_headers(
        request: &tokio_tungstenite::tungstenite::handshake::server::Request,
        mut response: tokio_tungstenite::tungstenite::handshake::server::Response,
    ) -> Result<
        tokio_tungstenite::tungstenite::handshake::server::Response,
        tokio_tungstenite::tungstenite::handshake::server::ErrorResponse,
    > {
        assert_eq!(request.uri().path(), "/apiws");
        assert_eq!(request.headers()["Sec-WebSocket-Protocol"], "binary");
        response
            .headers_mut()
            .insert("Sec-WebSocket-Protocol", "binary".parse().unwrap());
        Ok(response)
    }
    #[test]
    fn local_tls_upgrade_verifies_host_and_trust_and_uses_native_binary_frames() {
        use openssl::{
            asn1::Asn1Time,
            bn::BigNum,
            hash::MessageDigest,
            pkey::PKey,
            rsa::Rsa,
            x509::{
                X509, X509NameBuilder,
                extension::{BasicConstraints, SubjectAlternativeName},
            },
        };
        let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
        let mut name = X509NameBuilder::new().unwrap();
        name.append_entry_by_text("CN", "fixture.telebar.invalid")
            .unwrap();
        let name = name.build();
        let mut cert = X509::builder().unwrap();
        cert.set_version(2).unwrap();
        cert.set_serial_number(&BigNum::from_u32(1).unwrap().to_asn1_integer().unwrap())
            .unwrap();
        cert.set_subject_name(&name).unwrap();
        cert.set_issuer_name(&name).unwrap();
        cert.set_pubkey(&key).unwrap();
        cert.set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        cert.set_not_after(&Asn1Time::days_from_now(1).unwrap())
            .unwrap();
        cert.append_extension(BasicConstraints::new().critical().ca().build().unwrap())
            .unwrap();
        let san = SubjectAlternativeName::new()
            .dns("fixture.telebar.invalid")
            .build(&cert.x509v3_context(None, None))
            .unwrap();
        cert.append_extension(san).unwrap();
        cert.sign(&key, MessageDigest::sha256()).unwrap();
        let pem = cert.build().to_pem().unwrap();
        let identity =
            native_tls::Identity::from_pkcs8(&pem, &key.private_key_to_pem_pkcs8().unwrap())
                .unwrap();
        let acceptor =
            tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::new(identity).unwrap());
        let certificate = native_tls::Certificate::from_pem(&pem).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            timeout(Duration::from_secs(8), async {
                for (trusted, host, accepted) in [
                    (true, "fixture.telebar.invalid", true),
                    (false, "fixture.telebar.invalid", false),
                    (true, "other.telebar.invalid", false),
                ] {
                    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                        .await
                        .unwrap();
                    let address = listener.local_addr().unwrap();
                    let acceptor = acceptor.clone();
                    let server = tokio::spawn(async move {
                        let (stream, _) = listener.accept().await.unwrap();
                        let tls = acceptor.accept(stream).await;
                        if !accepted {
                            assert!(tls.is_err(), "untrusted/wrong-host TLS must be refused");
                            return;
                        }
                        let mut ws = tokio_tungstenite::accept_hdr_async(
                            tls.unwrap(),
                            binary_fixture_headers,
                        )
                        .await
                        .unwrap();
                        assert_eq!(
                            ws.next().await.unwrap().unwrap(),
                            Message::Binary(b"synthetic-native-transport".to_vec().into())
                        );
                        ws.send(Message::Ping(b"tls-idle".to_vec().into()))
                            .await
                            .unwrap();
                        assert_eq!(
                            ws.next().await.unwrap().unwrap(),
                            Message::Pong(b"tls-idle".to_vec().into())
                        );
                        ws.send(Message::Binary(b"synthetic-server-reply".to_vec().into()))
                            .await
                            .unwrap();
                        ws.send(Message::Close(None)).await.unwrap();
                    });
                    let mut builder = native_tls::TlsConnector::builder();
                    if trusted {
                        builder.add_root_certificate(certificate.clone());
                    }
                    let stream = TcpStream::connect(address).await.unwrap();
                    let client = tls_websocket(stream, host, builder.build().unwrap()).await;
                    if accepted {
                        let mut client = client.unwrap();
                        client
                            .send(Message::Binary(
                                b"synthetic-native-transport".to_vec().into(),
                            ))
                            .await
                            .unwrap();
                        assert_eq!(
                            client.next().await.unwrap().unwrap(),
                            Message::Ping(b"tls-idle".to_vec().into())
                        );
                        client.flush().await.unwrap();
                        assert_eq!(
                            client.next().await.unwrap().unwrap(),
                            Message::Binary(b"synthetic-server-reply".to_vec().into())
                        );
                        assert!(matches!(
                            client.next().await.unwrap().unwrap(),
                            Message::Close(_)
                        ));
                    } else {
                        assert!(client.is_err());
                    }
                    server.await.unwrap();
                }
            })
            .await
            .expect("finite TLS three-request no-retry budget");
        });
    }
    #[test]
    fn actual_relay_flushes_idle_ping_and_preserves_full_duplex_cipher_bytes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            timeout(Duration::from_secs(5), async {
                let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                    .await
                    .unwrap();
                let address = listener.local_addr().unwrap();
                let (connected, accepted) =
                    tokio::join!(TcpStream::connect(address), listener.accept());
                let mut client = connected.unwrap();
                let stream = accepted.unwrap().0;
                let (local, remote) = tokio::io::duplex(65536);
                let ws = WebSocketStream::from_raw_socket(
                    local,
                    tokio_tungstenite::tungstenite::protocol::Role::Client,
                    Some(
                        WebSocketConfig::default()
                            .write_buffer_size(0)
                            .max_message_size(Some(MAX_PACKET)),
                    ),
                )
                .await;
                let mut server = WebSocketStream::from_raw_socket(
                    remote,
                    tokio_tungstenite::tungstenite::protocol::Role::Server,
                    Some(
                        WebSocketConfig::default()
                            .write_buffer_size(0)
                            .max_message_size(Some(MAX_PACKET)),
                    ),
                )
                .await;
                let mut handshake = [7u8; 64];
                for (i, b) in handshake.iter_mut().enumerate() {
                    *b = (i + 1) as u8;
                }
                let secret = [3u8; 16];
                let relay =
                    crypto::relay_with_random([11u8; 64], [1, 2], crypto::SECURE, 2).unwrap();
                let owner = Crypto::new(&handshake, &secret, &relay).unwrap();
                let peer = Crypto::new(&handshake, &secret, &relay).unwrap();
                let splitter = Splitter::new(&relay, crypto::SECURE).unwrap();
                let counts = Counts::default();
                let tracked = counts.clone();
                let (read, write) = stream.into_split();
                let task = tokio::spawn(async move {
                    relay_frames(read, write, ws, owner, splitter, tracked).await
                });
                // No application upload traffic: automatic pong still flushes.
                server
                    .send(Message::Ping(b"idle-keepalive".to_vec().into()))
                    .await
                    .unwrap();
                let pong = server.next().await.unwrap().unwrap();
                assert_eq!(pong, Message::Pong(b"idle-keepalive".to_vec().into()));
                use crate::proxy_crypto::Ctr;
                use openssl::hash::{MessageDigest, hash};
                let mut seed = handshake[8..40].to_vec();
                seed.extend(secret);
                let key = hash(MessageDigest::sha256(), &seed).unwrap();
                let mut encrypt = Ctr::new(&key, &handshake[40..56]).unwrap();
                encrypt.update(&[0; 64]).unwrap();
                let mut plain = Vec::new();
                for i in 0..128u32 {
                    plain.extend(1024u32.to_le_bytes());
                    plain.extend(vec![i as u8; 1024]);
                }
                let cipher = encrypt.update(&plain).unwrap();
                let mut expected = peer.telegram_enc;
                let expected = expected.update(&plain).unwrap();
                let upload = async {
                    for chunk in cipher.chunks(13) {
                        client.write_all(chunk).await.unwrap();
                    }
                };
                let receive = async {
                    let mut got = vec![];
                    while got.len() < expected.len() {
                        match server.next().await.unwrap().unwrap() {
                            Message::Binary(bytes) => got.extend(bytes),
                            other => panic!("unexpected relay frame {other:?}"),
                        }
                    }
                    assert_eq!(got, expected);
                };
                tokio::join!(upload, receive);
                let down = vec![0x5a; 65536];
                let expected = vec![0x5a; 65536];
                // Compose Telegram-to-client cipher direction using the exact
                // native owner inverses; no Telegram account or TLS involved.
                let reverse: Vec<_> = relay[8..56].iter().copied().rev().collect();
                let mut telegram = Ctr::new(&reverse[..32], &reverse[32..]).unwrap();
                let mut expected_cipher = peer.client_enc;
                let expected = expected_cipher.update(&expected).unwrap();
                let wire = telegram.update(&down).unwrap();
                let down_send = async {
                    server.send(Message::Binary(wire.into())).await.unwrap();
                };
                let down_read = async {
                    let mut got = vec![0; expected.len()];
                    client.read_exact(&mut got).await.unwrap();
                    assert_eq!(got, expected);
                };
                tokio::join!(down_send, down_read);
                assert_eq!(
                    counts.0.up_bytes.load(std::sync::atomic::Ordering::Relaxed),
                    plain.len() as u64
                );
                assert_eq!(
                    counts
                        .0
                        .down_bytes
                        .load(std::sync::atomic::Ordering::Relaxed),
                    down.len() as u64
                );
                server.send(Message::Close(None)).await.unwrap();
                assert!(task.await.unwrap().is_ok());
            })
            .await
            .expect("finite relay pressure/ping budget");
        });
    }
    #[test]
    fn upgrade_input_cannot_exceed_the_original_header_budget() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (mut write, read) = tokio::io::duplex(32);
            write.write_all(b"0123456789").await.unwrap();
            drop(write);
            let mut limited = UpgradeStream {
                inner: read,
                remaining: Some(4),
                scratch: Vec::new(),
            };
            let mut output = vec![];
            assert!(limited.read_to_end(&mut output).await.is_err());
            assert_eq!(output, b"0123");
        });
    }
}
