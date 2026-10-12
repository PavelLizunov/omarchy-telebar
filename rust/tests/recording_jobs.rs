//! Real ffmpeg/finite worker and request owner, synthetic recordings only.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};
use telebar_runtime::{
    engine::{Account, Effects, Engine},
    jobs::Pool,
    recorded_send::Status,
    state::State,
    wire::Request,
};
fn req(id: i32, command: &str, args: Value) -> Request {
    Request::parse(
        json!({"id":id,"cmd":command,"args":args})
            .to_string()
            .as_bytes(),
    )
    .unwrap()
}
fn complete(engine: &mut Engine, effects: Effects) -> Effects {
    let (token, work) = effects.work.into_iter().next().unwrap();
    let mut pool = Pool::default();
    pool.start(token, work).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(done) = pool.poll(&|t| engine.job_current(t)).pop() {
            return engine.job_complete(done.token, done.result, Instant::now());
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn selected_recording_preparation_and_explicit_retry_are_admitted() {
    let root = std::env::temp_dir().join(format!("telebar-recording-jobs-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let runtime = root.join("runtime");
    let store = telebar_runtime::storage::Storage::open(&root).unwrap();
    store.ensure_dir("runtime/omagram/rec").unwrap();
    let source = runtime.join("omagram/rec/note.mp4");
    let out = telebar_runtime::process::run(
        "/usr/bin/ffmpeg",
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=80x60:r=10",
            "-t",
            "1.1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            source.to_str().unwrap(),
        ],
        &BTreeMap::new(),
        &[],
        Duration::from_secs(10),
        65536,
    )
    .unwrap();
    assert!(out.ok());
    let mut engine = Engine::new("default".into());
    engine.accounts.insert(
        "default".into(),
        Account {
            client: 1,
            generation: 1,
            state: State::default(),
            ready: true,
        },
    );
    let args = json!({"chatId":42,"path":source,"silent":true,"topicId":7});
    let mut plan = |c: &str, a: &Value, s: &State| {
        telebar_runtime::recording_jobs::plan(c, a, s, root.clone(), &runtime, "default", 0.0)
    };
    // Every validation precedes reservation and native work; symlinks rejected.
    let alias = runtime.join("omagram/rec/alias.mp4");
    std::os::unix::fs::symlink(&source, &alias).unwrap();
    assert!(
        plan(
            "videonote.send",
            &json!({"chatId":42,"path":alias}),
            &State::default()
        )
        .is_err()
    );
    let invalid = engine.request_using(
        7,
        req(
            1,
            "videonote.send",
            json!({"chatId":42,"path":source,"silent":"yes"}),
        ),
        Instant::now(),
        &mut plan,
    );
    assert!(invalid.work.is_empty());
    assert!(engine.accounts["default"].state.retained.record.is_none());
    let effects = engine.request_using(
        7,
        req(2, "videonote.send", args.clone()),
        Instant::now(),
        &mut plan,
    );
    assert_eq!(effects.replies[0].1["recordedSend"]["state"], "preparing");
    assert!(effects.native.is_empty());
    let token = engine.accounts["default"]
        .state
        .retained
        .record
        .as_ref()
        .unwrap()
        .token
        .clone();
    let duplicate = engine.request_using(
        8,
        req(3, "videonote.send", args.clone()),
        Instant::now(),
        &mut plan,
    );
    assert!(duplicate.work.is_empty());
    assert_eq!(duplicate.replies[0].1["ok"], false);
    let effects = complete(&mut engine, effects);
    assert_eq!(effects.native.len(), 1);
    let query = &effects.native[0].query;
    assert_eq!(query["options"]["disable_notification"], true);
    engine.native_dispatched(query).unwrap(); // Actual TDLib boundary retains uncertain input.
    let output = query["input_message_content"]["video_note"]["video_note"]["path"]
        .as_str()
        .unwrap();
    assert!(std::path::Path::new(output).starts_with(root.join(".local/share/omagram/sent")));
    assert_eq!(telebar_runtime::media::probe(output), [1, 480, 480]);
    assert!(source.exists());
    let refused =
        engine.receive(json!({"@type":"error","@client_id":1,"@extra":query["@extra"],"code":400}));
    assert_eq!(refused.replies[0].1["recordedSend"]["state"], "rejected");
    let retry = engine.request_using(
        8,
        req(4, "recording.retry", json!({"token":token})),
        Instant::now(),
        &mut plan,
    );
    assert_eq!(retry.native.len(), 1);
    assert!(retry.work.is_empty());
    assert_eq!(
        retry.native[0].query["input_message_content"],
        query["input_message_content"]
    );
    let retired = engine.retire_peer(8);
    assert_eq!(retired.replies[0].1["recordedSend"]["state"], "unknown");
    let retry = engine.request_using(
        8,
        req(5, "recording.retry", json!({"token":token})),
        Instant::now(),
        &mut plan,
    );
    assert!(retry.native.is_empty() && retry.work.is_empty());
    let discard = engine.request_using(
        8,
        req(8, "recording.discard", json!({"token":token})),
        Instant::now(),
        &mut plan,
    );
    assert_eq!(discard.replies[0].1["event"], "recordedSend");
    assert!(discard.replies[0].1["recordedSend"].is_null());
    assert!(discard.replies[1].1["ok"] == true);
    assert!(std::path::Path::new(output).exists() && source.exists());
    // A failed pre-send helper is retryable. Cancellation never publishes send.
    let bad = runtime.join("omagram/rec/bad.mp4");
    fs::write(&bad, b"not media").unwrap();
    let effects = engine.request_using(
        9,
        req(6, "videonote.send", json!({"chatId":42,"path":bad})),
        Instant::now(),
        &mut plan,
    );
    let effects = complete(&mut engine, effects);
    assert!(effects.native.is_empty());
    assert_eq!(
        engine.accounts["default"]
            .state
            .retained
            .record
            .as_ref()
            .unwrap()
            .status,
        Status::PrepareFailed
    );
    let token = engine.accounts["default"]
        .state
        .retained
        .record
        .as_ref()
        .unwrap()
        .token
        .clone();
    let effects = engine.request_using(
        9,
        req(7, "recording.retry", json!({"token":token})),
        Instant::now(),
        &mut plan,
    );
    assert_eq!(effects.work.len(), 1);
    let cancelled = engine.retire_peer(9);
    assert_eq!(
        cancelled.replies[0].1["recordedSend"]["state"],
        "prepareFailed"
    );
    let effects = complete(&mut engine, effects);
    assert!(effects.native.is_empty() && effects.replies.is_empty());
    let discard = engine.request_using(
        9,
        req(9, "recording.discard", json!({"token":token})),
        Instant::now(),
        &mut plan,
    );
    assert_eq!(discard.replies[1].1["ok"], true);
    assert!(!bad.exists());
    // A forged retained path must not expand deletion beyond owned media roots.
    fs::write(root.join("unrelated.txt"), b"keep").unwrap();
    engine
        .accounts
        .get_mut("default")
        .unwrap()
        .state
        .retained
        .record = Some(telebar_runtime::recorded_send::Recording {
        token: "b".repeat(32),
        kind: "voice".into(),
        path: root.join("unrelated.txt").to_str().unwrap().into(),
        source: None,
        chat: 42,
        args: json!({}),
        content: None,
        status: Status::PrepareFailed,
        input: None,
    });
    let refused = engine.request_using(
        9,
        req(10, "recording.discard", json!({"token":"b".repeat(32)})),
        Instant::now(),
        &mut plan,
    );
    assert_eq!(refused.replies[0].1["ok"], false);
    assert!(root.join("unrelated.txt").exists());
    assert!(engine.accounts["default"].state.retained.record.is_some());
    fs::remove_dir_all(root).unwrap();
}
