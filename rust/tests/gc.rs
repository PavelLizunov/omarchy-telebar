//! Legacy raw cleanup has a finite owned scope and protects pending inputs.
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    time::{Duration, SystemTime},
};
#[test]
fn stale_raw_only_and_protected_inputs_survive() {
    let root = std::env::temp_dir().join(format!("telebar-gc-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::create_dir_all(root.join("omagram/rec")).unwrap();
    let directory = root.join("omagram/rec");
    for name in ["note-old", "paste-old", "voice-old", "note-pending"] {
        fs::write(directory.join(name), b"keep").unwrap();
        let file = fs::File::open(directory.join(name)).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(7200))
            .unwrap();
    }
    symlink(directory.join("voice-old"), directory.join("note-link")).unwrap();
    let protected = vec![directory.join("note-pending").to_str().unwrap().into()];
    assert_eq!(
        telebar_runtime::gc::raw(&root, &protected, SystemTime::now()).unwrap(),
        1
    );
    assert!(directory.join("note-old").exists()); // May be crash-retained/uncertain.
    assert!(directory.join("voice-old").exists());
    assert!(directory.join("note-pending").exists());
    assert!(directory.join("note-link").is_symlink());
    fs::remove_dir_all(root).unwrap();
}
