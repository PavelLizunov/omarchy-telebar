//! Missing TDLib still exposes compatible hello/settings and a clean close.
use serde_json::json;
use std::fs;
use telebar_runtime::{
    application::Application, native_driver::Driver, storage::Storage, wire::Request,
};
#[test]
fn unavailable_native_library_keeps_local_consumer_usable() {
    let root = std::env::temp_dir().join(format!("telebar-no-library-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let home = Storage::open(&root).unwrap();
    let mut app = Application::load(&home).unwrap();
    let mut driver = Driver::open_optional(
        &root.join("missing/lib/libtdjson.so"),
        "default".into(),
        "en".into(),
    );
    driver
        .register("default".into(), "Inert".into(), vec![])
        .unwrap();
    assert!(
        driver
            .register("default".into(), "Duplicate".into(), vec![])
            .is_err()
    );
    assert!(
        driver
            .register("../foreign".into(), "Invalid".into(), vec![])
            .is_err()
    );
    let hello = Request::parse(b"{\"id\":1,\"cmd\":\"hello\",\"args\":{}}").unwrap();
    let result = driver
        .request_application(&mut app, &home, 7, hello)
        .unwrap();
    assert_eq!(result.replies[0].1["result"]["auth"]["state"], "noLibrary");
    let change = Request::parse(
        json!({"id":2,"cmd":"settings.quiet","args":{"quiet":true}})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    let result = driver
        .request_application(&mut app, &home, 7, change)
        .unwrap();
    assert_eq!(result.replies[0].1["ok"], true);
    assert_eq!(app.settings.value["quiet"], true);
    for (index, command, args) in [
        (3, "auth.phone", json!({"phone":"+12025550123"})),
        (4, "account.logout", json!({})),
        (5, "proxies.list", json!({})),
    ] {
        let request = Request::parse(
            json!({"id":index,"cmd":command,"args":args})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        let result = driver
            .request_application(&mut app, &home, 7, request)
            .unwrap();
        assert_eq!(
            result.replies[0].1["error"], "Telegram is not available",
            "{command}"
        );
        assert!(result.native.is_empty());
        assert_eq!(result.disconnect, None);
    }
    let request = Request::parse(b"{\"id\":6,\"cmd\":\"hello\",\"args\":{}}").unwrap();
    let result = driver
        .request_application(&mut app, &home, 7, request)
        .unwrap();
    assert_eq!(result.replies[0].1["result"]["auth"]["state"], "noLibrary");
    driver.close(std::time::Duration::ZERO).unwrap();
    fs::remove_dir_all(root).unwrap();
}
