//! Path discard never consumes a retained or unrelated recording.
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
use telebar_runtime::{
    commands::Action,
    recorded_send::{Recording, Status},
    recording_jobs,
    state::State,
    storage::Storage,
};
#[test]
fn raw_discard_is_narrow_and_retained_sources_require_tokens() {
    let root = std::env::temp_dir().join(format!("telebar-raw-discard-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let store = Storage::open(&root).unwrap();
    store
        .write("omagram/rec/note.mp4", b"synthetic", 0o600)
        .unwrap();
    store.write("foreign", b"keep", 0o600).unwrap();
    let path = root.join("omagram/rec/note.mp4");
    let mut state = State::default();
    state
        .retained
        .reserve(Recording {
            token: "a".repeat(32),
            kind: "video".into(),
            path: path.to_str().unwrap().into(),
            source: None,
            chat: 42,
            args: json!({}),
            content: None,
            status: Status::Preparing,
        })
        .unwrap();
    assert!(
        recording_jobs::plan(
            "videonote.discard",
            &json!({"path":path}),
            &state,
            root.clone(),
            &root,
            "default",
            0.0
        )
        .is_err()
    );
    assert!(path.exists());
    state.retained.record = None;
    assert!(
        recording_jobs::plan(
            "videonote.discard",
            &json!({"path":root.join("foreign")}),
            &state,
            root.clone(),
            &root,
            "default",
            0.0
        )
        .is_err()
    );
    assert_eq!(fs::read(root.join("foreign")).unwrap(), b"keep");
    symlink(root.join("foreign"), root.join("omagram/rec/link.mp4")).unwrap();
    assert!(
        recording_jobs::plan(
            "videonote.discard",
            &json!({"path":root.join("omagram/rec/link.mp4")}),
            &state,
            root.clone(),
            &root,
            "default",
            0.0
        )
        .is_err()
    );
    let result = recording_jobs::plan(
        "videonote.discard",
        &json!({"path":path}),
        &state,
        root.clone(),
        &root,
        "default",
        0.0,
    )
    .unwrap();
    assert!(matches!(result, Action::Reply(_)));
    assert!(!path.exists());
    assert_eq!(fs::read(root.join("foreign")).unwrap(), b"keep");
    fs::remove_dir_all(root).unwrap();
}
