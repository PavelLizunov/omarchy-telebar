//! Numeric-only local diagnostics and admitted UI reports, no real profile.
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    time::{Duration, Instant},
};
use telebar_runtime::{
    application::Application,
    authorization::Runtime,
    diagnostics::{Diagnostics, Event, Phase, numeric_fields, process_metrics},
    storage::Storage,
    wire::Request,
};
#[test]
fn diagnostic_writer_rotates_refuses_links_and_watchdog_observes_blocked_owner() {
    let root = std::env::temp_dir().join(format!("telebar-diagnostics-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let home = Storage::open(&root).unwrap();
    assert_eq!(
        numeric_fields(
            &json!({"pending":7,"visible":true,"password":"PRIVATE-CONTENT","height":-1,"tx_requests":1e100})
        ),
        json!({"pending":7,"visible":true})
    );
    let log = Diagnostics::start_with(&home, 1200, 2).unwrap();
    for _ in 0..100 {
        log.emit(Event::UiSample,json!({"metrics":{"pending":7,"PRIVATE-KEY-NAME":"PRIVATE-CONTENT","oldest_ms":"PRIVATE-CONTENT"}}));
    }
    drop(log);
    let directory = root.join(".local/state/omagram/diagnostics");
    let entries: Vec<_> = fs::read_dir(&directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert!((2..=3).contains(&entries.len()));
    for file in &entries {
        assert_eq!(file.metadata().unwrap().permissions().mode() & 0o777, 0o600);
        assert!(file.metadata().unwrap().len() <= 1200);
        let text = fs::read_to_string(file).unwrap();
        assert!(!text.contains("PRIVATE"));
        for line in text.lines() {
            assert!(serde_json::from_str::<Value>(line).unwrap().is_object());
        }
        fs::remove_file(file).unwrap();
    }
    fs::write(root.join("untouched"), b"original").unwrap();
    symlink(root.join("untouched"), directory.join("trace.jsonl")).unwrap();
    let log = Diagnostics::start(&home).unwrap();
    std::thread::sleep(Duration::from_millis(120));
    assert_eq!(log.counters()["logger_write_errors"], 1);
    drop(log);
    assert_eq!(fs::read(root.join("untouched")).unwrap(), b"original");
    fs::remove_file(directory.join("trace.jsonl")).unwrap();
    let log = Diagnostics::start(&home).unwrap();
    log.progress(Phase::Native);
    // No owner progress call during this finite wait: independent watchdog must
    // detect it, not a manually generated stall event or test-only clock override.
    std::thread::sleep(Duration::from_millis(2800));
    drop(log);
    let records: Vec<Value> = fs::read_to_string(directory.join("trace.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let stall = records
        .iter()
        .find(|r| r["event"] == "main_loop_stall")
        .unwrap();
    assert_eq!(stall["phase"], "td_receive");
    assert_eq!(stall["stack_capture_supported"], false);
    let metrics = process_metrics(std::process::id());
    assert!(metrics["rss_kb"].as_u64().unwrap() > 0);
    assert!(metrics.get("cmdline").is_none());
    let mut app = Application::load(&home).unwrap();
    let mut runtime = Runtime::new("default".into());
    runtime
        .register(
            "default".into(),
            "Synthetic".into(),
            41,
            vec![],
            Instant::now(),
        )
        .unwrap();
    app.diagnostic_peers.connect(7, std::process::id());
    let req = |id, command, args| {
        Request::parse(
            json!({"id":id,"cmd":command,"args":args})
                .to_string()
                .as_bytes(),
        )
        .unwrap()
    };
    let report = app.request(
        &home,
        &mut runtime,
        7,
        req(
            1,
            "diagnostics.ui",
            json!({"window":true,"pending":3,"password":"PRIVATE-CONTENT"}),
        ),
    );
    assert_eq!(report.replies[0].1["ok"], true);
    assert!(app.windows.contains(&7));
    let limited = app.request(
        &home,
        &mut runtime,
        7,
        req(2, "diagnostics.ui", json!({"pending":99})),
    );
    assert_eq!(
        limited.replies[0].1["error"],
        "diagnostic reports are rate limited"
    );
    let status = app.request(
        &home,
        &mut runtime,
        7,
        req(3, "diagnostics.status", json!({})),
    );
    let view = &status.replies[0].1["result"];
    assert_eq!(view["ui"][0]["metrics"], json!({"pending":3}));
    assert_eq!(view["ui"][0]["surface"], "window");
    assert_eq!(view["account_count"], 1);
    assert_eq!(view["pending_ipc"], 0);
    let request = req(4, "chat.history", json!({"chatId":42}));
    runtime.engine.accounts.get_mut("default").unwrap().ready = true;
    let pending = runtime.engine.request(7, request, Instant::now());
    assert_eq!(pending.native.len(), 1);
    let duplicate = app.request(
        &home,
        &mut runtime,
        7,
        req(4, "diagnostics.ui", json!({"pending":99})),
    );
    assert_eq!(duplicate.disconnect, Some(7));
    app.retire_peer(7);
    assert!(app.diagnostic_peers.is_empty());
    fs::remove_dir_all(root).unwrap();
}
