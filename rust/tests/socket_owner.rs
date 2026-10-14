//! Single-owner exclusion and anchored cleanup use real Unix socket inodes.
use std::{
    fs,
    os::unix::{
        fs::{MetadataExt, PermissionsExt, symlink},
        net::UnixListener,
    },
};
use telebar_runtime::socket_owner::Owner;

#[test]
fn native_flock_and_socket_owner_exclude_each_other_without_opening_profiles() {
    use std::{
        fs::OpenOptions,
        os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    };
    let root = std::env::temp_dir().join(format!("telebar-flock-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("omagram.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
        .unwrap();
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    let stale = UnixListener::bind(root.join("omagram.sock")).unwrap();
    let socket_inode = fs::symlink_metadata(root.join("omagram.sock"))
        .unwrap()
        .ino();
    let lock_inode = lock.metadata().unwrap().ino();
    let error = match Owner::acquire(&root, "omagram.sock", "omagram.lock") {
        Ok(_) => panic!("native owner admitted while lock held"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    assert_eq!(
        fs::symlink_metadata(root.join("omagram.sock"))
            .unwrap()
            .ino(),
        socket_inode
    );
    drop(lock);
    drop(stale);
    let owner = Owner::acquire(&root, "omagram.sock", "omagram.lock").unwrap();
    let competitor = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
        .unwrap();
    assert_eq!(competitor.metadata().unwrap().ino(), lock_inode);
    assert_eq!(
        unsafe { libc::flock(competitor.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().kind(),
        std::io::ErrorKind::WouldBlock
    );
    drop(owner);
    assert!(!root.join("omagram.sock").exists());
    assert!(!root.join("database").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ownership_excludes_competitors_and_preserves_replaced_paths() {
    let root = std::env::temp_dir().join(format!("telebar-socket-owner-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let socket = root.join("omagram.sock");
    let owner = Owner::acquire(&root, "omagram.sock", "omagram.lock").unwrap();
    assert_eq!(fs::symlink_metadata(&socket).unwrap().mode() & 0o777, 0o600);
    assert!(Owner::acquire(&root, "omagram.sock", "omagram.lock").is_err());
    fs::remove_file(&socket).unwrap();
    fs::write(&socket, b"keep replacement").unwrap();
    drop(owner);
    assert_eq!(fs::read(&socket).unwrap(), b"keep replacement");
    assert!(Owner::acquire(&root, "omagram.sock", "omagram.lock").is_err());
    fs::remove_file(&socket).unwrap();
    let stale = UnixListener::bind(&socket).unwrap();
    drop(stale);
    let owner = Owner::acquire(&root, "omagram.sock", "omagram.lock").unwrap();
    // Rename does not redirect cleanup to the replacement directory.
    let moved = root.with_extension("moved");
    fs::rename(&root, &moved).unwrap();
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&socket, b"foreign replacement directory").unwrap();
    drop(owner);
    assert!(!moved.join("omagram.sock").exists());
    assert_eq!(fs::read(&socket).unwrap(), b"foreign replacement directory");
    fs::remove_file(&socket).unwrap();
    symlink(moved.join("omagram.lock"), root.join("omagram.lock")).unwrap();
    assert!(Owner::acquire(&root, "omagram.sock", "omagram.lock").is_err());
    assert!(Owner::acquire(&root, "../foreign", "omagram.lock").is_err());
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(moved).unwrap();
}
