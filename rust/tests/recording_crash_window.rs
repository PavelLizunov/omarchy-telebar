//! Checkpoint/journal crash window and write refusal: no native/device access.
use serde_json::json;
use std::{
    fs,
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use telebar_runtime::{
    recorded_send::{Recording, Retained, Status},
    recording_checkpoint::Checkpoint,
    storage::Storage,
};
fn raw(path: &Path) -> Recording {
    Recording {
        token: "a".repeat(32),
        kind: "video".into(),
        path: path.to_str().unwrap().into(),
        source: None,
        chat: 42,
        args: json!({}),
        content: None,
        status: Status::Preparing,
        input: None,
    }
}
#[test]
fn unknown_and_corrupt_dismissal_never_releases_crash_window_false_journal() {
    let root =
        std::env::temp_dir().join(format!("telebar-checkpoint-window-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let store = Storage::open(&root).unwrap();
    fs::write(root.join("selected"), b"fixture").unwrap();
    for corrupt in [false, true] {
        let mut source = fs::File::open(root.join("selected")).unwrap();
        let copied = store
            .snapshot_upload(
                ".local/share/omagram/sent",
                &mut source,
                7,
                "voice.ogg",
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(2),
            )
            .unwrap();
        let path = root.join(&copied);
        let mut record = raw(&path);
        record.status = Status::Sending;
        record.content = Some(json!({"@type":"inputMessageVideoNote"}));
        let checkpoint = Checkpoint::open(&root, &root, "default").unwrap();
        checkpoint.save(Some(&record), true).unwrap();
        // Crash here: retained dispatched durable, generated-input journal false.
        if corrupt {
            store
                .write(
                    ".local/share/omagram/retained-recording-v1.json",
                    b"{bad",
                    0o600,
                )
                .unwrap();
        }
        let mut restored = Retained::default();
        restored
            .attach(Checkpoint::open(&root, &root, "default").unwrap())
            .unwrap();
        assert_eq!(restored.view("default")["state"], "unknown");
        let token = restored.view("default")["token"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(restored.retry(&token).is_err());
        restored.discard(&token).unwrap();
        drop(restored);
        assert_eq!(
            telebar_runtime::upload_lease::recover_unsent(&store, ".local/share/omagram/sent")
                .unwrap(),
            0
        );
        assert!(path.exists());
        assert!(
            Checkpoint::open(&root, &root, "default")
                .unwrap()
                .load()
                .unwrap()
                .is_none()
        );
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn checkpoint_write_failure_during_retry_rolls_back_live_state_without_worker() {
    use telebar_runtime::{
        engine::{Account, Engine},
        state::State,
        wire::Request,
    };
    let root =
        std::env::temp_dir().join(format!("telebar-checkpoint-refusal-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let store = Storage::open(&root).unwrap();
    store
        .write("omagram/rec/note.mp4", b"fixture", 0o600)
        .unwrap();
    let path = root.join("omagram/rec/note.mp4");
    let mut state = State::default();
    state
        .retained
        .attach(Checkpoint::open(&root, &root, "default").unwrap())
        .unwrap();
    state.retained.reserve(raw(&path)).unwrap();
    state.retained.prepare_failed(&"a".repeat(32)).unwrap();
    let metadata = root.join(".local/share/omagram/retained-recording-v1.json");
    fs::rename(&metadata, root.join("saved-checkpoint")).unwrap();
    fs::create_dir(&metadata).unwrap();
    let mut engine = Engine::new("default".into());
    engine.accounts.insert(
        "default".into(),
        Account {
            client: 41,
            generation: 1,
            state,
            ready: true,
        },
    );
    let request = Request::parse(
        format!(
            "{}",
            json!({"id":1,"cmd":"recording.retry","args":{"token":"a".repeat(32)}})
        )
        .as_bytes(),
    )
    .unwrap();
    let done = engine.request_using(7, request, Instant::now(), &mut |c, a, s| {
        telebar_runtime::recording_jobs::plan(c, a, s, root.clone(), &root, "default", 0.0)
    });
    assert!(done.work.is_empty() && done.native.is_empty());
    assert!(
        done.replies
            .iter()
            .any(|(_, v)| v["id"] == 1 && v["ok"] == false)
    );
    assert!(
        done.replies
            .iter()
            .any(|(_, v)| v["recordedSend"]["state"] == "prepareFailed")
    );
    assert_eq!(
        engine.accounts["default"].state.retained.view("default")["state"],
        "prepareFailed"
    );
    assert!(path.exists());
    fs::remove_dir_all(root).unwrap();
}
