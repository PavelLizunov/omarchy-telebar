//! Synthetic ordered notification, action and sound policy; no actual native sends.
use serde_json::json;
use std::{
    fs,
    time::{Duration, Instant},
};
use telebar_runtime::{
    authorization::{Metadata, Runtime},
    engine::Account,
    notification_policy::{Decision, Policy},
    notifications::{Key, Notifier},
    state::State,
};
#[test]
fn notification_policy_preserves_quiet_focus_account_buttons_and_sound_gaps() {
    let mut runtime = Runtime::new("default".into());
    let mut state = State::default();
    state.chats.insert(42,json!({"id":42,"title":"Synthetic","kind":"private","notification":{"@type":"chatNotificationSettings","use_default_mute_for":true}}));
    state
        .users
        .insert(7, json!({"id":7,"name":"Synthetic sender"}));
    runtime.engine.accounts.insert(
        "default".into(),
        Account {
            client: 41,
            generation: 1,
            ready: true,
            state,
        },
    );
    runtime
        .metadata
        .insert("default".into(), Metadata::new("First".into()));
    runtime.engine.accounts.insert(
        "other".into(),
        Account {
            client: 42,
            generation: 3,
            ready: true,
            state: State::default(),
        },
    );
    runtime
        .metadata
        .insert("other".into(), Metadata::new("Second".into()));
    let mut policy = Policy::default();
    let mut notifier = Notifier::default();
    let key = Key {
        account: "default".into(),
        chat: 42,
    };
    let message = json!({"@type":"message","chat_id":42,"id":5,"sender_id":{"@type":"messageSenderUser","user_id":7},"content":{"@type":"messageText","text":{"@type":"formattedText","text":"<img> & synthetic","entities":[]}}});
    let event = json!({"chat_id":42,"total_count":2,"added_notifications":[{"@type":"notification","id":1,"is_silent":false,"type":{"@type":"notificationTypeNewMessage","show_preview":true,"message":message}}]});
    let Decision::Show(show) = policy.group(&runtime, &notifier, &json!({}), "default", &event)
    else {
        panic!("show")
    };
    assert_eq!(show.notification.title, "First • Synthetic (2)");
    assert_eq!(show.notification.body, "&lt;img&gt; &amp; synthetic");
    assert_eq!(show.seed, 7);
    assert_eq!(show.message, 5);
    let mut hidden = event.clone();
    hidden["added_notifications"][0]["type"]["show_preview"] = json!(false);
    let Decision::Show(show) = policy.group(&runtime, &notifier, &json!({}), "default", &hidden)
    else {
        panic!("hidden")
    };
    assert_eq!(show.notification.body, "New message");
    assert!(matches!(
        policy.group(
            &runtime,
            &notifier,
            &json!({"quiet":true}),
            "default",
            &event
        ),
        Decision::Close(_)
    ));
    notifier.focused = Some(key.clone());
    assert!(matches!(
        policy.group(&runtime, &notifier, &json!({}), "default", &event),
        Decision::None
    ));
    notifier.focused = None;
    let mut silent = event.clone();
    silent["added_notifications"][0]["is_silent"] = json!(true);
    assert!(matches!(
        policy.group(&runtime, &notifier, &json!({}), "default", &silent),
        Decision::None
    ));
    silent["total_count"] = json!(0);
    assert!(matches!(
        policy.group(&runtime, &notifier, &json!({}), "default", &silent),
        Decision::Close(_)
    ));
    let read = policy.button(&key, "read", &runtime).unwrap();
    assert_eq!(read["source"]["@type"], "messageSourceNotification");
    assert_eq!(read["message_ids"], json!([5]));
    assert_eq!(
        policy.button(&key, "mute", &runtime).unwrap()["notification_settings"]["mute_for"],
        3600
    );
    assert_eq!(
        policy.button(&key, "react", &runtime).unwrap()["reaction_type"]["emoji"],
        "👍"
    );
    assert!(policy.button(&key, "forged", &runtime).is_none());
    let now = Instant::now();
    assert!(policy.sound(7, &json!({}), None, now));
    assert!(!policy.sound(8, &json!({}), None, now + Duration::from_secs(1)));
    assert!(!policy.sound(7, &json!({}), None, now + Duration::from_secs(2)));
    assert!(policy.sound(8, &json!({}), None, now + Duration::from_secs(2)));
    assert!(policy.sound(7, &json!({}), None, now + Duration::from_secs(4)));
    let root = std::env::temp_dir().join(format!("telebar-notify-policy-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let home = telebar_runtime::storage::Storage::open(&root).unwrap();
    home.write(
        ".local/state/omarchy/notifications.json",
        br#"{"dnd":true}"#,
        0o600,
    )
    .unwrap();
    assert!(!policy.sound(9, &json!({}), Some(&home), now + Duration::from_secs(8)));
    assert!(
        telebar_runtime::notification_policy::picture(
            &home,
            &root,
            &[],
            &[json!({"path":"/etc/passwd"})]
        )
        .is_none()
    );
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode([255, 216, 255, 217]);
    let path =
        telebar_runtime::notification_policy::picture(&home, &root, &[], &[json!({"data":data})])
            .unwrap();
    assert_eq!(fs::read(path).unwrap(), [255, 216, 255, 217]);
    fs::remove_dir_all(root).unwrap();
}
