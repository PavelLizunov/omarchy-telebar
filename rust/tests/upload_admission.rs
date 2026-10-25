//! A changing untrusted parent must not redirect an admitted upload into a database.
use serde_json::json;
use std::{
    fs,
    io::Read,
    os::unix::fs::{PermissionsExt, symlink},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use telebar_runtime::uploads::admit;

#[test]
fn directory_swap_cannot_bypass_database_exclusion() {
    let root = std::env::temp_dir().join(format!("telebar-upload-parent-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let database = root.join("database");
    let chosen = root.join("chosen");
    let held = root.join("held");
    fs::create_dir(&database).unwrap();
    fs::create_dir(&chosen).unwrap();
    fs::write(database.join("payload"), b"synthetic-private-database").unwrap();
    fs::write(chosen.join("payload"), b"chosen-public-file").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let running = stop.clone();
    let race_chosen = chosen.clone();
    let race_database = database.clone();
    let race = thread::spawn(move || {
        while !running.load(Ordering::Relaxed) {
            fs::rename(&race_chosen, &held).unwrap();
            symlink(&race_database, &race_chosen).unwrap();
            thread::sleep(Duration::from_micros(80));
            fs::remove_file(&race_chosen).unwrap();
            fs::rename(&held, &race_chosen).unwrap();
            thread::sleep(Duration::from_micros(80));
        }
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut attempts = 0;
    let mut admitted = 0;
    let mut private = false;
    while attempts < 20_000 && Instant::now() < deadline {
        attempts += 1;
        if let Ok(mut upload) = admit(
            &json!(chosen.join("payload")),
            std::slice::from_ref(&database),
        ) {
            let mut bytes = Vec::new();
            upload.file.read_to_end(&mut bytes).unwrap();
            admitted += 1;
            if bytes == b"synthetic-private-database" {
                private = true;
                break;
            }
            assert_eq!(bytes, b"chosen-public-file");
        }
    }
    stop.store(true, Ordering::Relaxed);
    race.join().unwrap();
    fs::remove_dir_all(&root).unwrap();
    eprintln!("upload parent race: attempts={attempts}, admitted={admitted}, private={private}");
    assert!(!private, "a swapped parent bypassed the database exclusion");
    assert!(
        admitted > 0,
        "the ordinary selected file must remain usable"
    );
}
