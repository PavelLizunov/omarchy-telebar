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
            let connector = native_tls::TlsConnector::new().map_err(|_| failure())?;
            let tls = tokio_native_tls::TlsConnector::from(connector)
                .connect(&host, stream)
                .await
                .map_err(|_| failure())?;
            let stream = UpgradeStream {
                inner: tls,
                remaining: Some(16384),
                scratch: Vec::new(),
            };
            let (mut ws, _) =
                tokio_tungstenite::client_async_with_config(request, stream, Some(config))
                    .await
                    .map_err(|_| failure())?;
            ws.get_mut().remaining = None;
            Ok::<_, io::Error>(ws)
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
async fn relay(stream: TcpStream, secret: [u8; 16], counts: Counts) -> io::Result<()> {
    let (mut reader, mut writer) = stream.into_split();
    let mut handshake = [0; 64];
    timeout(Duration::from_secs(10), reader.read_exact(&mut handshake))
        .await
        .map_err(|_| failure())??;
    let (dc, tag) = crypto::client_handshake(&handshake, &secret)?;
    let mut ws = websocket(dc).await?;
    let relay = crypto::relay_handshake(tag, dc)?;
    let crypto = Crypto::new(&handshake, &secret, &relay)?;
    let mut splitter = Splitter::new(&relay, tag)?;
    timeout(
        Duration::from_secs(30),
        ws.send(Message::Binary(relay.to_vec().into())),
    )
    .await
    .map_err(|_| failure())?
    .map_err(|_| failure())?;
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
