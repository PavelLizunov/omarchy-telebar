//! Explicit sound preparation/cache and real private PipeWire consumer.
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use telebar_runtime::{commands::Action, sound_playback::Job};
#[test]
fn sound_cache_is_bounded_owned_and_cancelled_without_a_device() {
    let root = std::env::temp_dir().join(format!("telebar-sound-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let settings = json!({"sounds":{"style":"pop","variants":{"7":2}}});
    assert!(Job::prepare(root.clone(), 0, &settings, BTreeMap::new()).is_none());
    assert!(
        Job::prepare(
            root.clone(),
            7,
            &json!({"sounds":{"style":"off"}}),
            BTreeMap::new()
        )
        .is_none()
    );
    assert!(
        Job::prepare(root.clone(), 7, &settings, BTreeMap::new())
            .unwrap()
            .run(&AtomicBool::new(true))
            .is_err()
    );
    assert!(!root.join(".cache").exists());
    let action = Job::prepare(root.clone(), 7, &settings, BTreeMap::new())
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    let Action::SoundReady(_) = action else {
        panic!("sound")
    };
    let directory = root.join(".cache/omagram/sounds");
    let path = fs::read_dir(&directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let bytes = fs::read(&path).unwrap();
    assert_eq!(
        bytes,
        telebar_runtime::sounds::render(&telebar_runtime::sounds::motif("7", 2), "pop").unwrap()
    );
    assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
    fs::rename(&path, root.join("original.wav")).unwrap();
    symlink(root.join("original.wav"), &path).unwrap();
    assert!(
        Job::prepare(root.clone(), 7, &settings, BTreeMap::new())
            .unwrap()
            .run(&AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(fs::read(root.join("original.wav")).unwrap(), bytes);
    fs::remove_file(&path).unwrap();
    fs::write(&path, b"bad cache").unwrap();
    assert!(
        Job::prepare(root.clone(), 7, &settings, BTreeMap::new())
            .unwrap()
            .run(&AtomicBool::new(false))
            .is_err()
    );
    fs::remove_file(&path).unwrap();
    for i in 0..300 {
        fs::write(directory.join(format!("fixture-{i}.wav")), b"synthetic").unwrap();
    }
    Job::prepare(root.clone(), 7, &settings, BTreeMap::new())
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}
#[test]
#[ignore = "requires reviewed private PipeWire namespace"]
fn actual_notification_sound_uses_only_the_isolated_null_sink() {
    assert_eq!(
        std::env::var("TELEBAR_ISOLATED_PIPEWIRE").as_deref(),
        Ok("1")
    );
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap();
    assert_eq!(
        std::path::Path::new(&runtime).parent(),
        Some(std::path::Path::new("/tmp"))
    );
    assert!(runtime.contains("telebar-pipewire-"));
    let environment = BTreeMap::from([("XDG_RUNTIME_DIR".into(), runtime.clone())]);
    let settings = json!({"sounds":{"style":"pop","variants":{}}});
    let Action::SoundReady(_prepared) =
        Job::prepare(runtime.clone().into(), 7, &settings, environment)
            .unwrap()
            .run(&AtomicBool::new(false))
            .unwrap()
    else {
        panic!("prepared")
    };
    // Integration crates link the non-test library: trust checks are not
    // overridden for overflow UID. This private namespace verifies PipeWire
    // consumer output; host tests separately verify Prepared::spawn policy.
    use std::os::unix::fs::MetadataExt;
    let tool = std::path::Path::new("/usr/bin/pw-play");
    let m = tool.metadata().unwrap();
    assert!(m.is_file() && m.mode() & 0o022 == 0 && m.mode() & 0o111 != 0);
    let sound = fs::read_dir(std::path::Path::new(&runtime).join(".cache/omagram/sounds"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut child = std::process::Command::new(tool)
        .args(["--media-role", "Notification"])
        .arg(sound)
        .env_clear()
        .env("XDG_RUNTIME_DIR", &runtime)
        .spawn()
        .unwrap();
    let pid = child.id();
    let deadline = Instant::now() + Duration::from_secs(5);
    let output = loop {
        if let Some(out) = child.try_wait().unwrap() {
            break out;
        }
        assert!(Instant::now() < deadline, "isolated sound did not finish");
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(output.success(), "private PipeWire playback failed");
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
}
