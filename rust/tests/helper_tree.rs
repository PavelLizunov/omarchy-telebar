//! Actual descendant-lifecycle regression, isolated in this integration process.
use std::{collections::BTreeMap, time::Duration};
#[test]
fn fixture_leader() {
    if std::env::var("TELEBAR_INERT_FORK").as_deref() != Ok("1") {
        return;
    }
    // SAFETY: isolated inert test process. Child uses only async-signal-safe
    // libc calls after fork, no inherited application/profile state exists.
    let pid = unsafe { libc::fork() };
    assert!(pid >= 0);
    if pid == 0 {
        unsafe {
            libc::sleep(30);
            libc::_exit(0);
        }
    }
    println!("DESCENDANT={pid}");
    // Exit leader without waiting, deliberately retaining the child's pipes.
}
#[test]
fn exited_leader_does_not_leave_owned_descendants() {
    if std::env::var("TELEBAR_INERT_FORK").is_ok() {
        return;
    }
    // SAFETY: only this integration-test process adopts its disposable children.
    assert_eq!(
        unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) },
        0
    );
    let executable = std::env::current_exe().unwrap();
    let env = BTreeMap::from([("TELEBAR_INERT_FORK".into(), "1".into())]);
    let out = telebar_runtime::process::run(
        executable.to_str().unwrap(),
        &["--exact", "fixture_leader", "--nocapture"],
        &env,
        &[],
        Duration::from_secs(3),
        4096,
    )
    .unwrap();
    assert!(out.ok());
    let text = String::from_utf8(out.stdout).unwrap();
    let pid: i32 = text
        .lines()
        .find_map(|line| line.strip_prefix("DESCENDANT="))
        .unwrap()
        .parse()
        .unwrap();
    let mut status = 0;
    // SAFETY: exact adopted fixture PID; group cleanup precedes leader reaping.
    assert_eq!(unsafe { libc::waitpid(pid, &mut status, 0) }, pid);
    assert!(libc::WIFSIGNALED(status));
    assert_eq!(libc::WTERMSIG(status), libc::SIGKILL);
}
