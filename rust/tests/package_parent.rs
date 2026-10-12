//! Deterministic output-parent replacement regression, no concurrent race loop.
use std::{fs, os::unix::fs::symlink};
use telebar_runtime::storage::Storage;
#[test]
fn exclusive_output_creation_stays_under_the_admitted_parent_inode() {
    let root = std::env::temp_dir().join(format!("telebar-package-parent-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let original = root.join("parent");
    let moved = root.join("admitted");
    let foreign = root.join("foreign");
    fs::create_dir(&original).unwrap();
    fs::create_dir(&foreign).unwrap();
    let parent = Storage::open(&original).unwrap();
    fs::rename(&original, &moved).unwrap();
    symlink(&foreign, &original).unwrap();
    // The old pathname implementation creates foreign/candidate here. The
    // pinned parent must instead create only admitted/candidate and write there.
    let output = parent.create_new_child("candidate").unwrap();
    output.write("result", b"owned", 0o600).unwrap();
    assert_eq!(fs::read(moved.join("candidate/result")).unwrap(), b"owned");
    assert!(!foreign.join("candidate").exists());
    assert!(parent.create_new_child("candidate").is_err());
    symlink(&foreign, moved.join("planted")).unwrap();
    assert!(parent.create_new_child("planted").is_err());
    assert!(parent.create_new_child("nested/output").is_err());
    assert_eq!(fs::read_dir(&foreign).unwrap().count(), 0);
    fs::remove_dir_all(root).unwrap();
}
