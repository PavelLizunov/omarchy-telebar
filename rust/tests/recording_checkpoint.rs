//! Durable retained UI metadata and explicit resolution, inert user-owned media.
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
fn record(path: &Path) -> Recording {
    Recording {
        token: "a".repeat(32),
        kind: "video".into(),
        path: path.to_str().unwrap().into(),
        source: None,
        chat: 42,
        args: json!({"topicId":7,"replyToMessageId":99,"silent":true}),
        content: None,
        status: Status::Preparing,
        input: None,
    }
}
#[test]
fn durable_unsent_and_dispatched_recovery_keeps_destination_and_never_auto_retries() {
    let root =
        std::env::temp_dir().join(format!("telebar-record-checkpoint-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let store = Storage::open(&root).unwrap();
    store
        .write("omagram/rec/note.mp4", b"inert original recording", 0o600)
        .unwrap();
    let path = root.join("omagram/rec/note.mp4");
    for (dispatch, rejected, expected) in [
        (false, false, "prepareFailed"),
        (true, false, "unknown"),
        (true, true, "prepareFailed"),
    ] {
        let mut retained = Retained::default();
        retained
            .attach(Checkpoint::open(&root, &root, "default").unwrap())
            .unwrap();
        retained.reserve(record(&path)).unwrap();
        retained.record.as_mut().unwrap().content = Some(json!({"@type":"inputMessageVideoNote"}));
        retained.sending(&"a".repeat(32)).unwrap();
        if dispatch {
            retained.native_dispatched(&"a".repeat(32)).unwrap();
        }
        if rejected {
            retained.reject(&"a".repeat(32), 400).unwrap();
        }
        drop(retained);
        let mut recovered = Retained::default();
        recovered
            .attach(Checkpoint::open(&root, &root, "default").unwrap())
            .unwrap();
        assert_eq!(recovered.view("default")["state"], expected);
        let r = recovered.record.as_ref().unwrap();
        assert_eq!(r.chat, 42);
        assert_eq!(
            r.args,
            json!({"topicId":7,"replyToMessageId":99,"silent":true})
        );
        assert!(r.content.is_none());
        assert!(path.exists());
        if expected == "unknown" {
            assert!(recovered.retry(&"a".repeat(32)).is_err());
        } else {
            assert!(recovered.retry(&"a".repeat(32)).unwrap());
        }
        recovered.discard(&"a".repeat(32)).unwrap();
        assert!(
            Checkpoint::open(&root, &root, "default")
                .unwrap()
                .load()
                .unwrap()
                .is_none()
        );
        if expected == "unknown" {
            assert!(path.exists());
        } else {
            assert!(!path.exists());
            store
                .write("omagram/rec/note.mp4", b"inert original recording", 0o600)
                .unwrap();
        }
    }
    // A changed inode restores as unknown; dismiss cannot delete replacement.
    let mut retained = Retained::default();
    retained
        .attach(Checkpoint::open(&root, &root, "default").unwrap())
        .unwrap();
    retained.reserve(record(&path)).unwrap();
    retained.prepare_failed(&"a".repeat(32)).unwrap();
    drop(retained);
    fs::rename(&path, root.join("raw-kept")).unwrap();
    fs::write(&path, b"raw replacement").unwrap();
    let mut recovered = Retained::default();
    recovered
        .attach(Checkpoint::open(&root, &root, "default").unwrap())
        .unwrap();
    assert_eq!(recovered.view("default")["state"], "unknown");
    assert!(recovered.retry(&"a".repeat(32)).is_err());
    recovered.discard(&"a".repeat(32)).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"raw replacement");
    // Per-account checkpoint scope cannot consume another account's input.
    let foreign = root.join("foreign");
    fs::write(&foreign, b"user keep").unwrap();
    let mut retained = Retained::default();
    retained
        .attach(Checkpoint::open(&root, &root, "other").unwrap())
        .unwrap();
    assert!(retained.reserve(record(&foreign)).is_err());
    assert!(foreign.exists());
    assert!(retained.record.is_none());
    // A missing source never reaches native dispatch, even if both the saved
    // and current metadata would otherwise say missing.
    let missing = root.join("omagram/rec/missing.mp4");
    retained.reserve(record(&missing)).unwrap();
    retained.record.as_mut().unwrap().content = Some(json!({"@type":"inputMessageVideoNote"}));
    retained.sending(&"a".repeat(32)).unwrap();
    assert!(retained.native_dispatched(&"a".repeat(32)).is_err());
    retained.fail_preparation(&"a".repeat(32));
    retained.discard(&"a".repeat(32)).unwrap();
    // Corrupt/escaping metadata refuses loading without accepting deletion.
    store
        .write(
            ".local/share/omagram/retained-recording-v1.json",
            b"{bad",
            0o600,
        )
        .unwrap();
    assert!(
        Checkpoint::open(&root, &root, "default")
            .unwrap()
            .load()
            .is_err()
    );
    assert!(path.exists());
    let mut blocked = Retained::default();
    blocked
        .attach(Checkpoint::open(&root, &root, "default").unwrap())
        .unwrap();
    assert!(blocked.recovery_blocked());
    assert_eq!(blocked.view("default")["state"], "unknown");
    assert!(blocked.reserve(record(&path)).is_err());
    blocked.discard(&"0".repeat(32)).unwrap();
    assert!(!blocked.recovery_blocked());
    assert!(path.exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn restored_generated_owner_protects_unsent_input_and_rejects_inode_substitution() {
    let root =
        std::env::temp_dir().join(format!("telebar-record-generated-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let store = Storage::open(&root).unwrap();
    fs::write(root.join("source"), b"synthetic").unwrap();
    let mut source = fs::File::open(root.join("source")).unwrap();
    let copied = store
        .snapshot_upload(
            ".local/share/omagram/sent",
            &mut source,
            9,
            "voice.ogg",
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap();
    let path = root.join(&copied);
    let mut retained = Retained::default();
    retained
        .attach(Checkpoint::open(&root, &root, "default").unwrap())
        .unwrap();
    retained.reserve(record(&path)).unwrap();
    retained.prepare_failed(&"a".repeat(32)).unwrap();
    drop(retained);
    let mut restored = Retained::default();
    restored
        .attach(Checkpoint::open(&root, &root, "default").unwrap())
        .unwrap();
    let protected = restored.protected_inputs(&root);
    assert_eq!(
        telebar_runtime::upload_lease::recover_unsent_preserving(
            &store,
            ".local/share/omagram/sent",
            &protected
        )
        .unwrap(),
        0
    );
    assert!(path.exists());
    fs::rename(&path, root.join("admitted")).unwrap();
    fs::write(&path, b"replacement keep").unwrap();
    assert!(restored.native_dispatched(&"a".repeat(32)).is_err());
    restored.discard(&"a".repeat(32)).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"replacement keep");
    // Reload after replacement is unknown and cannot enable retry/deletion.
    let checkpoint = Checkpoint::open(&root, &root, "default").unwrap();
    checkpoint
        .save(Some(&record(&root.join("admitted"))), false)
        .unwrap_err();
    assert_eq!(fs::read(root.join("source")).unwrap(), b"synthetic");
    fs::remove_dir_all(root).unwrap();
}
