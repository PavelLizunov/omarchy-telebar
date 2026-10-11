//! Explicit parameter preparation, fake Secret Service and disposable storage.
use serde_json::json;
use std::{collections::BTreeMap, fs, os::unix::fs::PermissionsExt};
use telebar_runtime::{startup::Context, storage::Storage};
#[test]
fn preparation_preserves_identity_redacts_failures_and_never_opens_database() {
    let root = std::env::temp_dir().join(format!("telebar-startup-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let store = Storage::open(&root).unwrap();
    store
        .write(
            ".local/share/omagram/accounts/other/database/sentinel",
            b"keep",
            0o600,
        )
        .unwrap();
    let context = Context {
        home: root.clone(),
        environment: BTreeMap::new(),
        version: "fixture".into(),
        language: "ru".into(),
    };
    let key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB=";
    let hash = "0123456789abcdef0123456789abcdef";
    let mut requests = vec![];
    let prepared = context.prepare_with(
        "other",
        &mut |name| {
            requests.push(name.to_owned());
            Ok(Some(
                match name {
                    "api_id" => "123",
                    "api_hash" => hash,
                    "database_key_other" => key,
                    _ => panic!("unknown identity"),
                }
                .into(),
            ))
        },
        &mut |_, _, _| panic!("existing key must never be replaced"),
    );
    assert_eq!(requests, ["api_id", "api_hash", "database_key_other"]);
    assert!(!format!("{prepared:?}").contains(hash));
    let query = prepared.0.unwrap().unwrap();
    assert_eq!(query["database_encryption_key"], key);
    // Feed the actual prepared parameters through the native effect boundary,
    // without dispatching them or opening the disposable/working database.
    let mut runtime = telebar_runtime::authorization::Runtime::new("other".into());
    runtime
        .register(
            "other".into(),
            "Synthetic".into(),
            41,
            vec![],
            std::time::Instant::now(),
        )
        .unwrap();
    let sent = runtime.configure("other", Ok(Some(query.clone())), std::time::Instant::now());
    assert_eq!(sent.native.len(), 1);
    assert_eq!(sent.native[0].client, 41);
    assert_eq!(sent.native[0].query["database_encryption_key"], key);
    assert_eq!(sent.native[0].query["api_hash"], hash);
    assert!(runtime.parameters_pending("other"));
    let rejected = runtime.receive(json!({"@type":"error","@client_id":41,"@extra":sent.native[0].query["@extra"],"code":400,"message":hash}),std::time::Instant::now(),&mut |_|panic!("no re-preparation"),&mut |_|None);
    assert!(!runtime.parameters_pending("other"));
    assert!(rejected.native.is_empty());
    assert!(!format!("{:?}", rejected.replies).contains(hash));
    assert_eq!(
        query["database_directory"],
        json!(root.join(".local/share/omagram/accounts/other/database"))
    );
    assert_eq!(
        fs::read(root.join(".local/share/omagram/accounts/other/database/sentinel")).unwrap(),
        b"keep"
    );
    let absent = context.prepare_with("other", &mut |_| Ok(None), &mut |_, _, _| {
        panic!("missing API credentials cannot create a key")
    });
    assert!(absent.0.unwrap().is_none());
    let failed = context.prepare_with(
        "other",
        &mut |_| Err(std::io::Error::other(hash)),
        &mut |_, _, _| panic!("lookup failure cannot create a key"),
    );
    let error = failed.0.unwrap_err();
    assert!(!error.contains(hash));
    let missing = context.prepare_with(
        "other",
        &mut |name| {
            Ok(match name {
                "api_id" => Some("123".into()),
                "api_hash" => Some(hash.into()),
                _ => None,
            })
        },
        &mut |_, _, _| panic!("nonempty database cannot get a replacement key"),
    );
    assert_eq!(
        missing.0.unwrap_err(),
        "the database key is missing from the keyring"
    );
    fs::remove_dir_all(root).unwrap();
}
