//! Synthetic lavfi only: no microphone, camera, PipeWire or Telegram sends.
use serde_json::json;
use std::{fs, sync::atomic::AtomicBool, time::Duration};
use telebar_runtime::{
    capture::{Capture, Context},
    commands::Action,
};
fn input(kind: &str) -> Vec<String> {
    if kind == "voice" {
        ["-re", "-f", "lavfi", "-i", "sine=frequency=400:duration=3"]
            .map(str::to_owned)
            .to_vec()
    } else {
        [
            "-re",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=10:duration=3",
            "-re",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=400:duration=3",
        ]
        .map(str::to_owned)
        .to_vec()
    }
}
#[test]
fn synthetic_owned_capture_stops_and_prepares_retained_inputs_without_sends() {
    let root = std::env::temp_dir().join(format!("telebar-capture-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let context = Context {
        home: root.clone(),
        runtime: root.clone(),
        environment: Default::default(),
    };
    for kind in ["voice", "video"] {
        let mut capture =
            Capture::start_with(&context, "default".into(), 1, kind, 42, Some(input(kind)))
                .unwrap();
        let pid = capture.pid();
        let preview = capture.preview.clone();
        std::thread::sleep(Duration::from_millis(1300));
        assert!(!capture.poll(), "synthetic recorder exited early");
        let Action::PrepareRecorded {
            recording: Some(record),
            preparation,
        } = capture
            .prepare_send(json!({"send":true}), root.clone(), json!({}))
            .unwrap()
        else {
            panic!("recording")
        };
        assert_eq!(record.kind, kind);
        assert_eq!(record.chat, 42);
        let Action::Recorded { content, input, .. } =
            preparation.run(&AtomicBool::new(false)).unwrap()
        else {
            panic!("prepared content")
        };
        let field = if kind == "voice" {
            "voice_note"
        } else {
            "video_note"
        };
        let path = content[field][field]["path"].as_str().unwrap();
        assert!(fs::metadata(path).unwrap().len() > 0);
        assert!(content[field]["duration"].as_u64().unwrap() >= 1);
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        if let Some(preview) = preview {
            assert!(!preview.exists());
        }
        // A worker result never dispatched to TDLib owns cleanup, not a
        // submitted flag set prematurely inside the preparation worker.
        let owned = path.to_owned();
        drop(input);
        assert!(!std::path::Path::new(&owned).exists());
    }
    let capture = Capture::start_with(
        &context,
        "default".into(),
        1,
        "voice",
        42,
        Some(input("voice")),
    )
    .unwrap();
    let path = capture.path.clone();
    let pid = capture.pid();
    std::thread::sleep(Duration::from_millis(200));
    let Action::Work(telebar_runtime::jobs::Work::StopCapture(stop)) = capture
        .stop(json!({"send":false}), root.clone(), json!({}))
        .unwrap()
    else {
        panic!("stop")
    };
    assert!(matches!(
        stop.run(&AtomicBool::new(false)).unwrap(),
        Action::Reply(_)
    ));
    assert!(!path.exists());
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    let capture = Capture::start_with(
        &context,
        "default".into(),
        1,
        "video",
        42,
        Some(input("video")),
    )
    .unwrap();
    let path = capture.path.clone();
    let preview = capture.preview.clone().unwrap();
    let pid = capture.pid();
    capture.discard();
    assert!(!path.exists());
    assert!(!preview.exists());
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    let capture = Capture::start_with(
        &context,
        "default".into(),
        1,
        "voice",
        42,
        Some(input("voice")),
    )
    .unwrap();
    let path = capture.path.clone();
    fs::rename(&path, root.join("original-output")).unwrap();
    fs::write(&path, b"unrelated replacement").unwrap();
    capture.discard();
    assert_eq!(fs::read(&path).unwrap(), b"unrelated replacement");
    // A regular replaced root must not turn Drop into foreign cleanup.
    let capture = Capture::start_with(
        &context,
        "default".into(),
        1,
        "voice",
        42,
        Some(input("voice")),
    )
    .unwrap();
    let relative = capture.path.strip_prefix(&root).unwrap().to_owned();
    let moved = root.with_extension("moved");
    fs::rename(&root, &moved).unwrap();
    fs::create_dir(&root).unwrap();
    let replacement = telebar_runtime::storage::Storage::open(&root).unwrap();
    replacement
        .write(relative.to_str().unwrap(), b"foreign root input", 0o600)
        .unwrap();
    drop(capture);
    assert_eq!(
        fs::read(root.join(relative)).unwrap(),
        b"foreign root input"
    );
    fs::remove_dir_all(&moved).unwrap();
    fs::remove_dir_all(root).unwrap();
}
