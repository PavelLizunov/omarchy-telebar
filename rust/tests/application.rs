//! Actual local application owners in a disposable descriptor-anchored home.
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    time::Instant,
};
use telebar_runtime::{
    application::Application, authorization::Runtime, storage::Storage, wire::Request,
};
#[test]
fn hello_switch_and_settings_observe_persisted_state() {
    let root = std::env::temp_dir().join(format!("telebar-application-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let home = Storage::open(&root).unwrap();
    home.write(
        ".local/share/omagram/accounts.json",
        json!({"active":"default","accounts":[{"id":"default"},{"id":"other"}]})
            .to_string()
            .as_bytes(),
        0o600,
    )
    .unwrap();
    let mut app = Application::load(&home).unwrap();
    let mut runtime = Runtime::new("default".into());
    for (id, client) in [("default", 1), ("other", 2)] {
        runtime
            .register(id.into(), id.into(), client, vec![], Instant::now())
            .unwrap();
    }
    let request = |id, command, args| {
        Request::parse(
            json!({"id":id,"cmd":command,"args":args})
                .to_string()
                .as_bytes(),
        )
        .unwrap()
    };
    let hello = app.request(
        &home,
        &mut runtime,
        7,
        request(1, "hello", json!({"window":true})),
    );
    assert_eq!(hello.replies[0].1["ok"], true);
    assert!(hello.replies[0].1["result"]["allChats"].is_array());
    assert_eq!(
        hello.replies[0].1["result"]["accounts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let switched = app.request(
        &home,
        &mut runtime,
        7,
        request(2, "account.switch", json!({"id":"other"})),
    );
    assert_eq!(runtime.engine.active, "other");
    assert_eq!(switched.replies[0].1["event"], "accountSwitched");
    assert_eq!(switched.replies[1].1["result"]["accountRevision"], 1);
    assert!(switched.replies[1].1["result"]["allChats"].is_array());
    assert_eq!(Application::load(&home).unwrap().catalog["active"], "other");
    let changed = app.request(
        &home,
        &mut runtime,
        7,
        request(3, "settings.chatListWidth", json!({"width":230})),
    );
    assert_eq!(changed.replies[0].1["ok"], true);
    assert_eq!(
        Application::load(&home).unwrap().settings.value["chatListWidth"],
        230
    );
    // Explicit target logout retains envelope validation and the target's
    // original request generation; no real native dispatch is performed.
    let logout = app.request(
        &home,
        &mut runtime,
        7,
        request(8, "account.logout", json!({"id":"default"})),
    );
    assert_eq!(logout.native[0].client, 1);
    assert_eq!(logout.native[0].query["@type"], "logOut");
    let duplicate = app.request(
        &home,
        &mut runtime,
        7,
        request(8, "account.logout", json!({"id":"missing"})),
    );
    assert_eq!(duplicate.disconnect, Some(7));
    assert!(duplicate.native.is_empty());
    fs::remove_file(root.join(".local/share/omagram/accounts.json")).unwrap();
    symlink("foreign", root.join(".local/share/omagram/accounts.json")).unwrap();
    let rejected = app.request(
        &home,
        &mut runtime,
        7,
        request(4, "account.switch", json!({"id":"default"})),
    );
    assert_eq!(rejected.replies[0].1["ok"], false);
    assert_eq!(runtime.engine.active, "other");
    assert_eq!(app.revision, 1);
    let before = runtime.engine.accounts.len();
    let failed = app.add(
        &home,
        &mut runtime,
        9,
        request(9, "account.add", json!({"name":"Synthetic"})),
        &mut || Ok(3),
    );
    assert_eq!(runtime.engine.accounts.len(), before);
    assert_eq!(failed.replies[0].1["ok"], false);
    assert_eq!(failed.native[0].query["@type"], "close");
    fs::remove_file(root.join(".local/share/omagram/accounts.json")).unwrap();
    let added = app.add(
        &home,
        &mut runtime,
        9,
        request(10, "account.add", json!({"name":" Synthetic "})),
        &mut || Ok(4),
    );
    let id = added.replies[1].1["result"]["activeAccount"]
        .as_str()
        .unwrap();
    assert_eq!(runtime.engine.accounts[id].client, 4);
    assert_eq!(runtime.metadata[id].name, "Synthetic");
    assert_eq!(Application::load(&home).unwrap().catalog["active"], id);
    assert_eq!(added.replies[0].1["event"], "accountAdded");
    assert_eq!(added.native[0].query["@type"], "getOption");
    app.retire_peer(7);
    assert!(!app.windows.contains(&7));
    fs::remove_dir_all(root).unwrap();
}
