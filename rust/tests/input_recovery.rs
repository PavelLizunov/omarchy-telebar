//! Crash-left input journal and bounded admission. No native/database/device I/O.
use serde_json::json;
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use telebar_runtime::{
    storage::Storage,
    upload_lease::{self, Lease},
};
fn root(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("telebar-input-{name}-{}", std::process::id()));
    fs::create_dir(&p).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
    p
}
fn snapshot(store: &Storage, source: &Path, directory: &str) -> String {
    let mut file = fs::File::open(source).unwrap();
    let size = file.metadata().unwrap().len();
    store
        .snapshot_upload(
            directory,
            &mut file,
            size,
            "selected.txt",
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(3),
        )
        .unwrap()
}
#[test]
fn crash_left_pre_dispatch_copies_recover_but_uncertain_native_inputs_survive() {
    let root = root("recovery");
    let source = root.join("selected.txt");
    fs::write(&source, b"original user file").unwrap();
    let store = Storage::open(&root).unwrap();
    let directory = ".local/share/omagram/sent";
    let unsent = snapshot(&store, &source, directory);
    let submitted = snapshot(&store, &source, directory);
    let mut lease = Lease::new(root.clone()).unwrap();
    lease.add(submitted.clone()).unwrap();
    lease.submitted().unwrap();
    drop(lease);
    let corrupt = snapshot(&store, &source, directory);
    let parent = Path::new(&corrupt).parent().unwrap().to_str().unwrap();
    store
        .write(&format!("{parent}/{}", upload_lease::JOURNAL), b"{}", 0o600)
        .unwrap();
    let replaced = snapshot(&store, &source, directory);
    fs::rename(root.join(&replaced), root.join("former-copy")).unwrap();
    fs::write(root.join(&replaced), b"unrelated replacement").unwrap();
    let aliased = snapshot(&store, &source, directory);
    let parent = Path::new(&aliased).parent().unwrap().to_owned();
    symlink(&source, root.join(&parent).join("foreign-link")).unwrap();
    store
        .write(&format!("{directory}/legacy.ogg"), b"legacy keep", 0o600)
        .unwrap();
    assert_eq!(upload_lease::recover_unsent(&store, directory).unwrap(), 1);
    assert!(!root.join(unsent).exists());
    for p in [submitted, corrupt, replaced.clone(), aliased] {
        assert!(root.join(p).exists());
    }
    assert_eq!(
        fs::read(root.join(replaced)).unwrap(),
        b"unrelated replacement"
    );
    assert_eq!(fs::read(&source).unwrap(), b"original user file");
    assert_eq!(upload_lease::recover_unsent(&store, directory).unwrap(), 0);
    // Restore only the planted fixture link so new admission can proceed.
    fs::remove_file(root.join(parent).join("foreign-link")).unwrap();
    assert_eq!(upload_lease::recover_unsent(&store, directory).unwrap(), 1);
    // Malformed journals cannot smuggle a relative escape/deletion authority.
    let escaped = snapshot(&store, &source, directory);
    let container = Path::new(&escaped).parent().unwrap().to_str().unwrap();
    store.write(&format!("{container}/{}",upload_lease::JOURNAL),&serde_json::to_vec(&json!({"version":1,"submitted":false,"files":[{"name":"../selected.txt","dev":1,"ino":1}]})).unwrap(),0o600).unwrap();
    assert_eq!(upload_lease::recover_unsent(&store, directory).unwrap(), 0);
    assert!(root.join(escaped).exists());
    assert!(source.exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn pre_native_cleanup_and_submission_cannot_follow_a_replaced_home_or_input() {
    let root = root("pinned");
    let home = root.join("home");
    let moved = root.join("moved");
    let foreign = root.join("foreign");
    fs::create_dir(&home).unwrap();
    fs::create_dir(&foreign).unwrap();
    fs::write(home.join("source"), b"input").unwrap();
    let store = Storage::open(&home).unwrap();
    let path = snapshot(&store, &home.join("source"), "sent");
    let mut lease = Lease::new(home.clone()).unwrap();
    lease.add(path.clone()).unwrap();
    fs::rename(&home, &moved).unwrap();
    symlink(&foreign, &home).unwrap();
    assert!(
        lease.submitted().is_err(),
        "consumer-visible home identity must match, not only retained fd"
    );
    drop(lease);
    assert!(!moved.join(path).exists());
    assert_eq!(fs::read_dir(&foreign).unwrap().count(), 0);
    let store = Storage::open(&moved).unwrap();
    // A regular replacement tree is also refused, not just a symlink.
    fs::remove_file(&home).unwrap();
    fs::create_dir(&home).unwrap();
    let mut input = Lease::new(moved.clone()).unwrap();
    let copied = snapshot(&store, &moved.join("source"), "sent");
    input.add(copied.clone()).unwrap();
    let relocated = root.join("relocated");
    fs::rename(&moved, &relocated).unwrap();
    fs::create_dir(&moved).unwrap();
    let replacement = Storage::open(&moved).unwrap();
    replacement.write(&copied, b"foreign bytes", 0o600).unwrap();
    assert!(input.submitted().is_err());
    drop(input);
    assert_eq!(fs::read(moved.join(&copied)).unwrap(), b"foreign bytes");
    assert!(!relocated.join(copied).exists());
    fs::remove_dir_all(&moved).unwrap();
    fs::rename(&relocated, &moved).unwrap();
    let store = Storage::open(&moved).unwrap();
    let path = snapshot(&store, &moved.join("source"), "sent");
    let mut lease = Lease::new(moved.clone()).unwrap();
    lease.add(path.clone()).unwrap();
    fs::rename(moved.join(&path), moved.join("old")).unwrap();
    fs::write(moved.join(&path), b"replacement").unwrap();
    assert!(lease.submitted().is_err());
    drop(lease);
    assert_eq!(fs::read(moved.join(path)).unwrap(), b"replacement");
    // No durable submission => dispatch cannot continue with a missing journal.
    let path = snapshot(&store, &moved.join("source"), "sent");
    let mut lease = Lease::new(moved.clone()).unwrap();
    lease.add(path.clone()).unwrap();
    let container = Path::new(&path).parent().unwrap().to_str().unwrap();
    store
        .remove_file(&format!("{container}/{}", upload_lease::JOURNAL))
        .unwrap();
    assert!(lease.submitted().is_err());
    drop(lease);
    assert!(!moved.join(path).exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn admission_counts_legacy_and_uncertain_bytes_serializes_workers_and_never_evicts() {
    let root = root("budget");
    let store = Storage::open(&root).unwrap();
    store.ensure_dir("sent").unwrap();
    let mut huge = store.create_file("sent/legacy").unwrap();
    huge.set_len(upload_lease::MAX_BYTES).unwrap();
    huge.seek(SeekFrom::Start(0)).unwrap();
    huge.write_all(b"keep").unwrap();
    assert!(
        upload_lease::reserve(
            &store,
            "sent",
            1,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(1)
        )
        .is_err()
    );
    assert_eq!(huge.metadata().unwrap().len(), upload_lease::MAX_BYTES);
    // Payload at the boundary must include durable/atomic journal headroom.
    huge.set_len(upload_lease::MAX_BYTES - 1).unwrap();
    fs::write(root.join("one-byte"), b"x").unwrap();
    let mut source = fs::File::open(root.join("one-byte")).unwrap();
    assert!(
        store
            .snapshot_upload(
                "sent",
                &mut source,
                1,
                "one-byte",
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(1)
            )
            .is_err()
    );
    assert_eq!(huge.metadata().unwrap().len(), upload_lease::MAX_BYTES - 1);
    store.remove_file("sent/legacy").unwrap();
    let first = upload_lease::reserve(
        &store,
        "sent",
        64,
        &AtomicBool::new(false),
        Instant::now() + Duration::from_secs(1),
    )
    .unwrap();
    assert!(
        upload_lease::reserve(
            &store,
            "sent",
            64,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_millis(15)
        )
        .is_err()
    );
    drop(first);
    assert!(
        upload_lease::reserve(
            &store,
            "sent",
            64,
            &AtomicBool::new(true),
            Instant::now() + Duration::from_secs(1)
        )
        .is_err()
    );
    store
        .write(
            "sent/unrelated",
            &serde_json::to_vec(&json!({"keep":true})).unwrap(),
            0o600,
        )
        .unwrap();
    symlink(root.join("sent"), root.join("sent/link")).unwrap();
    assert!(
        upload_lease::reserve(
            &store,
            "sent",
            0,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(1)
        )
        .is_err()
    );
    assert!(root.join("sent/unrelated").exists());
    fs::remove_file(root.join("sent/link")).unwrap();
    for i in 0..upload_lease::MAX_ENTRIES - 2 {
        store.create_file(&format!("sent/item-{i}")).unwrap();
    }
    assert!(
        upload_lease::reserve(
            &store,
            "sent",
            1,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(1)
        )
        .is_err()
    );
    fs::remove_dir_all(root).unwrap();
}
