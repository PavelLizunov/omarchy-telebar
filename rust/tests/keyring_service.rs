//! Actual secret-tool/Secret Service and official pre-phone startup on private
//! synthetic credentials. No real session bus, live HOME, user secrets or sends.
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    time::{Duration, Instant},
};
use telebar_runtime::{keyring::Keyring, process::Managed, storage::Storage};
#[test]
#[ignore = "requires explicit reviewed private Secret Service fixture on host-owned tools"]
fn actual_private_keyring_and_driver_startup_preserve_account_key_identity() {
    assert_eq!(std::env::var("TELEBAR_PRIVATE_KEYRING").as_deref(), Ok("1"));
    // The wrapper's seccomp address-family policy must actually deny network.
    // SAFETY: socket has no borrowed pointers; a success fd would be immediately
    // closed before refusing this fixture, never used for a connection.
    for family in [libc::AF_INET, libc::AF_INET6] {
        let fd = unsafe { libc::socket(family, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0) };
        if fd >= 0 {
            unsafe { libc::close(fd) };
            panic!("private fixture network denial missing");
        }
    }
    let library = std::env::var_os("TELEBAR_TEST_TDLIB").expect("explicit official TDLib");
    let root = std::path::PathBuf::from(
        std::env::var_os("TELEBAR_PRIVATE_ROOT").expect("explicit Landlock fixture root"),
    );
    assert!(root.is_absolute());
    let sentinel = std::env::var_os("TELEBAR_DENIED_FILE").expect("external non-secret sentinel");
    assert!(fs::File::open(&sentinel).is_err(), "host file exposure");
    let host_socket =
        std::env::var_os("TELEBAR_DENIED_SOCKET").expect("external inert Unix socket");
    assert!(
        std::os::unix::net::UnixStream::connect(host_socket).is_err(),
        "host Unix socket exposure"
    );
    assert!(
        fs::File::open("/home/slovn/.config/omarchy/shell.json").is_err(),
        "host home exposure"
    );
    assert!(
        std::os::unix::net::UnixStream::connect("/run/user/1000/bus").is_err(),
        "host session bus exposure"
    );
    for device in [
        "/dev/full",
        "/dev/input",
        "/dev/snd",
        "/dev/dri",
        "/dev/video0",
    ] {
        assert!(fs::File::open(device).is_err(), "physical device exposure");
    }
    assert_eq!(
        fs::metadata("/usr/bin/secret-tool").unwrap().uid(),
        0,
        "root helper identity must remain unchanged"
    );
    let root = root.join("fixture-home");
    fs::create_dir(&root).unwrap();
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let storage = Storage::open(&root).unwrap();
    storage.ensure_dir("runtime").unwrap();
    storage.ensure_dir("control").unwrap();
    let address = format!("unix:path={}/bus", root.display());
    // Explicit private bus policy; no service activation directories or host
    // address. Both daemons are owned finite process groups with RAII shutdown.
    let config = format!(
        "<busconfig><type>session</type><listen>{address}</listen><auth>EXTERNAL</auth><policy context=\"default\"><allow own=\"*\"/><allow send_destination=\"*\"/><allow receive_sender=\"*\"/></policy></busconfig>"
    );
    storage.write("bus.conf", config.as_bytes(), 0o600).unwrap();
    let environment = BTreeMap::from([
        ("HOME".into(), root.to_str().unwrap().into()),
        (
            "XDG_RUNTIME_DIR".into(),
            root.join("runtime").to_str().unwrap().into(),
        ),
        ("DBUS_SESSION_BUS_ADDRESS".into(), address.clone()),
    ]);
    let mut bus = Managed::start(
        "/usr/bin/dbus-daemon",
        &[
            "--nofork",
            "--nopidfile",
            "--config-file",
            root.join("bus.conf").to_str().unwrap(),
            "--print-address=1",
        ],
        &environment,
        &[],
        Duration::from_secs(45),
        16384,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !root.join("bus").exists() {
        assert!(Instant::now() < deadline);
        assert!(bus.poll().unwrap().is_none());
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut daemon = Managed::start(
        "/usr/bin/gnome-keyring-daemon",
        &[
            "--foreground",
            "--unlock",
            "--components=secrets",
            "--control-directory",
            root.join("control").to_str().unwrap(),
        ],
        &environment,
        b"synthetic-fixture-password\n",
        Duration::from_secs(40),
        16384,
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    let connection = zbus::blocking::connection::Builder::address(address.as_str())
        .unwrap()
        .method_timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let dbus = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .unwrap();
    loop {
        assert!(daemon.poll().unwrap().is_none(), "private keyring exited");
        let owner: bool = dbus
            .call("NameHasOwner", &("org.freedesktop.secrets",))
            .unwrap();
        if owner {
            break;
        }
        assert!(Instant::now() < until, "private Secret Service missing");
        std::thread::sleep(Duration::from_millis(2));
    }
    let keyring = Keyring::until(
        environment.clone(),
        Instant::now() + Duration::from_secs(12),
    );
    assert!(keyring.get("api_id").unwrap().is_none());
    assert!(
        keyring
            .save_credentials("123", "0123456789abcdef0123456789abcdef")
            .unwrap()
    );
    assert_eq!(keyring.credentials().unwrap().unwrap().0, 123);
    let default = keyring.database_key("default", &storage).unwrap();
    let other = keyring.database_key("other", &storage).unwrap();
    assert!(telebar_runtime::keyring::valid_key(&default));
    assert!(telebar_runtime::keyring::valid_key(&other));
    assert_ne!(default, other);
    assert_eq!(keyring.database_key("default", &storage).unwrap(), default);
    storage.write(".local/share/omagram/accounts.json",&serde_json::to_vec(&json!({"active":"other","accounts":[{"id":"other","name":"First"},{"id":"default","name":"Second"}]})).unwrap(),0o600).unwrap();
    let mut app = telebar_runtime::application::Application::load(&storage).unwrap();
    let context = telebar_runtime::session::Context::new(
        root.clone(),
        root.join("runtime"),
        environment,
        "en".into(),
    )
    .unwrap();
    context.configure_application(&mut app).unwrap();
    let mut driver = telebar_runtime::native_driver::Driver::open(
        Path::new(&library),
        "default".into(),
        "en".into(),
    )
    .unwrap();
    context.configure_driver(&mut app, &mut driver);
    context.register_catalog(&app, &mut driver).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while driver
        .runtime
        .metadata
        .values()
        .any(|m| m.auth["state"] != "phone")
        && Instant::now() < until
    {
        driver
            .tick_application(&mut app, &storage, &mut |_| {
                panic!("configured asynchronous startup must own private keyring")
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    for id in ["default", "other"] {
        assert_eq!(driver.runtime.metadata[id].auth["state"], "phone");
        assert!(!driver.runtime.engine.accounts[id].ready);
        assert!(driver.runtime.metadata[id].bridge_ready);
    }
    // Reopen the same disposable encrypted databases with existing keys. This
    // is sequential ownership, never two clients/processes on one database.
    driver.close(Duration::from_secs(5)).unwrap();
    drop(driver);
    let mut app = telebar_runtime::application::Application::load(&storage).unwrap();
    let mut reopened = telebar_runtime::native_driver::Driver::open(
        Path::new(&library),
        "default".into(),
        "en".into(),
    )
    .unwrap();
    context.configure_application(&mut app).unwrap();
    context.configure_driver(&mut app, &mut reopened);
    context.register_catalog(&app, &mut reopened).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while reopened
        .runtime
        .metadata
        .values()
        .any(|m| m.auth["state"] != "phone")
        && Instant::now() < until
    {
        reopened
            .tick_application(&mut app, &storage, &mut |_| panic!("private async owner"))
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    for id in ["default", "other"] {
        assert_eq!(reopened.runtime.metadata[id].auth["state"], "phone");
    }
    // Ordinary account.add uses the exact driver/catalog/checkpoint owners,
    // generates a new database key only in the private Secret Service, and
    // reaches the official pre-phone boundary without a Telegram mutation.
    let request = telebar_runtime::wire::Request::parse(
        br#"{"id":10,"cmd":"account.add","args":{"name":"Third private account"}}"#,
    )
    .unwrap();
    let added = reopened
        .request_application(&mut app, &storage, 7, request)
        .unwrap();
    let id =
        added.replies.iter().find(|(_, v)| v["id"] == 10).unwrap().1["result"]["activeAccount"]
            .as_str()
            .unwrap()
            .to_owned();
    assert!(
        reopened.runtime.engine.accounts[&id]
            .state
            .retained
            .has_checkpoint()
    );
    let until = Instant::now() + Duration::from_secs(10);
    while reopened.runtime.metadata[&id].auth["state"] != "phone" && Instant::now() < until {
        reopened
            .tick_application(&mut app, &storage, &mut |_| panic!("private async owner"))
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(reopened.runtime.metadata[&id].auth["state"], "phone");
    assert!(
        storage
            .nonempty_directory(&format!(".local/share/omagram/accounts/{id}/database"))
            .unwrap()
    );
    assert!(keyring.database_key(&id, &storage).is_ok());
    reopened.close(Duration::from_secs(5)).unwrap();
    assert_eq!(keyring.database_key("default", &storage).unwrap(), default);
    assert_eq!(keyring.database_key("other", &storage).unwrap(), other);
    let daemon_pid = daemon.pid();
    let bus_pid = bus.pid();
    drop(daemon);
    drop(bus);
    // Managed cancellation waited for the exact children; proc is deliberately
    // outside the Landlock policy, so process visibility is not a false proof.
    let _ = (daemon_pid, bus_pid);
    println!(
        "{}",
        json!({"actualSecretTool":true,"privateSecretService":true,"accountKeysPreserved":true,"asynchronousOfficialStartupAccounts":3,"sequentialDatabaseReopen":true,"accountAdd":true,"authorization":"phone","workingProfiles":false,"realUserSecrets":false,"helpersReaped":true,"landlockFileUnixDeviceDenial":true,"systemHelperRootIdentity":true})
    );
    fs::remove_dir_all(root).unwrap();
}
