//! Official TDLib parameter/database startup on disposable profiles with no
//! network, live home, credentials or devices. Explicit wrapper opt-in only.
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    time::{Duration, Instant},
};
use telebar_runtime::{application::Application, native_driver::Driver, session, storage::Storage};
#[test]
#[ignore = "requires official TDLib and the reviewed no-network disposable namespace"]
fn official_parameters_reach_authorization_with_distinct_disposable_databases() {
    assert_eq!(
        std::env::var("TELEBAR_ISOLATED_PARAMETERS").as_deref(),
        Ok("1")
    );
    let library = std::env::var_os("TELEBAR_TEST_TDLIB").expect("explicit official library");
    let root = Path::new("/tmp/native-parameters-home");
    fs::create_dir(root).unwrap();
    let storage = Storage::open(root).unwrap();
    storage.write(".local/share/omagram/accounts.json",&serde_json::to_vec(&json!({"active":"other","accounts":[{"id":"other","name":"First"},{"id":"default","name":"Second"}]})).unwrap(),0o600).unwrap();
    let context =
        session::Context::new(root.into(), root.into(), BTreeMap::new(), "en".into()).unwrap();
    let app = Application::load(&storage).unwrap();
    let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
    context.register_catalog(&app, &mut driver).unwrap();
    let mut prepared = std::collections::HashMap::new();
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline
        && driver
            .runtime
            .metadata
            .values()
            .any(|m| m.auth["state"] != "phone")
    {
        let prepared = &mut prepared;
        driver
            .tick(&mut |id| {
                *prepared.entry(id.to_owned()).or_insert(0usize) += 1;
                telebar_runtime::startup::Context {
                    home: root.into(),
                    environment: BTreeMap::new(),
                    version: "isolated-native-startup".into(),
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
                    &mut |_, _, _| panic!("existing synthetic key must not be replaced"),
                )
                .0
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    for id in ["default", "other"] {
        assert_eq!(
            driver.runtime.metadata[id].auth["state"], "phone",
            "official parameter startup: {id}"
        );
        assert_eq!(prepared[id], 1);
        assert!(!driver.runtime.engine.accounts[id].ready);
        assert!(driver.runtime.metadata[id].bridge_ready);
    }
    assert_eq!(driver.runtime.engine.active, "other");
    let roots: Vec<_> = driver
        .runtime
        .engine
        .accounts
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(roots, ["other", "default"]);
    driver.close(Duration::from_secs(5)).unwrap();
    let default = root.join(".local/share/omagram/database");
    let other = root.join(".local/share/omagram/accounts/other/database");
    assert!(default.is_dir() && other.is_dir() && default != other);
    assert!(fs::read_dir(&default).unwrap().next().is_some());
    assert!(fs::read_dir(&other).unwrap().next().is_some());
    println!(
        "{}",
        json!({"officialTdlibParameters":true,"accounts":2,"parameterAttempts":prepared,"disposableDatabases":true,"authorization":"phone","closed":true,"network":false,"workingProfiles":false,"realKeyring":false})
    );
    fs::remove_dir_all(root).unwrap();
}
