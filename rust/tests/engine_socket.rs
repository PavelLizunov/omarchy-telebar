use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn send(reader: &mut BufReader<UnixStream>, request: Value) -> Value {
    writeln!(reader.get_mut(), "{request}").unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(&line).unwrap()
}
#[test]
fn actual_engine_socket_routes_accounts_and_disconnects_duplicate_requests() {
    let dir = std::env::temp_dir().join(format!("telebar-engine-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    let mut child = Owned(
        Command::new(env!("CARGO_BIN_EXE_telebar-runtime"))
            .args(["--inert-engine", dir.to_str().unwrap()])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut line = String::new();
    BufReader::new(child.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let mut reader = BufReader::new(UnixStream::connect(dir.join("migration.sock")).unwrap());
    reader
        .get_mut()
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    for account in ["default", "other"] {
        assert_eq!(
            send(
                &mut reader,
                json!({"id":1,"cmd":"chat.history","account":account,"args":{"chatId":42}})
            ),
            json!({"id":1,"ok":true,"result":{"chatId":42,"messages":[]}})
        );
    }
    assert_eq!(
        send(
            &mut reader,
            json!({"id":2,"cmd":"chat.history","account":"missing","args":{"chatId":42}})
        )["error"],
        "unknown account"
    );
    assert_eq!(
        send(
            &mut reader,
            json!({"id":3,"cmd":"chats.list","args":{"limit":false}})
        )["error"],
        "limit must be an integer"
    );
    writeln!(
        reader.get_mut(),
        "{}",
        json!({"id":4,"cmd":"message.send","args":{"chatId":42,"text":"synthetic"}})
    )
    .unwrap();
    writeln!(
        reader.get_mut(),
        "{}",
        json!({"id":4,"cmd":"chat.history","args":{"chatId":42}})
    )
    .unwrap();
    line.clear();
    assert!(matches!(reader.read_line(&mut line), Ok(0) | Err(_)));
    // SAFETY: exact retained task-owned Child, no unrelated process discovery.
    assert_eq!(unsafe { libc::kill(child.0.id() as i32, libc::SIGTERM) }, 0);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!dir.join("migration.sock").exists());
    fs::remove_file(dir.join("migration.lock")).unwrap();
    fs::remove_dir(dir).unwrap();
}
