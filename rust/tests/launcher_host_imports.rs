//! Explicit read-only host import admission; no window, manager job or backend.
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    time::Instant,
};
use telebar_runtime::launcher::Context;

#[test]
#[ignore = "explicit host import metadata only; no QML/window/startup"]
fn normal_launcher_admits_root_owned_imports_in_private_runtime_without_python() {
    assert_eq!(
        std::env::var("TELEBAR_HOST_IMPORTS_READONLY").as_deref(),
        Ok("1")
    );
    let began = Instant::now();
    let root = std::env::temp_dir().join(format!("telebar-host-imports-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let context = Context {
        home: root.clone(),
        runtime: root.clone(),
        plugin: root.join("inert-plugin"),
        binary: root.join("inert-plugin/bin/telebar-runtime"),
        environment: BTreeMap::new(),
    };
    // Calls the normal root-owned import checks, not prepare_root_with's seam.
    context.prepare_root().unwrap();
    let app = root.join("omagram/app");
    for (name, expected) in [
        ("App", context.plugin.join("app")),
        ("bin", context.plugin.join("bin")),
        (
            "Commons",
            std::path::PathBuf::from("/usr/share/omarchy/shell/Commons"),
        ),
        (
            "Ui",
            std::path::PathBuf::from("/usr/share/omarchy/shell/Ui"),
        ),
    ] {
        assert_eq!(fs::read_link(app.join(name)).unwrap(), expected);
    }
    let entry = fs::read_to_string(app.join("shell.qml")).unwrap();
    assert_eq!(
        entry,
        "import Quickshell\nimport \"App\" as App\n\nShellRoot {\n  App.Main {}\n}\n"
    );
    assert!(!entry.contains("python"));
    assert_eq!(
        fs::metadata(app.join("shell.qml")).unwrap().mode() & 0o777,
        0o600
    );
    context.prepare_root().unwrap();
    assert_eq!(fs::read_to_string(app.join("shell.qml")).unwrap(), entry);
    assert!(!context.socket().exists());
    assert!(!root.join(".local").exists());
    // remove_dir_all removes these generated links without following targets.
    fs::remove_dir_all(&root).unwrap();
    assert!(!root.exists());
    assert!(began.elapsed().as_secs() < 10);
}
