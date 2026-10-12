//! System and owned-helper trust remains enforced outside test-only namespace.
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
use telebar_runtime::executable::checked;
#[test]
fn loader_rejects_relative_writable_helpers_and_unsafe_ancestors() {
    assert!(checked(std::path::Path::new("cat")).is_err());
    assert!(checked(std::path::Path::new("/usr/bin/cat")).is_ok());
    let root = std::env::temp_dir().join(format!("telebar-executable-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let p = root.join("helper");
    fs::write(&p, b"inert").unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(checked(&p).is_ok());
    symlink(&p, root.join("link")).unwrap();
    assert!(checked(&root.join("link")).is_ok());
    fs::set_permissions(&p, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(checked(&p).is_err());
    fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(checked(&p).is_err());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::remove_dir_all(root).unwrap();
}
