//! Real driver/TDLib parameter boundary with deliberately invalid explicit root.
//! The worker refuses storage before Secret Service; no profile is opened.
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};
use telebar_runtime::{native_driver::Driver, startup::Context};
#[test]
#[ignore = "requires an explicitly provided verified TDLib library"]
fn asynchronous_startup_failure_is_bounded_and_never_prepares_real_parameters() {
    let library = std::env::var_os("TELEBAR_TEST_TDLIB")
        .expect("explicit verified TDLib library is required for this opt-in test");
    let root = std::env::temp_dir().join(format!("telebar-startup-driver-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(root.join("not-home"), b"synthetic").unwrap();
    let mut driver = Driver::open(
        std::path::Path::new(&library),
        "default".into(),
        "en".into(),
    )
    .unwrap();
    driver.set_startup_context(Context {
        home: root.join("not-home"),
        environment: BTreeMap::new(),
        version: "fixture".into(),
        language: "en".into(),
    });
    driver
        .register("default".into(), "Synthetic".into(), vec![])
        .unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    let mut observed = false;
    while Instant::now() < until {
        let effect = driver
            .tick(&mut |_| {
                panic!("configured startup uses the bounded worker, not the synchronous provider")
            })
            .unwrap();
        observed |= effect
            .replies
            .iter()
            .any(|(_, r)| r["auth"]["state"] == "error");
        if observed {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(observed);
    assert_eq!(
        driver.runtime.metadata["default"].auth["reason"],
        "the startup credentials could not be prepared; check the keyring and account storage"
    );
    assert!(!driver.runtime.engine.accounts["default"].ready);
    driver.close(Duration::from_secs(5)).unwrap();
    fs::remove_dir_all(root).unwrap();
}
