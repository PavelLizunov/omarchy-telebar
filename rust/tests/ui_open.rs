//! Window routing over admitted envelopes; no desktop or Telegram I/O.
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};
use telebar_runtime::{
    application::Application, authorization::Runtime, storage::Storage, wire::Request,
};
#[test]
fn opens_are_window_only_account_owned_and_consumed_once() {
    let root = std::env::temp_dir().join(format!("telebar-ui-open-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let home = Storage::open(&root).unwrap();
    let mut app = Application::load(&home).unwrap();
    let mut runtime = Runtime::new("default".into());
    for (id, cid) in [("default", 1), ("other", 2)] {
        runtime
            .register(id.into(), id.into(), cid, vec![], Instant::now())
            .unwrap();
        runtime.engine.accounts.get_mut(id).unwrap().ready = true;
    }
    let req = |id, cmd, args| {
        Request::parse(
            json!({"id":id,"cmd":cmd,"args":args,"account":"other"})
                .to_string()
                .as_bytes(),
        )
        .unwrap()
    };
    let open = app.request(
        &home,
        &mut runtime,
        5,
        req(1, "ui.open", json!({"chatId":42,"reply":true})),
    );
    assert_eq!(open.replies.len(), 1);
    assert_eq!(open.replies[0].1["ok"], true);
    let shell = app.request(&home, &mut runtime, 5, req(2, "hello", json!({})));
    assert!(shell.replies[0].1["result"]["open"].is_null());
    assert!(app.pending_open.is_some());
    let window = app.request(
        &home,
        &mut runtime,
        6,
        req(3, "hello", json!({"window":true})),
    );
    assert_eq!(
        window.replies[0].1["result"]["open"],
        json!({"chatId":42,"reply":true,"account":"other"})
    );
    assert!(app.pending_open.is_none());
    let open = app.request(
        &home,
        &mut runtime,
        5,
        req(4, "ui.open", json!({"url":"HTTPS://WWW.T.ME/synthetic"})),
    );
    assert_eq!(open.replies[0].0, 6);
    assert_eq!(open.replies[0].1["event"], "open");
    assert_eq!(open.replies[1].0, 5);
    assert!(open.native.is_empty());
    for url in [
        "https://t.me.evil/synthetic",
        "tg://login\nsecret",
        "https://example.test/",
    ] {
        let refused = app.request(
            &home,
            &mut runtime,
            5,
            req(5, "ui.open", json!({"url":url})),
        );
        assert_eq!(refused.replies.len(), 1);
        assert_eq!(refused.replies[0].1["ok"], false);
        assert!(app.pending_open.is_none());
    }
    app.retire_peer(6);
    app.request(
        &home,
        &mut runtime,
        5,
        req(6, "ui.open", json!({"chatId":7})),
    );
    app.pending_open.as_mut().unwrap().0 = Instant::now() - Duration::from_secs(31);
    let window = app.request(
        &home,
        &mut runtime,
        6,
        req(7, "hello", json!({"window":true})),
    );
    assert!(window.replies[0].1["result"]["open"].is_null());
    assert!(app.pending_open.is_none());
    app.retire_peer(6);
    let held = runtime.engine.request(
        5,
        req(8, "chat.history", json!({"chatId":7})),
        Instant::now(),
    );
    assert_eq!(held.native.len(), 1);
    let duplicate = app.request(
        &home,
        &mut runtime,
        5,
        req(8, "ui.open", json!({"chatId":42})),
    );
    assert_eq!(duplicate.disconnect, Some(5));
    assert!(app.pending_open.is_none());
    // Focus mutates local state only after admission and queues an owned read.
    let key = telebar_runtime::notifications::Key {
        account: "other".into(),
        chat: 42,
    };
    app.notifier.shown(key.clone(), 17);
    runtime
        .engine
        .accounts
        .get_mut("other")
        .unwrap()
        .state
        .chats
        .insert(42, json!({"id":42,"reactions":2}));
    let focused = app.request(
        &home,
        &mut runtime,
        6,
        req(10, "ui.focus", json!({"chatId":42})),
    );
    assert_eq!(focused.replies[0].1["ok"], true);
    assert_eq!(focused.native[0].client, 2);
    assert_eq!(focused.native[0].query["@type"], "readAllChatReactions");
    assert_eq!(app.notifier.focused, Some(key.clone()));
    assert_eq!(app.notifications_to_close, [17]);
    let malformed = app.request(
        &home,
        &mut runtime,
        7,
        req(11, "ui.focus", json!({"chatId":false})),
    );
    assert_eq!(malformed.replies[0].1["ok"], false);
    assert!(malformed.native.is_empty());
    assert_eq!(app.focus_peer, Some(6));
    app.retire_peer(7);
    assert_eq!(app.notifier.focused, Some(key));
    app.retire_peer(6);
    assert!(app.notifier.focused.is_none());
    app.settings.value["reactionsSeen"] = json!(false);
    let disabled = app.request(
        &home,
        &mut runtime,
        6,
        req(12, "ui.focus", json!({"chatId":42})),
    );
    assert!(disabled.native.is_empty());
    let cleared = app.request(&home, &mut runtime, 6, req(13, "ui.focus", json!({})));
    assert!(cleared.native.is_empty());
    assert!(app.focus_peer.is_none());
    assert!(app.notifier.focused.is_none());
    runtime.engine.accounts.get_mut("other").unwrap().ready = false;
    let not_ready = app.request(
        &home,
        &mut runtime,
        5,
        req(9, "ui.open", json!({"chatId":42})),
    );
    assert_eq!(not_ready.replies[0].1["error"], "not signed in");
    assert!(app.pending_open.is_none());
    // Quit has no readiness gate and is broadcast before its terminal reply.
    let quit = app.request(&home, &mut runtime, 5, req(14, "app.quit", json!({})));
    assert_eq!(quit.replies[0].0, 0);
    assert_eq!(quit.replies[0].1["event"], "quit");
    assert_eq!(quit.replies[1].1["ok"], true);
    assert!(app.quit_deadline.is_some());
    fs::remove_dir_all(root).unwrap();
}
