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
fn legacy_python_flock_and_rust_exclude_each_other_without_opening_profiles() {
    use std::{
        collections::BTreeMap,
        time::{Duration, Instant},
    };
    use telebar_runtime::process::{self, Managed};
    let root = std::env::temp_dir().join(format!("telebar-legacy-lock-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let script = "import os,fcntl,sys,time\np=sys.argv[1]+'/omagram.lock'\nf=os.open(p,os.O_RDWR|os.O_CREAT|os.O_NOFOLLOW|os.O_CLOEXEC,0o600)\ntry: fcntl.flock(f,fcntl.LOCK_EX|fcntl.LOCK_NB)\nexcept BlockingIOError: print('blocked',flush=True);sys.exit(73)\nprint('locked',flush=True)\ntime.sleep(2)\n";
    let args = ["-I", "-c", script, root.to_str().unwrap()];
    let mut legacy = Managed::start(
        "/usr/bin/python3",
        &args,
        &BTreeMap::new(),
        &[],
        Duration::from_secs(3),
        1024,
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    loop {
        assert!(
            legacy.poll().unwrap().is_none(),
            "legacy fixture stopped before lock"
        );
        if legacy.output_snapshot().0 == b"locked\n" {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    // These exact open/flock flags and names match the preserved Python owner.
    // No Python daemon, TDLib, credentials or database is constructed here.
    let stale = UnixListener::bind(root.join("omagram.sock")).unwrap();
    let socket_inode = fs::symlink_metadata(root.join("omagram.sock"))
        .unwrap()
        .ino();
    let lock_inode = fs::symlink_metadata(root.join("omagram.lock"))
        .unwrap()
        .ino();
    let error = match Owner::acquire(&root, "omagram.sock", "omagram.lock") {
        Ok(_) => panic!("Rust admitted while legacy lock held"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    assert_eq!(
        fs::symlink_metadata(root.join("omagram.sock"))
            .unwrap()
            .ino(),
        socket_inode
    );
    legacy.cancel().unwrap();
    drop(stale);
    let native = Owner::acquire(&root, "omagram.sock", "omagram.lock").unwrap();
    assert_eq!(
        fs::symlink_metadata(root.join("omagram.lock"))
            .unwrap()
            .ino(),
        lock_inode
    );
    let refused = process::run(
        "/usr/bin/python3",
        &args,
        &BTreeMap::new(),
        &[],
        Duration::from_secs(3),
        1024,
    )
    .unwrap();
    assert_eq!(refused.code, Some(73));
    assert_eq!(refused.stdout, b"blocked\n");
    drop(native);
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
