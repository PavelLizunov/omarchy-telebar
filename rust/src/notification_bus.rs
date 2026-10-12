//! Bounded independent D-Bus transport. Never places previews in process argv.
use crate::notifications::{Key, Notification};
use futures_lite::{StreamExt, future};
use std::{collections::HashMap, sync::mpsc, thread::JoinHandle, time::Duration};
const BUS: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
#[derive(Debug)]
pub enum Event {
    Shown(Key, u64, u32),
    Action(u32, String),
    Closed(u32),
    Unavailable,
}
enum Command {
    Show(Key, u64, Notification),
    Close(u32),
}
pub struct Transport {
    sender: async_channel::Sender<Command>,
    receiver: mpsc::Receiver<Event>,
    stopping: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Transport {
    pub fn start(address: String, icon: String) -> std::io::Result<Self> {
        if icon.len() > 4096 || (!icon.is_empty() && !icon.starts_with("file://")) {
            return Err(std::io::Error::other("invalid notification icon"));
        }
        // Explicit local Unix bus only: no environment/session discovery or network.
        if !address.starts_with("unix:path=/") || address.len() > 4096 || address.contains(';') {
            return Err(std::io::Error::other(
                "explicit local notification bus required",
            ));
        }
        let (sender, requests) = async_channel::bounded(64);
        let (events, receiver) = mpsc::sync_channel(64);
        let stopping = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop = stopping.clone();
        let thread = std::thread::Builder::new()
            .name("telebar-notify".into())
            .spawn(move || {
                async_io::block_on(async move {
                    let operation = async {
                        let connection = zbus::connection::Builder::address(address.as_str())?
                            .method_timeout(Duration::from_secs(2))
                            .max_queued(64)
                            .build()
                            .await?;
                        let proxy = zbus::Proxy::new(&connection, BUS, PATH, BUS).await?;
                        let signals = proxy.receive_all_signals().await?;
                        Ok::<_, zbus::Error>((proxy, signals))
                    };
                    let initialized = future::race(operation, async {
                        async_io::Timer::after(Duration::from_secs(3)).await;
                        Err(zbus::Error::Failure(
                            "notification initialization timeout".into(),
                        ))
                    })
                    .await;
                    let Ok((proxy, mut signals)) = initialized else {
                        requests.close();
                        let _ = events.try_send(Event::Unavailable);
                        return;
                    };
                    enum Next {
                        Command(std::result::Result<Command, async_channel::RecvError>),
                        Signal(Option<zbus::Message>),
                    }
                    let mut by_chat: HashMap<Key, u32> = HashMap::new();
                    loop {
                        if stop.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        match future::race(async { Next::Command(requests.recv().await) }, async {
                            Next::Signal(signals.next().await)
                        })
                        .await
                        {
                            Next::Command(Err(_)) | Next::Signal(None) => {
                                requests.close();
                                break;
                            }
                            Next::Command(Ok(Command::Close(id))) => {
                                by_chat.retain(|_, value| *value != id);
                                let _ = proxy.call::<_, _, ()>("CloseNotification", &(id,)).await;
                            }
                            Next::Command(Ok(Command::Show(key, serial, mut n))) => {
                                n.replaces = by_chat.get(&key).copied().unwrap_or(n.replaces);
                                let mut hints: HashMap<&str, zbus::zvariant::Value<'_>> =
                                    HashMap::from([
                                        ("category", zbus::zvariant::Value::from("im.received")),
                                        ("desktop-entry", zbus::zvariant::Value::from("omagram")),
                                    ]);
                                let image = if n.image.is_empty() {
                                    None
                                } else {
                                    Some(file_uri(&n.image))
                                };
                                if let Some(image) = &image {
                                    hints.insert(
                                        "image-path",
                                        zbus::zvariant::Value::from(image.as_str()),
                                    );
                                }
                                if let Ok(id) = proxy
                                    .call::<_, _, u32>(
                                        "Notify",
                                        &(
                                            "Telebar",
                                            n.replaces,
                                            icon.as_str(),
                                            n.title.as_str(),
                                            n.body.as_str(),
                                            n.actions,
                                            hints,
                                            -1i32,
                                        ),
                                    )
                                    .await
                                {
                                    if by_chat.len() >= 512 && !by_chat.contains_key(&key) {
                                        by_chat.clear();
                                    }
                                    by_chat.insert(key.clone(), id);
                                    let _ = events.try_send(Event::Shown(key, serial, id));
                                }
                            }
                            Next::Signal(Some(message)) => {
                                match message.header().member().map(|m| m.as_str()) {
                                    Some("ActionInvoked") => {
                                        if let Ok((id, action)) =
                                            message.body().deserialize::<(u32, String)>()
                                            && action.len() <= 32
                                        {
                                            let _ = events.try_send(Event::Action(id, action));
                                        }
                                    }
                                    Some("NotificationClosed") => {
                                        if let Ok((id, _)) =
                                            message.body().deserialize::<(u32, u32)>()
                                        {
                                            by_chat.retain(|_, value| *value != id);
                                            let _ = events.try_send(Event::Closed(id));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                })
            })?;
        Ok(Self {
            sender,
            receiver,
            stopping,
            thread: Some(thread),
        })
    }
    pub fn show(&self, key: Key, serial: u64, notification: Notification) -> bool {
        if key.chat == 0
            || !crate::paths::valid_account_id(&key.account)
            || serial == 0
            || key.account.len() > 64
            || notification.title.chars().count() > 120
            || notification.body.len() > 4096
            || notification.image.len() > 4096
            || notification.actions != crate::notifications::ACTIONS
        {
            return false;
        }
        self.sender
            .try_send(Command::Show(key, serial, notification))
            .is_ok()
    }
    pub fn close(&self, id: u32) {
        let _ = self.sender.try_send(Command::Close(id));
    }
    pub fn poll(&self) -> Vec<Event> {
        self.receiver.try_iter().take(64).collect()
    }
}
impl Drop for Transport {
    fn drop(&mut self) {
        self.stopping
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.sender.close();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub fn file_uri(path: &str) -> String {
    let mut value = String::from("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            value.push(byte as char)
        } else {
            value.push_str(&format!("%{byte:02X}"));
        }
    }
    value
}
