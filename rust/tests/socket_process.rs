use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
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

#[test]
fn real_socket_process_bounds_frames_and_cleans_up() {
    let directory =
        std::env::temp_dir().join(format!("telebar-rust-socket-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let executable = env!("CARGO_BIN_EXE_telebar-runtime");
    let mut child = Owned(
        Command::new(executable)
            .args(["--inert-socket", directory.to_str().unwrap()])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut ready = String::new();
    BufReader::new(child.0.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&ready).unwrap()["production_profiles"],
        false
    );
    let path = directory.join("migration.sock");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let mut reader = BufReader::new(UnixStream::connect(&path).unwrap());
    reader
        .get_mut()
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    for (payload, expected) in [
        (
            json!({"id":1,"cmd":"diagnostics.status"}),
            json!({"id":1,"ok":true,"result":{"runtime":"rust","migration_complete":false,"production_profiles":false}}),
        ),
        (
            json!({"id":2,"cmd":"hello"}),
            json!({"id":2,"ok":false,"error":"migration command is not implemented","code":0}),
        ),
        (
            json!({"id":false,"cmd":"diagnostics.status"}),
            json!({"id":null,"ok":false,"error":"id must be an integer","code":0}),
        ),
    ] {
        let bytes = format!("{payload}\n").into_bytes();
        for byte in bytes {
            reader.get_mut().write_all(&[byte]).unwrap();
        }
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap(), expected);
    }
    reader.get_mut().write_all(&vec![b'x'; 65537]).unwrap();
    let mut byte = [0];
    assert!(matches!(reader.read(&mut byte), Ok(0) | Err(_)));
    // SAFETY: the retained Child identifies our still-running process; no PID
    // discovery or foreign process is involved, and SIGTERM requests cleanup.
    assert_eq!(
        unsafe { libc::kill(i32::try_from(child.0.id()).unwrap(), libc::SIGTERM) },
        0
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let exit = loop {
        if let Some(exit) = child.0.try_wait().unwrap() {
            break exit;
        }
        assert!(
            Instant::now() < deadline,
            "owned socket process did not stop"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(exit.success());
    assert!(!path.exists());
    fs::remove_file(directory.join("migration.lock")).unwrap();
    fs::remove_dir(directory).unwrap();
}
