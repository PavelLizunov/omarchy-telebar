//! Native CLI/desktop/runtime-root characterization with disposable storage.
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    os::unix::{
        fs::{PermissionsExt, symlink},
        net::UnixListener,
    },
    time::Duration,
};
use telebar_runtime::{
    launcher::{Context, Target, language, parse, telegram_url},
    storage::Storage,
};
#[test]
fn native_launcher_keeps_identity_entries_and_existing_owner() {
    let root = std::env::temp_dir().join(format!("telebar-launcher-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let context = Context {
        home: root.clone(),
        runtime: root.clone(),
        plugin: root.join("plugin"),
        binary: root.join("plugin/bin/telebar-runtime"),
        environment: BTreeMap::new(),
    };
    fs::create_dir_all(context.plugin.join("bin")).unwrap();
    fs::create_dir(context.plugin.join("assets")).unwrap();
    fs::write(context.plugin.join("bin/omagram"), b"inert launcher").unwrap();
    fs::write(context.plugin.join("assets/omagram.svg"), b"<svg/>\n").unwrap();
    assert_eq!(parse(&[]).unwrap(), Target::Window);
    assert_eq!(
        parse(&["--chat".into(), "-00042".into()]).unwrap(),
        Target::Chat("-00042".into())
    );
    for url in [
        "tg://resolve?domain=sample",
        "https://t.me/sample",
        "http://telegram.me/sample",
        "https://t.me:443/sample",
    ] {
        assert!(telegram_url(url));
    }
    for url in [
        "tg://",
        "https://t.me.evil/sample",
        "https://user@t.me/sample",
        "https://t.me/sample\n",
        "file:///fixture",
        "https://www.telegram.me/sample",
    ] {
        assert!(!telegram_url(url));
    }
    assert_eq!(
        language(&BTreeMap::from([("LANG".into(), "ru_RU.UTF-8".into())])),
        "ru-RU"
    );
    assert_eq!(
        language(&BTreeMap::from([("LANG".into(), "C.UTF-8".into())])),
        "en"
    );
    let argv = context.managed_argv("telebar-backend", "--daemon", &[]);
    assert_eq!(
        &argv[argv.len() - 2..],
        [context.binary.to_str().unwrap(), "--daemon"]
    );
    assert!(argv.contains(&"--property=KillMode=control-group".into()));
    assert!(!argv.iter().any(|v| v == "/usr/bin/python3"));
    assert_eq!(context.install_desktop_entry().unwrap(), "written");
    let path = root.join(".local/share/applications/omagram.desktop");
    let entry = fs::read_to_string(&path).unwrap();
    assert!(entry.contains(&format!(
        "Exec={} %u",
        context.plugin.join("bin/omagram").display()
    )));
    assert!(!entry.contains("python"));
    fs::write(
        &path,
        entry
            .replace(
                "[Desktop Entry]\n",
                "[Desktop Entry]\nHidden=true\nNoDisplay=true\n",
            )
            .replace(
                "X-Omagram-Managed=io.github.pavellizunov.telebar",
                "X-Omagram-Managed=reidenxerx.omagram",
            ),
    )
    .unwrap();
    assert_eq!(context.install_desktop_entry().unwrap(), "written");
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("NoDisplay=true\nHidden=true\n")
    );
    assert_eq!(context.install_desktop_entry().unwrap(), "unchanged");
    fs::write(&path, b"[Desktop Entry]\nName=foreign\n").unwrap();
    assert_eq!(context.install_desktop_entry().unwrap(), "foreign");
    let shell = root.join("shell");
    fs::create_dir(&shell).unwrap();
    context.prepare_root_with(&shell).unwrap();
    context.prepare_root_with(&shell).unwrap();
    assert_eq!(
        fs::read_link(root.join("omagram/app/App")).unwrap(),
        context.plugin.join("app")
    );
    fs::remove_file(root.join("omagram/app/App")).unwrap();
    fs::write(root.join("omagram/app/App"), b"foreign").unwrap();
    assert!(context.prepare_root_with(&shell).is_err());
    let listener = UnixListener::bind(context.socket()).unwrap();
    assert!(context.start_service().unwrap()); // No systemd call when a socket owner exists.
    let server = std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut buf = Vec::new();
            let mut reader = std::io::BufReader::new(sock.try_clone().unwrap());
            let n = std::io::BufRead::read_until(&mut reader, b'\n', &mut buf).unwrap();
            if n > 0 {
                let value: serde_json::Value = serde_json::from_slice(&buf).unwrap();
                assert_eq!(value["args"]["chatId"], 42);
                sock.write_all(json!({"id":1,"ok":true,"result":{}}).to_string().as_bytes())
                    .unwrap();
            }
        }
    });
    context.request_open(&Target::Chat("42".into())).unwrap();
    server.join().unwrap();
    // Runtime parents are not traversed through caller-planted links.
    let elsewhere = root.join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    symlink(&elsewhere, root.join("linked")).unwrap();
    assert!(Storage::open(&root.join("linked")).is_err());
    assert!(
        telebar_runtime::launcher::desktop_entry(
            &root.join("plugin with spaces"),
            &BTreeSet::new()
        )
        .is_none()
    );
    fs::remove_dir_all(root).unwrap();
}
