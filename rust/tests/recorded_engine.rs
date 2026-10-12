//! Real request/account owner, synthetic native sends only.
use serde_json::{Value, json};
use std::time::Instant;
use telebar_runtime::{
    commands::Action,
    engine::{Account, Engine},
    recorded_send::{Recording, Status},
    state::State,
    wire::Request,
};
fn engine() -> Engine {
    let mut e = Engine::new("default".into());
    let mut state = State::default();
    state
        .retained
        .reserve(Recording {
            token: "a".repeat(32),
            kind: "voice".into(),
            path: "/fixture/sent/voice.ogg".into(),
            source: None,
            chat: 42,
            args: json!({}),
            content: None,
            status: Status::Preparing,
            input: None,
        })
        .unwrap();
    e.accounts.insert(
        "default".into(),
        Account {
            client: 1,
            generation: 1,
            state,
            ready: true,
        },
    );
    e
}
fn request() -> Request {
    Request::parse(
        br#"{"id":1,"cmd":"recording.retry","args":{"token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}"#,
    )
    .unwrap()
}
fn action() -> Action {
    Action::Recorded {
        token: "a".repeat(32),
        content: json!({"@type":"inputMessageVoiceNote"}),
        options: Value::Null,
        input: None,
    }
}
#[test]
fn acknowledgements_rejections_timeouts_and_disconnects_keep_outcome_ownership() {
    let now = Instant::now();
    for event in [
        json!({"@type":"message","chat_id":43,"id":9}),
        json!({"@type":"message","chat_id":42,"id":0}),
        json!({"@type":"ok"}),
    ] {
        let mut e = engine();
        let effect = e.request_using(7, request(), now, &mut |_, _, _| Ok(action()));
        let mut event = event;
        event["@client_id"] = json!(1);
        event["@extra"] = effect.native[0].query["@extra"].clone();
        e.receive_at(event, now, &mut |_| None);
        assert_eq!(
            e.accounts["default"]
                .state
                .retained
                .record
                .as_ref()
                .unwrap()
                .status,
            Status::Unknown
        );
    }
    // A wrong account response cannot consume the original outcome owner.
    let mut e = engine();
    e.accounts.insert(
        "other".into(),
        Account {
            client: 2,
            generation: 1,
            state: State::default(),
            ready: true,
        },
    );
    let effect = e.request_using(7, request(), now, &mut |_, _, _| Ok(action()));
    let extra = effect.native[0].query["@extra"].clone();
    let wrong = e.receive_at(
        json!({"@type":"error","@client_id":2,"@extra":extra,"code":400}),
        now,
        &mut |_| None,
    );
    assert!(wrong.replies.is_empty());
    assert_eq!(
        e.accounts["default"]
            .state
            .retained
            .record
            .as_ref()
            .unwrap()
            .status,
        Status::Sending
    );
    let closed=e.receive_at(json!({"@type":"updateAuthorizationState","@client_id":1,"authorization_state":{"@type":"authorizationStateClosed"}}),now,&mut |_|None);
    assert!(
        closed
            .replies
            .iter()
            .any(|(_, r)| r["recordedSend"]["state"] == "unknown")
    );
    assert_eq!(
        e.accounts["default"]
            .state
            .retained
            .record
            .as_ref()
            .unwrap()
            .status,
        Status::Unknown
    );
    let mut e = engine();
    e.accounts
        .get_mut("default")
        .unwrap()
        .state
        .retained
        .record
        .as_mut()
        .unwrap()
        .status = Status::Rejected;
    e.reserve_external(telebar_runtime::wire::TD_PENDING_MAX);
    let full = e.request_using(7, request(), now, &mut |_, _, _| Ok(action()));
    assert!(full.native.is_empty());
    assert_eq!(
        e.accounts["default"]
            .state
            .retained
            .record
            .as_ref()
            .unwrap()
            .status,
        Status::Rejected
    );
    // Duplicate request teardown also exposes the unknown state to observers.
    let mut e = engine();
    e.request_using(7, request(), now, &mut |_, _, _| Ok(action()));
    let duplicate = e.request_using(7, request(), now, &mut |_, _, _| Ok(action()));
    assert_eq!(duplicate.disconnect, Some(7));
    assert_eq!(duplicate.replies[0].1["recordedSend"]["state"], "unknown");
    for code in [400, 408, 500, 0] {
        let mut e = engine();
        let effect = e.request_using(7, request(), now, &mut |_, _, _| Ok(action()));
        let q = &effect.native[0].query;
        let done=e.receive_at(json!({"@type":"error","@client_id":1,"@extra":q["@extra"],"code":code,"message":"synthetic"}),now,&mut |_|None);
        assert_eq!(done.replies.iter().filter(|(_, r)| r["id"] == 1).count(), 1);
        let retained = &mut e.accounts.get_mut("default").unwrap().state.retained;
        assert_eq!(
            retained.record.as_ref().unwrap().status,
            if code == 400 {
                Status::Rejected
            } else {
                Status::Unknown
            }
        );
        assert_eq!(retained.retry(&"a".repeat(32)).is_ok(), code == 400);
    }
    let mut e = engine();
    let effect = e.request_using(7, request(), now, &mut |_, _, _| Ok(action()));
    let extra = effect.native[0].query["@extra"].clone();
    let late=e.receive_at(json!({"@type":"message","@client_id":1,"@extra":extra,"chat_id":42,"id":9,"content":{"@type":"messageText","text":{"@type":"formattedText","text":"synthetic"}}}),now+telebar_runtime::lifecycle::REQUEST_TIMEOUT,&mut |_|None);
    assert!(late.native.is_empty());
    assert_eq!(
        e.accounts["default"]
            .state
            .retained
            .record
            .as_ref()
            .unwrap()
            .status,
        Status::Unknown
    );
    let mut e = engine();
    e.request_using(7, request(), now, &mut |_, _, _| Ok(action()));
    e.retire_peer(7);
    assert_eq!(
        e.accounts["default"]
            .state
            .retained
            .record
            .as_ref()
            .unwrap()
            .status,
        Status::Unknown
    );
    let mut e = engine();
    let effect = e.request_using(7, request(), now, &mut |_, _, _| Ok(action()));
    let done=e.receive_at(json!({"@type":"message","@client_id":1,"@extra":effect.native[0].query["@extra"],"chat_id":42,"id":9,"content":{"@type":"messageText","text":{"@type":"formattedText","text":"synthetic"}}}),now,&mut |_|None);
    assert!(e.accounts["default"].state.retained.record.is_none());
    assert_eq!(
        done.replies
            .iter()
            .filter(|(_, r)| r["id"] == 1 && r["ok"] == true)
            .count(),
        1
    );
}
