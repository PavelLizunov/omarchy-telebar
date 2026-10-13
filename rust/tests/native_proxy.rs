//! Official TDLib proxy persistence/restore on unauthenticated disposable DBs.
//! Explicit no-network namespace only. No working profile, keyring or messages.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    time::{Duration, Instant},
};
use telebar_runtime::{
    application::Application, native_driver::Driver, session, storage::Storage, wire::Request,
};
fn startup(root: &Path, home: &Storage, library: &Path, binary: &Path) -> (Application, Driver) {
    let context =
        session::Context::new(root.into(), root.into(), BTreeMap::new(), "en".into()).unwrap();
    let mut app = Application::load(home).unwrap();
    app.bridge.context = Some(telebar_runtime::bridge::Context {
        binary: binary.into(),
        environment: Default::default(),
    });
    let mut driver = Driver::open(library, "other".into(), "en".into()).unwrap();
    context.register_catalog(&app, &mut driver).unwrap();
    let until = Instant::now() + Duration::from_secs(12);
    let mut attempts = BTreeMap::<String, usize>::new();
    while driver
        .runtime
        .metadata
        .values()
        .any(|m| m.auth["state"] != "phone")
    {
        assert!(Instant::now() < until, "official startup before phone");
        driver
            .tick(&mut |id| {
                *attempts.entry(id.into()).or_default() += 1;
                telebar_runtime::startup::Context {
                    home: root.into(),
                    environment: Default::default(),
                    version: "isolated-official-proxy".into(),
                    language: "en".into(),
                }
                .prepare_with(
                    id,
                    &mut |key| {
                        Ok(Some(match key {
                            "api_id" => "123".into(),
                            "api_hash" => "0123456789abcdef0123456789abcdef".into(),
                            _ if key.starts_with("database_key") => {
                                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into()
                            }
                            _ => panic!("unexpected key"),
                        }))
                    },
                    &mut |_, _, _| panic!("never replace injected database key"),
                )
                .0
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(attempts.len(), 2);
    assert!(attempts.values().all(|v| *v == 1));
    (app, driver)
}
fn request(
    driver: &mut Driver,
    app: &mut Application,
    home: &Storage,
    id: u64,
    account: &str,
    cmd: &str,
    args: Value,
) -> Value {
    let req = Request::parse(
        json!({"id":id,"account":account,"cmd":cmd,"args":args})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    let mut effects = driver.request_application(app, home, 7, req).unwrap();
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some((_, value)) = effects.replies.into_iter().find(|(_, v)| v["id"] == id) {
            assert_eq!(value["ok"], true, "official {cmd}: {value}");
            return value["result"].clone();
        }
        assert!(Instant::now() < until, "finite official {cmd}");
        effects = driver
            .tick_application(app, home, &mut |_| panic!("parameters already prepared"))
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
#[ignore = "requires official TDLib and exact Rust proxy under explicit no-network disposable namespace"]
fn official_proxy_restore_preserves_saved_and_previous_account_identity() {
    assert_eq!(
        std::env::var("TELEBAR_ISOLATED_NATIVE_PROXY").as_deref(),
        Ok("1")
    );
    let library = std::env::var_os("TELEBAR_TEST_TDLIB").unwrap();
    let binary = std::env::var_os("TELEBAR_TEST_PROXY_BINARY").unwrap();
    let root = Path::new("/tmp/native-proxy-home");
    fs::create_dir(root).unwrap();
    let home = Storage::open(root).unwrap();
    home.write(".local/share/omagram/accounts.json",&serde_json::to_vec(&json!({"active":"other","accounts":[{"id":"other","name":"First"},{"id":"default","name":"Second"}]})).unwrap(),0o600).unwrap();
    let (mut app, mut driver) = startup(root, &home, Path::new(&library), Path::new(&binary));
    let mut previous = BTreeMap::new();
    let mut saved = BTreeMap::new();
    let mut id = 1;
    for account in ["other", "default"] {
        let proxy = request(
            &mut driver,
            &mut app,
            &home,
            id,
            account,
            "proxy.add",
            json!({"type":"socks5","server":"127.0.0.1","port":9}),
        );
        id += 1;
        let prior = proxy["proxy"]["id"].as_i64().unwrap();
        assert!(prior > 0);
        previous.insert(account, prior);
        let enabled = request(
            &mut driver,
            &mut app,
            &home,
            id,
            account,
            "bridge.set",
            json!({"enabled":true}),
        );
        id += 1;
        assert_eq!(enabled["enabled"], true);
        assert_eq!(enabled["running"], true);
        assert_eq!(app.bridge.metadata[account]["previous_id"], prior);
        let owned = app.bridge.metadata[account]["proxy_id"].as_i64().unwrap();
        assert_ne!(owned, prior);
        saved.insert(account, owned);
    }
    let old_pid = app.bridge.running.as_ref().unwrap().helper.pid();
    app.bridge.running = None;
    assert!(!Path::new(&format!("/proc/{old_pid}")).exists());
    driver.close(Duration::from_secs(4)).unwrap();
    drop(driver);
    drop(app);
    // Reopen the same two DISPOSABLE encrypted DBs with exact injected keys.
    // Actual saved bridge queue uses official getProxies/editProxy, not the
    // synthetic owner-clock continuation tested separately.
    let (mut app, mut driver) = startup(root, &home, Path::new(&library), Path::new(&binary));
    driver.restore_saved_bridge(&app);
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        driver
            .tick_application(&mut app, &home, &mut |_| panic!("no startup replay"))
            .unwrap();
        if app.bridge.running.is_some() && app.bridge.busy.is_empty() {
            let mut settled = true;
            for account in ["other", "default"] {
                let rows = request(
                    &mut driver,
                    &mut app,
                    &home,
                    id,
                    account,
                    "proxies.list",
                    json!({}),
                );
                id += 1;
                let owned = rows["proxies"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|p| p["id"] == saved[account])
                    .unwrap();
                settled &= owned["enabled"] == true
                    && owned["port"] == app.bridge.running.as_ref().unwrap().port;
            }
            if settled {
                break;
            }
        }
        assert!(
            Instant::now() < until,
            "finite official saved bridge restore"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    let new_pid = app.bridge.running.as_ref().unwrap().helper.pid();
    assert_ne!(new_pid, old_pid);
    for account in ["other", "default"] {
        assert_eq!(
            app.bridge.metadata[account],
            json!({"proxy_id":saved[account],"previous_id":previous[account]})
        );
        assert_eq!(driver.runtime.metadata[account].auth["state"], "phone");
        assert!(!driver.runtime.engine.accounts[account].ready);
        request(
            &mut driver,
            &mut app,
            &home,
            id,
            account,
            "bridge.set",
            json!({"enabled":false}),
        );
        id += 1;
        let rows = request(
            &mut driver,
            &mut app,
            &home,
            id,
            account,
            "proxies.list",
            json!({}),
        );
        id += 1;
        assert!(
            rows["proxies"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["id"] == previous[account] && p["enabled"] == true)
        );
    }
    assert!(app.bridge.running.is_none());
    assert!(app.bridge.metadata.as_object().unwrap().is_empty());
    assert!(!Path::new(&format!("/proc/{new_pid}")).exists());
    assert_eq!(driver.runtime.engine.active, "other");
    driver.close(Duration::from_secs(4)).unwrap();
    drop(driver);
    println!(
        "{}",
        json!({"officialProxyPersistence":true,"disposableAccounts":2,"saveRestorePriorIdentity":true,"nativeEditExistingProxy":true,"proxyChildrenReaped":true,"authorization":"phone","workingProfiles":false,"network":false,"messagesSent":0})
    );
    fs::remove_dir_all(root).unwrap();
}
