//! Abrupt test-owned process exit and real restored consumer socket, no TDLib.
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use telebar_runtime::{
    recorded_send::{Recording, Retained, Status},
    recording_checkpoint::Checkpoint,
    storage::Storage,
};
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn start(home: &Path) -> Owned {
    let mut child = Owned(
        Command::new(env!("CARGO_BIN_EXE_telebar-runtime"))
            .args(["--inert-no-library", home.to_str().unwrap()])
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "isolated owner stopped before readiness"
        );
        if UnixStream::connect(home.join("migration.sock")).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    child
}
fn request(home: &Path, id: u64, command: &str, args: Value) -> Value {
    let socket = UnixStream::connect(home.join("migration.sock")).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut reader = BufReader::new(socket);
    reader
        .get_mut()
        .write_all(format!("{}\n", json!({"id":id,"cmd":command,"args":args})).as_bytes())
        .unwrap();
    for _ in 0..50 {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0);
        let value: Value = serde_json::from_str(&line).unwrap();
        if value["id"] == id {
            return value;
        }
    }
    panic!("missing bounded response")
}
#[test]
fn abrupt_owner_exit_restores_recorded_send_and_explicit_dismiss_is_durable() {
    let root = std::env::temp_dir().join(format!("telebar-recovery-socket-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let store = Storage::open(&root).unwrap();
    for dispatched in [false, true] {
        store
            .write("omagram/rec/note.mp4", b"inert media retained", 0o600)
            .unwrap();
        let path = root.join("omagram/rec/note.mp4");
        let token = "a".repeat(32);
        let mut retained = Retained::default();
        retained
            .attach(Checkpoint::open(&root, &root, "default").unwrap())
            .unwrap();
        retained
            .reserve(Recording {
                token: token.clone(),
                kind: "video".into(),
                path: path.to_str().unwrap().into(),
                source: None,
                chat: 101,
                args: json!({"replyToMessageId":2,"silent":true}),
                content: Some(json!({"@type":"inputMessageVideoNote"})),
                status: Status::Preparing,
                input: None,
            })
            .unwrap();
        retained.sending(&token).unwrap();
        if dispatched {
            retained.native_dispatched(&token).unwrap();
        }
        drop(retained);
        let mut child = start(&root);
        let hello = request(&root, 1, "hello", json!({"window":true}));
        assert_eq!(hello["ok"], true);
        let expected = if dispatched {
            "unknown"
        } else {
            "prepareFailed"
        };
        assert_eq!(hello["result"]["recordedSend"]["state"], expected);
        assert_eq!(hello["result"]["recordedSend"]["chatId"], 101);
        // Only the retained disposable child is killed; no production owner or
        // Telegram request exists. Kernel file locks retire despite abrupt exit.
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        drop(child);
        let mut child = start(&root);
        assert_eq!(
            request(&root, 2, "hello", json!({}))["result"]["recordedSend"]["state"],
            expected
        );
        if dispatched {
            assert_eq!(
                request(&root, 3, "recording.retry", json!({"token":token}))["ok"],
                false
            );
        }
        let discard = request(&root, 4, "recording.discard", json!({"token":token}));
        assert_eq!(discard["ok"], true);
        assert_eq!(path.exists(), dispatched);
        assert!(request(&root, 5, "hello", json!({}))["result"]["recordedSend"].is_null());
        assert_eq!(request(&root, 6, "app.quit", json!({}))["ok"], true);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(exit) = child.0.try_wait().unwrap() {
                assert!(exit.success());
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        drop(child);
        assert!(!root.join("migration.sock").exists());
        let mut child = start(&root);
        assert!(request(&root, 7, "hello", json!({}))["result"]["recordedSend"].is_null());
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        drop(child);
    }
    assert!(!root.join(".local/share/omagram/database").exists());
    fs::remove_dir_all(root).unwrap();
}
