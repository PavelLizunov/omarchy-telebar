//! Registry-complete validation reachability. Not full behavioral coverage:
//! pure differential and actual-consumer evidence stay separate from this map.
use serde_json::json;
use std::fs;
use telebar_runtime::{
    application::Application, native_driver::Driver, registry, session, storage::Storage,
    wire::Request,
};
#[test]
fn every_registry_route_is_local_validated_or_native_without_missing_handler() {
    let root =
        std::env::temp_dir().join(format!("telebar-command-contract-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let home = Storage::open(&root).unwrap();
    let context =
        session::Context::new(root.clone(), root.clone(), Default::default(), "en".into()).unwrap();
    let mut checked = 0;
    for (command, _) in registry::COMMANDS {
        let mut app = Application::load(&home).unwrap();
        context.configure_application(&mut app).unwrap();
        let mut driver = Driver::without_native("default".into(), "en".into());
        driver
            .register("default".into(), "Synthetic".into(), vec![])
            .unwrap();
        // Make command planners reachable without any real native dispatch,
        // capture/upload/helper request. Empty args fail before device work.
        driver
            .runtime
            .engine
            .accounts
            .get_mut("default")
            .unwrap()
            .ready = true;
        driver.runtime.metadata.get_mut("default").unwrap().auth = json!({"state":"ready"});
        app.keyring_environment = None;
        app.clipboard_context = None;
        let request = Request::parse(
            json!({"id":1,"cmd":command,"args":{}})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        let effects = driver
            .request_application(&mut app, &home, 7, request)
            .unwrap();
        assert!(
            effects.native.is_empty(),
            "native work escaped missing-client guard: {command}"
        );
        assert_eq!(
            effects.disconnect, None,
            "normal validation lost peer: {command}"
        );
        let terminal = effects
            .replies
            .iter()
            .find(|(_, value)| value["id"] == 1)
            .unwrap_or_else(|| panic!("missing terminal validation: {command}"));
        let error = terminal.1["error"].as_str().unwrap_or("");
        assert!(
            !matches!(
                error,
                "unknown command" | "migration command is not implemented"
            ),
            "unimplemented registry route: {command}"
        );
        checked += 1;
        // No commands with empty args may be background/device operations.
        assert!(
            effects.work.is_empty(),
            "unexpected helper admission: {command}"
        );
        driver.close(std::time::Duration::ZERO).unwrap();
    }
    assert_eq!(checked, 170);
    fs::remove_dir_all(root).unwrap();
}
