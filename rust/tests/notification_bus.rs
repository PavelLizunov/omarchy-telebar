//! Private real D-Bus service, bounded transport, no desktop/session access.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use telebar_runtime::{
    notification_bus::{Event, Transport},
    notifications::{Key, Notifier},
};
#[derive(Clone)]
struct Server {
    calls: Arc<Mutex<Vec<(u32, String, String)>>>,
}
#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        _app: &str,
        replaces: u32,
        _icon: &str,
        title: &str,
        body: &str,
        actions: Vec<String>,
        hints: HashMap<String, zbus::zvariant::OwnedValue>,
        _expiry: i32,
    ) -> u32 {
        assert_eq!(actions, telebar_runtime::notifications::ACTIONS);
        assert_eq!(
            <&str>::try_from(hints.get("category").unwrap()).unwrap(),
            "im.received"
        );
        self.calls
            .lock()
            .unwrap()
            .push((replaces, title.into(), body.into()));
        42
    }
    fn close_notification(&self, id: u32) {
        assert_eq!(id, 42);
        self.calls
            .lock()
            .unwrap()
            .push((id, "closed".into(), String::new()));
    }
    #[zbus(signal)]
    async fn action_invoked(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        id: u32,
        action: &str,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn notification_closed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;
}
#[test]
#[ignore = "requires reviewed private notification bus namespace"]
fn actual_notification_bus_uses_private_server_and_retires() {
    assert_eq!(
        std::env::var("TELEBAR_ISOLATED_NOTIFICATION_BUS").as_deref(),
        Ok("1")
    );
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap();
    assert!(address.starts_with("unix:path=/tmp/"));
    let calls = Arc::new(Mutex::new(vec![]));
    let server = zbus::blocking::connection::Builder::address(address.as_str())
        .unwrap()
        .name("org.freedesktop.Notifications")
        .unwrap()
        .serve_at(
            "/org/freedesktop/Notifications",
            Server {
                calls: calls.clone(),
            },
        )
        .unwrap()
        .build()
        .unwrap();
    let transport = Transport::start(address.clone(), String::new()).unwrap();
    let mut notifier = Notifier::default();
    let key = Key {
        account: "default".into(),
        chat: 42,
    };
    assert!(
        transport.show(
            key.clone(),
            1,
            notifier
                .prepare(&key, "Synthetic", "<img> & hello", "")
                .unwrap()
        )
    );
    let until = Instant::now() + Duration::from_secs(5);
    let mut shown = false;
    while Instant::now() < until {
        for event in transport.poll() {
            if let Event::Shown(k, serial, id) = event {
                assert_eq!(serial, 1);
                assert_eq!(k, key);
                assert_eq!(id, 42);
                notifier.shown(k, id);
                shown = true;
            }
        }
        if shown {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(shown);
    assert_eq!(
        calls.lock().unwrap()[0],
        (0, "Synthetic".into(), "&lt;img&gt; &amp; hello".into())
    );
    server
        .emit_signal(
            None::<&str>,
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
            "ActionInvoked",
            &(42u32, "reply"),
        )
        .unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    let mut action = false;
    while Instant::now() < until {
        for event in transport.poll() {
            if let Event::Action(id, a) = event {
                assert_eq!(notifier.action(id, &a), Some(key.clone()));
                action = true;
            }
        }
        if action {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(action);
    transport.close(notifier.withdraw(&key).unwrap());
    let until = Instant::now() + Duration::from_secs(3);
    while calls.lock().unwrap().len() < 2 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(calls.lock().unwrap()[1].1, "closed");
    assert!(transport.show(
        key.clone(),
        2,
        notifier.prepare(&key, "Replacement", "two", "").unwrap()
    ));
    let until = Instant::now() + Duration::from_secs(3);
    let mut shown = false;
    while Instant::now() < until {
        shown |= transport
            .poll()
            .iter()
            .any(|e| matches!(e, Event::Shown(_, 2, 42)));
        if shown {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(shown);
    assert!(transport.show(
        key.clone(),
        3,
        notifier.prepare(&key, "Replacement", "three", "").unwrap()
    ));
    let until = Instant::now() + Duration::from_secs(3);
    let mut shown = false;
    while Instant::now() < until {
        shown |= transport
            .poll()
            .iter()
            .any(|e| matches!(e, Event::Shown(_, 3, 42)));
        if shown {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(shown);
    assert_eq!(
        calls.lock().unwrap()[3].0,
        42,
        "transport preserves replacement before parent consumes acknowledgements"
    );
    server
        .emit_signal(
            None::<&str>,
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
            "NotificationClosed",
            &(42u32, 2u32),
        )
        .unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    let mut closed = false;
    while Instant::now() < until {
        closed |= transport
            .poll()
            .iter()
            .any(|e| matches!(e, Event::Closed(42)));
        if closed {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(closed);
    // Same actual bus through Driver, but synthetic native account ID is never
    // dispatched; button effects are inspected before the TDLib boundary.
    if let Some(library) = std::env::var_os("TELEBAR_TEST_TDLIB") {
        use serde_json::json;
        let root =
            std::env::temp_dir().join(format!("telebar-notify-driver-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let home = telebar_runtime::storage::Storage::open(&root).unwrap();
        let mut app = telebar_runtime::application::Application::load(&home).unwrap();
        let mut driver = telebar_runtime::native_driver::Driver::open(
            std::path::Path::new(&library),
            "default".into(),
            "en".into(),
        )
        .unwrap();
        let mut state = telebar_runtime::state::State::default();
        state.chats.insert(42,json!({"id":42,"title":"Driver","kind":"private","notification":{"@type":"chatNotificationSettings"}}));
        driver.runtime.engine.accounts.insert(
            "default".into(),
            telebar_runtime::engine::Account {
                client: 41,
                generation: 1,
                ready: true,
                state,
            },
        );
        driver.runtime.metadata.insert(
            "default".into(),
            telebar_runtime::authorization::Metadata::new("Synthetic".into()),
        );
        app.windows.insert(7);
        driver.notification_transport =
            Some(Transport::start(address.clone(), String::new()).unwrap());
        let message = json!({"@type":"message","chat_id":42,"id":5,"sender_id":{"@type":"messageSenderUser","user_id":7},"content":{"@type":"messageText","text":{"@type":"formattedText","text":"Synthetic","entities":[]}}});
        let event = json!({"@type":"updateNotificationGroup","@client_id":41,"chat_id":42,"total_count":1,"added_notifications":[{"@type":"notification","id":1,"type":{"@type":"notificationTypeNewMessage","message":message,"show_preview":true}}]});
        driver.observe_notification(&event);
        let mut effects = telebar_runtime::engine::Effects::default();
        let until = Instant::now() + Duration::from_secs(3);
        while app.notifier.action(42, "reply").is_none() && Instant::now() < until {
            driver.pump_notifications(&mut app, &home, &mut effects);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(app.notifier.action(42, "reply"), Some(key.clone()));
        server
            .emit_signal(
                None::<&str>,
                "/org/freedesktop/Notifications",
                "org.freedesktop.Notifications",
                "ActionInvoked",
                &(42u32, "reply"),
            )
            .unwrap();
        let until = Instant::now() + Duration::from_secs(3);
        while effects.replies.is_empty() && Instant::now() < until {
            driver.pump_notifications(&mut app, &home, &mut effects);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(effects.replies.iter().any(|(peer, e)| *peer == 7
            && e["event"] == "open"
            && e["reply"] == true
            && e["account"] == "default"));
        server
            .emit_signal(
                None::<&str>,
                "/org/freedesktop/Notifications",
                "org.freedesktop.Notifications",
                "ActionInvoked",
                &(42u32, "read"),
            )
            .unwrap();
        let until = Instant::now() + Duration::from_secs(3);
        while effects.native.is_empty() && Instant::now() < until {
            driver.pump_notifications(&mut app, &home, &mut effects);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(effects.native[0].query["@type"], "viewMessages");
        assert_eq!(effects.native[0].client, 41);
        effects.native.clear();
        driver
            .runtime
            .engine
            .accounts
            .get_mut("default")
            .unwrap()
            .client = 0;
        driver.close(Duration::ZERO).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
    let started = Instant::now();
    drop(transport);
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(Transport::start("tcp:host=example.test".into(), String::new()).is_err());
}
