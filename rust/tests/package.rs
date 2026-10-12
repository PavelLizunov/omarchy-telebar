//! Candidate assembly is local-only, explicit, bounded and refuses dirty paths.
use std::{fs, os::unix::fs::symlink};
use telebar_runtime::{package, storage::Storage};
#[test]
fn native_package_has_only_native_entry_points_and_preserves_source() {
    let root = std::env::temp_dir().join(format!("telebar-package-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let source = root.join("source");
    fs::create_dir(&source).unwrap();
    let store = Storage::open(&source).unwrap();
    for name in ["app", "shell", "assets", "LICENSES"] {
        store.ensure_dir(name).unwrap();
    }
    for (path, bytes) in [
        (
            "app/TelebarClient.qml",
            b"import QtQuick\nItem {}\n".as_slice(),
        ),
        (
            "shell/TelebarService.qml",
            b"import QtQuick\nItem {}\n".as_slice(),
        ),
        (
            "shell/BarWidget.qml",
            b"import QtQuick\nItem {}\n".as_slice(),
        ),
        (
            "shell/TelebarQuickReply.qml",
            b"import QtQuick\nItem {}\n".as_slice(),
        ),
        ("assets/omagram.svg", b"<svg/>\n".as_slice()),
        ("LICENSE", b"inert-license".as_slice()),
        ("THIRD_PARTY_NOTICES.md", b"inert-notices".as_slice()),
        ("LICENSES/Unicode-3.0.txt", b"inert-unicode".as_slice()),
        (
            "bin/vendor/flowseal/LICENSE",
            b"inert-flowseal-license".as_slice(),
        ),
        (
            "menu.jsonc",
            b"\"open\":{\"action\":\"@PLUGIN_BIN@/omagram\"},\n".as_slice(),
        ),
        (
            "manifest.json",
            br#"{"schemaVersion":1,"id":"io.github.pavellizunov.telebar","kinds":["service","bar-widget","overlay"],"entryPoints":{"service":"shell/TelebarService.qml","barWidget":"shell/BarWidget.qml","overlay":"shell/TelebarQuickReply.qml"}}"#.as_slice(),
        ),
    ] {
        store.write(path, bytes, 0o644).unwrap();
    }
    let binary = std::path::Path::new(env!("CARGO_BIN_EXE_telebar-runtime"));
    let output = root.join("candidate");
    package::assemble(&source, &output, binary).unwrap();
    for (name, mode) in [
        ("omagram", "--launch"),
        ("omagramd", "--daemon"),
        ("omagram-menu-install", "--menu"),
        ("omagram-build-tdlib", "--build-tdlib"),
        ("omagram-proxy", "--proxy"),
    ] {
        let raw = fs::read_to_string(output.join(format!("bin/{name}"))).unwrap();
        assert!(raw.starts_with("#!/usr/bin/bash -p\n"));
        assert!(raw.contains(mode));
        assert!(!raw.contains("python"));
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("runtime-candidate.json")).unwrap()).unwrap();
    assert_eq!(manifest["cutoverReady"], false);
    assert_eq!(manifest["runtime"], "rust");
    assert_eq!(
        fs::read(output.join("bin/telebar-runtime")).unwrap(),
        fs::read(binary).unwrap()
    );
    assert_eq!(
        fs::read(output.join("LICENSES/Flowseal-MIT.txt")).unwrap(),
        b"inert-flowseal-license"
    );
    assert!(!output.join("bin/vendor").exists());
    assert!(package::assemble(&source, &output, binary).is_err()); // No overwrite/update semantics.
    assert!(package::assemble(&source, &source.join("inside"), binary).is_err());
    symlink("/usr/bin", source.join("app/planted")).unwrap();
    let refused = root.join("linked");
    assert!(package::assemble(&source, &refused, binary).is_err());
    assert!(!refused.exists());
    fs::remove_file(source.join("app/planted")).unwrap();
    for (index, path) in ["app/helper.py", "assets/helper.pyc", "shell/hidden.py"]
        .iter()
        .enumerate()
    {
        store.write(path, b"inert", 0o644).unwrap();
        let refused = root.join(format!("python-file-{index}"));
        assert!(package::assemble(&source, &refused, binary).is_err());
        assert!(!refused.exists());
        store.remove_file(path).unwrap();
    }
    store
        .write("app/helper.js", b"launch(\"/usr/bin/PYTHON3\")", 0o644)
        .unwrap();
    let refused = root.join("js-python");
    assert!(package::assemble(&source, &refused, binary).is_err());
    assert!(!refused.exists());
    store.remove_file("app/helper.js").unwrap();
    store
        .write(
            "app/TelebarClient.qml",
            b"Item { function start() { launch(\"/usr/bin/python3\") } }",
            0o644,
        )
        .unwrap();
    let refused = root.join("python");
    assert!(package::assemble(&source, &refused, binary).is_err());
    assert!(!refused.exists());
    assert_eq!(fs::read(source.join("LICENSE")).unwrap(), b"inert-license");
    fs::remove_dir_all(root).unwrap();
}
