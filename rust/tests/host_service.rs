//! Exact native user-manager request, isolated fake consumer or explicit live
//! disposable unit. The live case never starts daemon/window or opens profiles.
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};
use telebar_runtime::host_service;
#[test]
fn transient_properties_preserve_identity_and_refuse_ambient_programs() {
    let properties = host_service::properties(
        std::path::Path::new("/fixture/bin/telebar-runtime"),
        "--daemon",
        &[],
        None,
    )
    .unwrap();
    let values: HashMap<_, _> = properties.iter().map(|(k, v)| (k.as_str(), v)).collect();
    assert_eq!(<&str>::try_from(values["Type"]).unwrap(), "exec");
    assert_eq!(
        <&str>::try_from(values["KillMode"]).unwrap(),
        "control-group"
    );
    assert_eq!(u64::try_from(values["TimeoutStopUSec"]).unwrap(), 8_000_000);
    let unset: Vec<String> = values["UnsetEnvironment"]
        .try_clone()
        .unwrap()
        .try_into()
        .unwrap();
    assert!(unset.contains(&"LD_PRELOAD".into()));
    assert!(unset.contains(&"BASH_ENV".into()));
    assert!(!values.contains_key("RuntimeMaxUSec"));
    assert!(
        host_service::properties(std::path::Path::new("relative"), "--daemon", &[], None).is_err()
    );
    assert!(
        host_service::properties(
            std::path::Path::new("/fixture/bin/runtime"),
            "--project",
            &[],
            None
        )
        .is_err()
    );
    assert!(
        host_service::properties(
            std::path::Path::new("/fixture/bin/runtime"),
            "--inert-no-library",
            &[],
            Some(Duration::from_secs(121))
        )
        .is_err()
    );
}
#[test]
#[ignore = "requires an explicit disposable unit on the existing user manager"]
fn actual_manager_starts_only_a_finite_no_native_owner() {
    assert_eq!(
        std::env::var("TELEBAR_ISOLATED_HOST_HANDOFF").as_deref(),
        Ok("1")
    );
    let root = PathBuf::from(std::env::var_os("TELEBAR_HANDOFF_ROOT").unwrap());
    assert!(root.is_absolute());
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::symlink_metadata(&root).unwrap();
    assert!(
        metadata.is_dir()
            && metadata.uid() == unsafe { libc::getuid() }
            && metadata.mode() & 0o022 == 0
    );
    // Namespace fixture preparation checks only its owned leaf. The delegated
    // host owner validates the full storage ancestry before opening any input.
    let home = root.join(format!("host-inert-{}", std::process::id()));
    fs::create_dir(&home).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&home, fs::Permissions::from_mode(0o700)).unwrap();
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap();
    let unit = format!("telebar-rust-inert-handoff-{}.service", std::process::id());
    let binary = std::env::var_os("TELEBAR_HANDOFF_BINARY").unwrap();
    assert!(
        host_service::start(
            &address,
            &unit,
            std::path::Path::new(&binary),
            "--inert-no-library",
            &[home.to_str().unwrap().into()],
            Some(Duration::from_secs(15))
        )
        .unwrap()
    );
    let connection = zbus::blocking::connection::Builder::address(address.as_str())
        .unwrap()
        .method_timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let manager = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    )
    .unwrap();
    let path: zbus::zvariant::OwnedObjectPath = manager.call("GetUnit", &(&unit,)).unwrap();
    let service = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.systemd1",
        path.as_str(),
        "org.freedesktop.systemd1.Service",
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let pid = loop {
        let pid: u32 = service.get_property("MainPID").unwrap();
        if pid != 0 && home.join("migration.sock").exists() {
            break pid;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    };
    if std::env::var("TELEBAR_HANDOFF_NAMESPACED").as_deref() == Ok("1") {
        // A user namespace cannot ptrace/read host proc namespaces. The host
        // wrapper inspects this exact PID while the finite fixture is alive.
        let evidence = root.join("namespace-handoff.json");
        fs::write(
            &evidence,
            serde_json::json!({"pid":pid,"unit":unit,"home":home}).to_string(),
        )
        .unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while !root.join("namespace-handoff-verified").exists() {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(10));
        }
    } else {
        assert_eq!(
            fs::read_link(format!("/proc/{pid}/ns/user")).unwrap(),
            std::path::Path::new(&std::env::var("TELEBAR_HANDOFF_HOST_NAMESPACE").unwrap())
        );
        assert_eq!(
            fs::read_link(format!("/proc/{pid}/exe")).unwrap(),
            std::path::Path::new(&binary)
        );
        let environ = fs::read(format!("/proc/{pid}/environ")).unwrap();
        assert!(
            !environ
                .split(|b| *b == 0)
                .any(|v| v.starts_with(b"LD_PRELOAD=") || v.starts_with(b"PYTHONPATH="))
        );
    }
    assert!(
        !host_service::start(
            &address,
            &unit,
            std::path::Path::new(&binary),
            "--inert-no-library",
            &[home.to_str().unwrap().into()],
            Some(Duration::from_secs(15))
        )
        .unwrap()
    );
    assert_eq!(service.get_property::<u32>("MainPID").unwrap(), pid);
    let _: zbus::zvariant::OwnedObjectPath = manager.call("StopUnit", &(&unit, "replace")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while home.join("migration.sock").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    fs::remove_dir_all(home).unwrap();
}
#[test]
fn values_serialize_as_the_real_manager_method_contract() {
    let values = host_service::properties(
        std::path::Path::new("/fixture/bin/runtime"),
        "--host-window",
        &["--chat".into(), "-42".into()],
        None,
    )
    .unwrap();
    let start = values
        .into_iter()
        .find(|(k, _)| k == "ExecStart")
        .unwrap()
        .1;
    let entries: Vec<(String, Vec<String>, bool)> = start.try_into().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].1,
        vec!["/fixture/bin/runtime", "--host-window", "--chat", "-42"]
    );
    assert!(!entries[0].2);
    let desktop = host_service::properties(
        std::path::Path::new("/fixture/bin/runtime"),
        "--host-window",
        &["--desktop-entry".into()],
        None,
    )
    .unwrap();
    assert!(desktop.iter().any(|(key, _)| key == "ExecStart"));
    assert!(
        host_service::properties(
            std::path::Path::new("/fixture/bin/runtime"),
            "--host-window",
            &["--project".into()],
            None
        )
        .is_err()
    );
}
