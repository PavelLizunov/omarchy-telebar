//! Explicit session context never discovers credentials or working profiles.
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
use telebar_runtime::{application::Application, session::Context, storage::Storage};
#[test]
fn context_preserves_account_paths_and_limits_helper_environment() {
    let root = std::env::temp_dir().join(format!("telebar-session-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let storage = Storage::open(&root).unwrap();
    storage.write(".local/share/omagram/accounts.json", json!({"active":"other","accounts":[{"id":"other","name":"First","meId":7},{"id":"default","name":"Second"}]}).to_string().as_bytes(),0o600).unwrap();
    let environment = BTreeMap::from([
        ("LD_PRELOAD".into(), "untrusted.so".into()),
        ("PYTHONPATH".into(), "foreign".into()),
        ("WAYLAND_DISPLAY".into(), "fixture".into()),
        (
            "DBUS_SESSION_BUS_ADDRESS".into(),
            "unix:path=/fixture/bus".into(),
        ),
        ("HOME".into(), "/foreign".into()),
    ]);
    let context = Context::new(root.clone(), root.clone(), environment, "en".into()).unwrap();
    assert!(!context.environment.contains_key("LD_PRELOAD"));
    assert!(!context.environment.contains_key("PYTHONPATH"));
    assert_eq!(context.environment["HOME"], root.to_str().unwrap());
    assert_eq!(context.environment["WAYLAND_DISPLAY"], "fixture");
    let mut app = Application::load(&storage).unwrap();
    context.configure_application(&mut app).unwrap();
    assert_eq!(app.catalog["active"], "other");
    assert_eq!(app.catalog["accounts"][0]["id"], "other");
    let databases = &app.upload_context.as_ref().unwrap().1;
    assert_eq!(databases.len(), 2);
    assert_eq!(databases[0], root.join(".local/share/omagram/database"));
    assert_eq!(
        databases[1],
        root.join(".local/share/omagram/accounts/other/database")
    );
    assert_eq!(
        app.clipboard_context.as_ref().unwrap().databases,
        *databases
    );
    assert_eq!(app.recording_runtime.as_ref().unwrap(), &root);
    assert!(app.keyring_environment.is_none());
    assert!(!root.join(".local/share/omagram/database").exists());
    // Every ancestor is verified; a caller-provided symlink cannot redirect I/O.
    symlink(&root, root.join("foreign-runtime")).unwrap();
    assert!(
        Context::new(
            root.clone(),
            root.join("foreign-runtime"),
            BTreeMap::new(),
            "en".into()
        )
        .is_err()
    );
    fs::remove_dir_all(root).unwrap();
}
