//! Explicit host Secret Service read-only acceptance; never native startup.
//! No value/hash/log from credentials or database keys is emitted or stored.
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use telebar_runtime::{
    keyring::{self, Keyring},
    launcher, paths,
};

#[test]
#[ignore = "requires explicit host read-only capability and current account IDs; no TDLib/profile open"]
fn existing_host_keyring_identity_is_readable_without_creating_keys_or_clients() {
    assert_eq!(
        std::env::var("TELEBAR_HOST_KEYRING_READONLY").as_deref(),
        Ok("1")
    );
    let ids: Vec<String> = serde_json::from_str(
        &std::env::var("TELEBAR_HOST_ACCOUNT_IDS").expect("explicit account IDs only"),
    )
    .expect("account IDs JSON");
    assert!((1..=10).contains(&ids.len()) && ids.iter().all(|id| paths::valid_account_id(id)));
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
    let environment = launcher::session_environment();
    assert!(
        environment
            .get("DBUS_SESSION_BUS_ADDRESS")
            .is_some_and(|v| v.starts_with("unix:path=/"))
    );
    let store = Keyring::until(environment, Instant::now() + Duration::from_secs(10));
    let api_id = store
        .get("api_id")
        .expect("read-only API id lookup refused")
        .expect("API id absent");
    let api_hash = store
        .get("api_hash")
        .expect("read-only API hash lookup refused")
        .expect("API hash absent");
    assert!(
        keyring::valid_credentials(&api_id, &api_hash),
        "existing API credential representation invalid"
    );
    for id in &ids {
        let name = if id == "default" {
            "database_key".into()
        } else {
            format!("database_key_{id}")
        };
        let value = store
            .get(&name)
            .expect("read-only database-key lookup refused")
            .expect("existing key absent");
        assert!(
            keyring::valid_key(&value),
            "existing database key representation invalid"
        );
    }
    println!(
        "actual host native Keyring lookup: valid API metadata and {} existing account keys; writes0/nativeclients0/parameters0; values not emitted",
        ids.len()
    );
}
