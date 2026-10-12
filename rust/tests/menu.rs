//! Managed block matches legacy semantics and preserves unrelated user text.
use std::{fs, os::unix::fs::PermissionsExt};
use telebar_runtime::{menu, storage::Storage};
#[test]
fn menu_replaces_owned_block_idempotently_and_preserves_foreign_text() {
    let root = std::env::temp_dir().join(format!("telebar-menu-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let home = Storage::open(&root).unwrap();
    let plugin = root.join("plugin");
    fs::create_dir_all(plugin.join("bin")).unwrap();
    fs::write(plugin.join("bin/omagram"), b"inert").unwrap();
    fs::write(
        plugin.join("menu.jsonc"),
        b"  \"trigger.omagram\": { \"action\": \"@PLUGIN_BIN@/omagram\" },",
    )
    .unwrap();
    let original = "{\n  // user's note\n  \"user\": {\"keep\": true},\n}\n";
    home.write(
        ".config/omarchy/extensions/omarchy-menu.jsonc",
        original.as_bytes(),
        0o644,
    )
    .unwrap();
    menu::apply(&home, &plugin, "install").unwrap();
    let first = home
        .read(".config/omarchy/extensions/omarchy-menu.jsonc", 65536)
        .unwrap()
        .unwrap();
    assert!(menu::valid(std::str::from_utf8(&first).unwrap()));
    menu::apply(&home, &plugin, "install").unwrap();
    assert_eq!(
        home.read(".config/omarchy/extensions/omarchy-menu.jsonc", 65536)
            .unwrap()
            .unwrap(),
        first
    );
    menu::apply(&home, &plugin, "remove").unwrap();
    assert_eq!(
        home.read(".config/omarchy/extensions/omarchy-menu.jsonc", 65536)
            .unwrap()
            .unwrap(),
        original.as_bytes()
    );
    home.write(
        ".config/omarchy/extensions/omarchy-menu.jsonc",
        b"broken",
        0o644,
    )
    .unwrap();
    assert!(menu::apply(&home, &plugin, "install").is_err());
    assert_eq!(
        home.read(".config/omarchy/extensions/omarchy-menu.jsonc", 65536)
            .unwrap()
            .unwrap(),
        b"broken"
    );
    fs::remove_dir_all(root).unwrap();
}
