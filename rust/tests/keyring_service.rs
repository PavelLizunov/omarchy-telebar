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
        .any(|m| m.auth["state"] != "needCredentials")
    {
        assert!(
            Instant::now() < until,
            "private missing-credential boundary"
        );
        driver
            .tick_application(&mut app, &storage, &mut |_| panic!("private async owner"))
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    for (rid, account) in [(20, "other"), (21, "default")] {
        let request = telebar_runtime::wire::Request::parse(
            json!({"id":rid,"cmd":"credentials.set","account":account,"args":{"apiId":123,"apiHash":"0123456789abcdef0123456789abcdef"}}).to_string().as_bytes(),
        ).unwrap();
        let mut effects = driver
            .request_application(&mut app, &storage, 7, request)
            .unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        let mut acknowledged = false;
        let mut starting = false;
        loop {
            for (_, frame) in effects.replies {
                if frame["id"] == rid {
                    assert_eq!(frame["ok"], true, "private credentials request");
                    acknowledged = true;
                }
                if frame["event"] == "auth"
                    && frame["account"] == account
                    && frame["auth"]["state"] == "starting"
                {
                    assert!(acknowledged, "save reply must precede restart event");
                    starting = true;
                }
            }
            if acknowledged && driver.runtime.metadata[account].auth["state"] == "phone" {
                break;
            }
            assert!(
                Instant::now() < until,
                "private credentials route/startup settlement"
            );
            effects = driver
                .tick_application(&mut app, &storage, &mut |_| panic!("private async owner"))
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(starting);
        if account == "other" {
            assert_eq!(
                driver.runtime.metadata["default"].auth["state"], "needCredentials",
                "only routed account restarts"
            );
        }
    }
    assert_eq!(
        keyring.credentials().unwrap().unwrap(),
        (123, "0123456789abcdef0123456789abcdef".into())
    );
    let bad = telebar_runtime::wire::Request::parse(br#"{"id":22,"cmd":"credentials.set","args":{"apiId":false,"apiHash":"0123456789abcdef0123456789abcdef"}}"#).unwrap();
    let refused = driver
        .request_application(&mut app, &storage, 7, bad)
        .unwrap();
    assert!(
        refused
            .replies
            .iter()
            .any(|(_, frame)| frame["id"] == 22 && frame["ok"] == false)
    );
    assert_eq!(keyring.credentials().unwrap().unwrap().0, 123);
    for id in ["default", "other"] {
        assert_eq!(driver.runtime.metadata[id].auth["state"], "phone");
        assert!(!driver.runtime.engine.accounts[id].ready);
        assert!(driver.runtime.metadata[id].bridge_ready);
    }
    // Bidirectional legacy/native transition on these unauthorized disposable
    // profiles. The same lock excludes Python before it reads keys or loads TDLib.
    let lock_identity = |path: &Path| {
        let meta = fs::metadata(path).unwrap();
        (meta.dev(), meta.ino())
    };
    let owner = telebar_runtime::socket_owner::Owner::acquire(
        &root.join("runtime"),
        "handoff.sock",
        "omagram.lock",
    )
    .unwrap();
    let original_lock = lock_identity(&root.join("runtime/omagram.lock"));
    let mut legacy_environment = context.environment.clone();
    for name in ["TELEBAR_PRIVATE_ROOT", "TELEBAR_TEST_TDLIB"] {
        legacy_environment.insert(name.into(), std::env::var(name).unwrap());
    }
    legacy_environment.insert("TELEBAR_PRIVATE_KEYRING".into(), "1".into());
    legacy_environment.insert("PYTHONDONTWRITEBYTECODE".into(), "1".into());
    let oracle = std::path::PathBuf::from(std::env::var_os("TELEBAR_PRIVATE_ROOT").unwrap())
        .join("legacy-oracle/handoff.py");
    let blocked = telebar_runtime::process::run(
        "/usr/bin/python3",
        &["-I", oracle.to_str().unwrap(), "--lock-refusal"],
        &legacy_environment,
        &[],
        Duration::from_secs(4),
        4096,
    )
    .unwrap();
    assert_eq!(blocked.code, Some(73));
    assert_eq!(blocked.stdout, b"blocked\n");
    assert!(!blocked.timed_out && !blocked.truncated);
    // Official clients close and the receiver wrapper drops before releasing
    // the ownership lock. No file/database copy or simultaneous native owner.
    driver.close(Duration::from_secs(5)).unwrap();
    drop(driver);
    drop(owner);
    let legacy = telebar_runtime::process::run(
        "/usr/bin/python3",
        &["-I", oracle.to_str().unwrap(), "--handoff"],
        &legacy_environment,
        &[],
        Duration::from_secs(15),
        16384,
    )
    .unwrap();
    if !legacy.ok() {
        let diagnostic = std::str::from_utf8(&legacy.stderr)
            .ok()
            .and_then(|text| text.lines().last())
            .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .unwrap_or_default();
        let stage = diagnostic["handoffFailureStage"]
            .as_str()
            .unwrap_or("unknown");
        let kind = diagnostic["exceptionType"].as_str().unwrap_or("unknown");
        panic!(
            "legacy private handoff failed at {stage} ({kind}); exit {:?}; timeout {}",
            legacy.code, legacy.timed_out
        );
    }
    let report: serde_json::Value = serde_json::from_slice(&legacy.stdout).unwrap();
    assert_eq!(report["legacyOfficialAccounts"], 2);
    assert_eq!(report["closed"], 2);
    assert_eq!(report["keysPreserved"], true);
    assert_eq!(report["catalogPreserved"], true);
    let reopened_owner = telebar_runtime::socket_owner::Owner::acquire(
        &root.join("runtime"),
        "handoff.sock",
        "omagram.lock",
    )
    .unwrap();
    assert_eq!(
        lock_identity(&root.join("runtime/omagram.lock")),
        original_lock
    );
    assert_eq!(keyring.database_key("default", &storage).unwrap(), default);
    assert_eq!(keyring.database_key("other", &storage).unwrap(), other);
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
    let added_key = keyring.database_key(&id, &storage).unwrap();
    let old_client = reopened.runtime.engine.accounts[&id].client;
    let old_generation = reopened.runtime.engine.accounts[&id].generation;
    let catalog = app.catalog.clone();
    let untouched =
        ["default", "other"].map(|account| reopened.runtime.engine.accounts[account].client);
    let request = telebar_runtime::wire::Request::parse(
        json!({"id":30,"cmd":"account.logout","args":{"accountId":id}})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    let mut effects = reopened
        .request_application(&mut app, &storage, 7, request)
        .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    let mut acknowledged = false;
    loop {
        for (_, frame) in effects.replies {
            if frame["id"] == 30 {
                assert_eq!(
                    frame["ok"], true,
                    "official disposable logout reply {frame}"
                );
                acknowledged = true;
            }
        }
        let account = &reopened.runtime.engine.accounts[&id];
        if acknowledged
            && account.client != old_client
            && account.generation == old_generation + 1
            && reopened.runtime.metadata[&id].auth["state"] == "phone"
        {
            break;
        }
        assert!(
            Instant::now() < until,
            "official disposable closed client/reopen"
        );
        effects = reopened
            .tick_application(&mut app, &storage, &mut |_| panic!("private async owner"))
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(keyring.database_key(&id, &storage).unwrap(), added_key);
    assert_eq!(app.catalog["active"], catalog["active"]);
    assert_eq!(
        app.catalog["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].clone())
            .collect::<Vec<_>>(),
        catalog["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        ["default", "other"].map(|account| reopened.runtime.engine.accounts[account].client),
        untouched
    );
    reopened.close(Duration::from_secs(5)).unwrap();
    drop(reopened);
    drop(reopened_owner);
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
        json!({"actualSecretTool":true,"privateSecretService":true,"accountKeysPreserved":true,"asynchronousOfficialStartupAccounts":3,"sequentialDatabaseReopen":true,"accountAdd":true,"credentialsSetDriver":true,"replyBeforeRoutedStartup":true,"invalidSavePreservesCredentials":true,"disposableLogoutAndRecreate":true,"logoutKeyCatalogIdentityPreserved":true,"authorization":"phone","workingProfiles":false,"realUserSecrets":false,"helpersReaped":true,"landlockFileUnixDeviceDenial":true,"systemHelperRootIdentity":true,"legacyPythonLockRefusalBeforeStartup":true,"officialRustPythonRustProfileHandoff":true,"sameLockInode":true})
    );
    fs::remove_dir_all(root).unwrap();
}
