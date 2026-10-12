//! Actual Driver consumed-capture error transition, synthetic ABI/lavfi only.
use serde_json::json;
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
use telebar_runtime::{
    application::Application, commands::Action, native_driver::Driver,
    recording_checkpoint::Checkpoint, storage::Storage, wire::Request,
};
#[test]
#[ignore = "requires explicit synthetic JSON-ABI fixture, no Telegram/accounts/devices"]
fn failed_checkpoint_capture_recovery_delivers_truthful_idle_and_resolvable_record() {
    let library = std::env::var_os("TELEBAR_TEST_INERT_ABI").expect("synthetic ABI");
    let root = std::env::temp_dir().join(format!(
        "telebar-record-failure-driver-{}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let store = Storage::open(&root).unwrap();
    let mut app = Application::load(&store).unwrap();
    let context = telebar_runtime::session::Context::new(
        root.clone(),
        root.clone(),
        Default::default(),
        "en".into(),
    )
    .unwrap();
    context.configure_application(&mut app).unwrap();
    let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
    context.register_catalog(&app, &mut driver).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !driver.runtime.engine.accounts["default"].ready {
        assert!(Instant::now() < deadline);
        driver
            .tick_application(&mut app, &store, &mut |_| Ok(None))
            .unwrap();
    }
    let capture = telebar_runtime::capture::Capture::start_with(
        &telebar_runtime::capture::Context {
            home: root.clone(),
            runtime: root.clone(),
            environment: Default::default(),
        },
        "default".into(),
        1,
        "voice",
        101,
        Some(
            ["-re", "-f", "lavfi", "-i", "sine=frequency=440:duration=3"]
                .map(str::to_owned)
                .to_vec(),
        ),
    )
    .unwrap();
    let path = capture.path.clone();
    let pid = capture.pid();
    store
        .ensure_dir(".local/share/omagram/retained-recording-v1.json")
        .unwrap();
    let Action::RecoverRecorded(record) = capture
        .prepare_send(json!({"send":true}), root.clone(), json!({}))
        .unwrap()
    else {
        panic!("explicit failure action");
    };
    assert!(path.exists());
    let mut action = Some(Action::RecoverRecorded(record));
    let done = driver
        .request_with_local(
            7,
            Request::parse(br#"{"id":1,"cmd":"voice.stop","args":{"send":true}}"#).unwrap(),
            &mut |_, _, _| Some(Ok(action.take().unwrap())),
        )
        .unwrap();
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
        driver.runtime.engine.accounts["default"]
            .state
            .retained
            .view("default")["state"],
        "prepareFailed"
    );
    assert!(!Path::new(&format!("/proc/{pid}")).exists());
    assert!(path.exists());
    // Restore only the injected directory so the live recovery can be resolved.
    fs::remove_dir(root.join(".local/share/omagram/retained-recording-v1.json")).unwrap();
    driver.runtime.engine.accounts["default"]
        .state
        .retained
        .persist()
        .unwrap();
    let token = driver.runtime.engine.accounts["default"]
        .state
        .retained
        .record
        .as_ref()
        .unwrap()
        .token
        .clone();
    let done = driver
        .request_application(
            &mut app,
            &store,
            7,
            Request::parse(
                json!({"id":2,"cmd":"recording.discard","args":{"token":token}})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap(),
        )
        .unwrap();
    assert!(
        done.replies
            .iter()
            .any(|(_, v)| v["id"] == 2 && v["ok"] == true)
    );
    assert!(!path.exists());
    assert!(
        Checkpoint::open(&root, &root, "default")
            .unwrap()
            .load()
            .unwrap()
            .is_none()
    );
    driver.close(Duration::from_secs(2)).unwrap();
    fs::remove_dir_all(root).unwrap();
}
