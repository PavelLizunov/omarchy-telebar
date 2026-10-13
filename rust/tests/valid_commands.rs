//! Previously error-only routes: valid plans and actual Engine outcomes.
//! No sockets, Telegram, filesystem, credentials, mutations or device helpers.
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use telebar_runtime::{
    commands::Action,
    engine::{Account, Engine},
    state::State,
    wire::Request,
};
fn cases() -> Vec<(&'static str, Value, Vec<Value>, Value)> {
    let bot = vec![
        json!({"@type":"updateNewChat","chat":{"@type":"chat","id":43,"title":"Synthetic bot","type":{"@type":"chatTypePrivate","user_id":7}}}),
    ];
    let secret = vec![
        json!({"@type":"updateNewChat","chat":{"@type":"chat","id":45,"title":"Synthetic secret","type":{"@type":"chatTypeSecret","user_id":7,"secret_chat_id":6}}}),
    ];
    let folders = vec![
        json!({"@type":"updateChatFolders","chat_folders":[{"@type":"chatFolderInfo","id":10},{"@type":"chatFolderInfo","id":2}],"main_chat_list_position":1}),
    ];
    vec![
        (
            "bot.start",
            json!({"chatId":43,"parameter":"synthetic"}),
            bot,
            json!({"@type":"message","id":13,"chat_id":43,"content":{"@type":"messageText","text":{"text":"synthetic"}}}),
        ),
        (
            "secret.close",
            json!({"chatId":45}),
            secret,
            json!({"@type":"ok"}),
        ),
        (
            "thread.history",
            json!({"chatId":42,"threadId":8,"offset":-5,"limit":20}),
            vec![],
            json!({"@type":"messages","messages":[{"@type":"message","chat_id":42,"id":13,"content":{"@type":"messageText","text":{"text":"synthetic thread"}}}]}),
        ),
        (
            "folder.save",
            json!({"name":"Synthetic","includeGroups":true,"icon":"Work","colorId":3,"included":[42,43,42],"excluded":[44]}),
            vec![],
            json!({"@type":"chatFolderInfo","id":10}),
        ),
        (
            "folders.reorder",
            json!({"ids":[2,10]}),
            folders,
            json!({"@type":"ok"}),
        ),
        (
            "proxy.add",
            json!({"server":"127.0.0.1","port":123,"type":"socks5","username":"synthetic","password":"fixture"}),
            vec![],
            json!({"@type":"addedProxy","id":9,"is_enabled":true,"proxy":{"@type":"proxy","server":"127.0.0.1","port":123,"type":{"@type":"proxyTypeSocks5"}}}),
        ),
    ]
}
#[test]
fn formerly_error_only_routes_have_valid_account_owned_native_results() {
    for (command, args, updates, response) in cases() {
        let mut state = State::default();
        for u in &updates {
            state.apply(u);
        }
        let Action::Native { query, .. } =
            telebar_runtime::sending::plan(command, &args, &state, 0.0, "en", &mut |_| None)
                .unwrap()
        else {
            panic!("native valid {command}")
        };
        if command == "thread.history" {
            assert_eq!(query["message_id"], 8);
            assert!(query.get("message_thread_id").is_none());
            assert_eq!(query["offset"], -5);
            assert_eq!(query["limit"], 20);
        }
        if command == "folder.save" {
            assert_eq!(query["folder"]["included_chat_ids"], json!([42, 43]));
        }
        for (code, outcome) in [
            (0, response.clone()),
            (
                400,
                json!({"@type":"error","code":400,"message":"synthetic refusal"}),
            ),
        ] {
            let mut owned_state = State::default();
            for u in &updates {
                owned_state.apply(u);
            }
            let mut engine = Engine::new("default".into());
            engine.accounts.insert(
                "default".into(),
                Account {
                    client: 41,
                    generation: 1,
                    state: owned_state,
                    ready: true,
                },
            );
            engine.accounts.insert(
                "other".into(),
                Account {
                    client: 42,
                    generation: 1,
                    state: State::default(),
                    ready: true,
                },
            );
            let request = Request::parse(
                json!({"id":7,"cmd":command,"args":args})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
            let now = Instant::now();
            let sent = engine.request(3, request, now);
            assert_eq!(sent.native.len(), 1);
            assert_eq!(sent.native[0].client, 41);
            let extra = sent.native[0].query["@extra"].clone();
            let mut event = outcome;
            event["@extra"] = extra.clone();
            event["@client_id"] = json!(42);
            let wrong = engine.receive(event.clone());
            assert!(wrong.replies.is_empty() && wrong.native.is_empty());
            event["@client_id"] = json!(41);
            let done = engine.receive(event.clone());
            assert_eq!(done.replies.len(), 1, "{command}");
            assert_eq!(done.replies[0].0, 3);
            assert_eq!(done.replies[0].1["id"], 7);
            assert_eq!(done.replies[0].1["ok"], code == 0);
            if code == 400 {
                assert_eq!(done.replies[0].1["error"], "synthetic refusal");
                assert_eq!(done.replies[0].1["code"], 400);
            } else if command == "thread.history" {
                assert_eq!(done.replies[0].1["result"]["threadId"], 8);
                assert_eq!(
                    done.replies[0].1["result"]["messages"][0]["content"]["text"],
                    "synthetic thread"
                );
            }
            assert!(engine.receive(event).replies.is_empty());
            assert_eq!(engine.native_pending(), 0);
        }
    }
    // Local expiry never retries or lets a late native mutation satisfy an ID
    // reused by a newer account-owned request.
    let mut engine = Engine::new("default".into());
    engine.accounts.insert(
        "default".into(),
        Account {
            client: 41,
            generation: 1,
            state: State::default(),
            ready: true,
        },
    );
    let now = Instant::now();
    let request = || {
        Request::parse(br#"{"id":8,"cmd":"thread.history","args":{"chatId":42,"threadId":9}}"#)
            .unwrap()
    };
    let first = engine.request(3, request(), now);
    let extra = first.native[0].query["@extra"].clone();
    assert!(
        engine
            .expire(now + Duration::from_secs(181))
            .native
            .is_empty()
    );
    let fresh = engine.request(3, request(), now + Duration::from_secs(181));
    assert_ne!(fresh.native[0].query["@extra"], extra);
    let late = engine.receive_at(
        json!({"@type":"messages","messages":[],"@client_id":41,"@extra":extra}),
        now + Duration::from_secs(181),
        &mut |_| None,
    );
    assert!(late.native.is_empty() && late.replies.is_empty());
}
