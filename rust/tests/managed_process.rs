//! Real pollable helpers, bounded lifetime and exact drop/shutdown ownership.
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use telebar_runtime::process::{Managed, Output};
fn wait(child: &mut Managed) -> Output {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(output) = child.poll().unwrap() {
            return output;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn managed_helpers_finish_drain_signal_timeout_and_drop_without_orphans() {
    let mut echo = Managed::start(
        "/usr/bin/cat",
        &[],
        &BTreeMap::new(),
        b"synthetic",
        Duration::from_secs(2),
        64,
    )
    .unwrap();
    let output = wait(&mut echo);
    assert!(output.ok());
    assert_eq!(output.stdout, b"synthetic");
    assert!(echo.poll().unwrap().is_none());
    let mut noisy = Managed::start(
        "/usr/bin/yes",
        &[],
        &BTreeMap::new(),
        &[],
        Duration::from_secs(2),
        64,
    )
    .unwrap();
    let output = wait(&mut noisy);
    assert!(output.truncated);
    assert_eq!(output.stdout.len(), 64);
    let mut sleeping = Managed::start(
        "/usr/bin/sleep",
        &["10"],
        &BTreeMap::new(),
        &[],
        Duration::from_secs(2),
        64,
    )
    .unwrap();
    let pid = sleeping.pid();
    sleeping
        .signal_stop(libc::SIGTERM, Duration::from_millis(50))
        .unwrap();
    let output = wait(&mut sleeping);
    assert!(!output.ok());
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    let mut timed = Managed::start(
        "/usr/bin/sleep",
        &["10"],
        &BTreeMap::new(),
        &[],
        Duration::from_millis(20),
        64,
    )
    .unwrap();
    assert!(wait(&mut timed).timed_out);
    let dropped = Managed::start(
        "/usr/bin/sleep",
        &["10"],
        &BTreeMap::new(),
        &[],
        Duration::from_secs(2),
        64,
    )
    .unwrap();
    let pid = dropped.pid();
    drop(dropped);
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    assert!(
        Managed::start(
            "sleep",
            &[],
            &BTreeMap::new(),
            &[],
            Duration::from_secs(1),
            64
        )
        .is_err()
    );
}
