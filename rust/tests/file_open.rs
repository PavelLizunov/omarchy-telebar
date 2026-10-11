//! Actual native continuation and xdg-open against one inert desktop handler.
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    time::{Duration, Instant},
};
use telebar_runtime::{
    commands::{Action, Answer},
    file_open::Selection,
    state::State,
};
#[test]
fn opener_validates_native_identity_and_pins_the_selected_inode() {
    let root = std::env::temp_dir().join(format!("telebar-opener-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let apps = root.join("share/applications");
    fs::create_dir_all(&apps).unwrap();
    fs::write(apps.join("inert.desktop"),"[Desktop Entry]\nType=Application\nName=Inert read\nExec=/usr/bin/cat %f\nMimeType=text/plain;\n").unwrap();
    let config = root.join("config");
    fs::create_dir(&config).unwrap();
    fs::write(
        config.join("mimeapps.list"),
        "[Default Applications]\ntext/plain=inert.desktop\n",
    )
    .unwrap();
    let env = BTreeMap::from([
        ("HOME".into(), root.to_string_lossy().into_owned()),
        ("PATH".into(), "/usr/bin".into()),
        (
            "XDG_CONFIG_HOME".into(),
            config.to_string_lossy().into_owned(),
        ),
        (
            "XDG_DATA_HOME".into(),
            root.join("share").to_string_lossy().into_owned(),
        ),
        (
            "XDG_DATA_DIRS".into(),
            root.join("share").to_string_lossy().into_owned(),
        ),
        ("XDG_CURRENT_DESKTOP".into(), "generic".into()),
        ("DE".into(), "generic".into()),
        // Dummy address is never connected: xdg-open only uses it to select
        // the explicit inert MIME handler installed inside this test home.
        ("WAYLAND_DISPLAY".into(), "telebar-test-no-socket".into()),
    ]);
    let Action::Native {
        answer: Answer::OpenFile(selection),
        query,
    } = Selection::prepare(&json!({"fileId":3}), env).unwrap()
    else {
        panic!("getFile")
    };
    assert_eq!(query["@type"], "getFile");
    let mut state = State::new(vec![root.to_string_lossy().into_owned()]);
    state.me_id = 7;
    assert!(selection.complete(&json!({"id":4}), &state).is_err());
    assert!(
        selection
            .complete(
                &json!({"id":3,"local":{"is_downloading_completed":false}}),
                &state
            )
            .is_err()
    );
    let path = root.join("chosen.txt");
    fs::write(&path, b"original selected bytes\n").unwrap();
    let event = json!({"id":3,"local":{"is_downloading_completed":true,"path":path}});
    let Action::OpenFile(start) = selection.complete(&event, &state).unwrap() else {
        panic!("open")
    };
    fs::rename(&path, root.join("original.txt")).unwrap();
    symlink(root.join("original.txt"), &path).unwrap();
    assert!(start.spawn().is_err());
    fs::remove_file(&path).unwrap();
    fs::rename(root.join("original.txt"), &path).unwrap();
    let (mut helper, _descriptor) = start.spawn().unwrap();
    let pid = helper.pid();
    fs::rename(&path, root.join("original.txt")).unwrap();
    fs::write(&path, b"replacement bytes\n").unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    let out = loop {
        if let Some(out) = helper.poll().unwrap() {
            break out;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(
        out.ok(),
        "xdg-open failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"original selected bytes\n");
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    fs::remove_dir_all(root).unwrap();
}
