//! Actual Driver/FFI boundary refusal with disposable synthetic ABI only.
use serde_json::json;
use std::{
    fs,
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use telebar_runtime::{
    commands::{Action, Answer},
    native_driver::Driver,
    storage::Storage,
    upload_lease::Lease,
    wire::Request,
};
#[test]
#[ignore = "requires the explicit synthetic no-network JSON-ABI fixture library"]
fn failed_input_journal_refuses_native_submission_without_retiring_the_service() {
    let library = std::env::var_os("TELEBAR_TEST_INERT_ABI").expect("explicit synthetic ABI");
    let root = std::env::temp_dir().join(format!("telebar-input-driver-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let store = Storage::open(&root).unwrap();
    fs::write(root.join("selected"), b"fixture user bytes").unwrap();
    let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
    driver
        .register("default".into(), "Fixture".into(), vec![])
        .unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    while !driver.runtime.engine.accounts["default"].ready {
        assert!(Instant::now() < until);
        driver.tick(&mut |_| Ok(None)).unwrap();
    }
    let mut source = fs::File::open(root.join("selected")).unwrap();
    let copied = store
        .snapshot_upload(
            "sent",
            &mut source,
            18,
            "selected",
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap();
    let mut lease = Lease::new(root.clone()).unwrap();
    lease.add(copied.clone()).unwrap();
    let parent = Path::new(&copied).parent().unwrap().to_str().unwrap();
    store
        .write(
            &format!("{parent}/{}", telebar_runtime::upload_lease::JOURNAL),
            b"{}",
            0o600,
        )
        .unwrap();
    let mut action = Some(Action::Native {
        query: json!({"@type":"sendMessage"}),
        answer: Answer::Upload(lease, false),
    });
    let request = Request::parse(br#"{"id":1,"cmd":"message.sendFile","args":{}}"#).unwrap();
    let refused = driver
        .request_with_local(7, request, &mut |_, _, _| Some(Ok(action.take().unwrap())))
        .unwrap();
    assert_eq!(refused.replies.len(), 1);
    assert_eq!(refused.replies[0].1["ok"], false);
    assert_eq!(
        refused.replies[0].1["error"],
        "the generated input could not be retained; no message was submitted"
    );
    assert!(!root.join(copied).exists());
    // Actual native dispatch refuses a regular replacement absolute root too.
    let mut source = fs::File::open(root.join("selected")).unwrap();
    let copied = store
        .snapshot_upload(
            "sent",
            &mut source,
            18,
            "selected",
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap();
    let mut lease = Lease::new(root.clone()).unwrap();
    lease.add(copied.clone()).unwrap();
    let moved = root.with_extension("moved");
    fs::rename(&root, &moved).unwrap();
    fs::create_dir(&root).unwrap();
    Storage::open(&root)
        .unwrap()
        .write(&copied, b"foreign root bytes", 0o600)
        .unwrap();
    let mut action = Some(Action::Native {
        query: json!({"@type":"sendMessage"}),
        answer: Answer::Upload(lease, false),
    });
    let refused = driver
        .request_with_local(
            7,
            Request::parse(br#"{"id":3,"cmd":"message.sendFile","args":{}}"#).unwrap(),
            &mut |_, _, _| Some(Ok(action.take().unwrap())),
        )
        .unwrap();
    assert_eq!(
        refused.replies[0].1["error"],
        "the generated input could not be retained; no message was submitted"
    );
    assert_eq!(fs::read(root.join(&copied)).unwrap(), b"foreign root bytes");
    assert!(!moved.join(copied).exists());
    fs::remove_dir_all(&root).unwrap();
    fs::rename(&moved, &root).unwrap();
    let healthy = driver
        .request(
            7,
            Request::parse(br#"{"id":2,"cmd":"chat.history","args":{"chatId":101}}"#).unwrap(),
        )
        .unwrap();
    assert!(healthy.replies.is_empty());
    let until = Instant::now() + Duration::from_secs(2);
    let mut answered = false;
    while Instant::now() < until && !answered {
        answered = driver
            .tick(&mut |_| Ok(None))
            .unwrap()
            .replies
            .iter()
            .any(|(_, v)| v["id"] == 2 && v["ok"] == true);
    }
    assert!(
        answered,
        "the actual native consumer still answers after the local refusal"
    );
    assert!(driver.runtime.engine.accounts["default"].ready);
    driver.close(Duration::from_secs(2)).unwrap();
    assert_eq!(
        fs::read(root.join("selected")).unwrap(),
        b"fixture user bytes"
    );
    fs::remove_dir_all(root).unwrap();
}
