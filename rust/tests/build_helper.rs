//! Actual bounded logged helper, never a TDLib download/build or live install.
use std::{fs, os::unix::fs::PermissionsExt, time::Duration};
use telebar_runtime::{process::run_command_logged, storage::Storage};
#[test]
fn logged_build_retains_only_tail_and_checks_exit() {
    let root = std::env::temp_dir().join(format!("telebar-build-log-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let home = Storage::open(&root).unwrap();
    let command = || {
        use std::os::unix::process::CommandExt;
        let mut c = std::process::Command::new("/usr/bin/printf");
        c.arg("%0200d")
            .arg("1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .process_group(0);
        c
    };
    assert!(
        run_command_logged(
            command(),
            Duration::from_secs(2),
            &home,
            ".cache/omagram/build.log",
            64,
            &["/usr/bin/printf"]
        )
        .unwrap()
    );
    assert_eq!(
        home.read(".cache/omagram/build.log", 64).unwrap().unwrap(),
        format!("{:064}", 1).as_bytes()
    );
    let mut c = std::process::Command::new("/usr/bin/false");
    use std::os::unix::process::CommandExt;
    c.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .process_group(0);
    assert!(
        !run_command_logged(
            c,
            Duration::from_secs(2),
            &home,
            ".cache/omagram/build.log",
            64,
            &["/usr/bin/false"]
        )
        .unwrap()
    );
    assert!(telebar_runtime::tdlib_build::memory_jobs() > 0);
    fs::remove_dir_all(root).unwrap();
}
