//! Pure compositor binding policy and unavailable-context worker; no live I/O.
use serde_json::json;
use std::{collections::BTreeMap, sync::atomic::AtomicBool};
use telebar_runtime::global_shortcuts::{Context, Job, apply_with, bind_statement, lua_string};
#[test]
fn only_owned_unshared_bindings_change_and_commands_are_escaped() {
    let context = Context {
        plugin: "/fixture/plugin'quoted".into(),
        environment: BTreeMap::new(),
    };
    let desired = json!({"global.quickReply":"SUPER + Q","global.panel":"SUPER + P","global.openWindow":"CTRL + O"});
    let binds = json!([
        {"modmask":64,"key":"q","description":"Other binding"},
        {"modmask":64,"key":"q","description":"Omagram: Quick reply"},
        {"modmask":64,"key":"p","description":"Omagram: Bar panel"},
        {"modmask":64,"key":"x","description":"Omagram: Old action"},
        {"modmask":64,"key":"y","description":"Omagram: Old action"},
        {"modmask":64,"key":"y","description":"Other shared binding"}
    ]);
    let mut calls = Vec::new();
    let status = apply_with(&desired, &binds, &context, &mut |s| {
        calls.push(s.to_owned());
        true
    });
    assert_eq!(
        status,
        json!({"global.quickReply":"taken","global.panel":"active","global.openWindow":"active"})
    );
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0], "hl.unbind(\"SUPER + X\")");
    assert_eq!(
        calls[1],
        bind_statement("CTRL + O", "global.openWindow", &context).unwrap()
    );
    assert!(!calls[1].contains("python"));
    assert!(calls[1].contains("\\034"));
    assert_eq!(lua_string("a\"\n\\é"), "\"a\\034\\010\\092\\195\\169\"");
    let failed = apply_with(&desired, &json!([]), &context, &mut |_| false);
    assert_eq!(
        failed,
        json!({"global.quickReply":"failed","global.panel":"failed","global.openWindow":"failed"})
    );
    let unavailable = Job {
        context,
        settings: json!({"globalShortcuts":{"global.panel":"SUPER + P"}}),
    }
    .run(&AtomicBool::new(false))
    .unwrap();
    let telebar_runtime::commands::Action::Reply(view) = unavailable else {
        panic!("local status result")
    };
    assert_eq!(
        view["globalStatus"],
        json!({"global.quickReply":"off","global.panel":"unavailable","global.openWindow":"off"})
    );
}

#[test]
fn unbinding_keeps_captured_order_and_pre_cancel_has_no_effects() {
    let context = Context {
        plugin: "/fixture/plugin".into(),
        environment: BTreeMap::new(),
    };
    let binds = json!([
        {"modmask":64,"key":"z","description":"Omagram: Old"},
        {"modmask":4,"key":"a","description":"Omagram: Old"},
        {"modmask":64,"key":"z","description":"Omagram: New"}
    ]);
    let mut calls = Vec::new();
    apply_with(&json!({}), &binds, &context, &mut |s| {
        calls.push(s.to_owned());
        true
    });
    assert_eq!(
        calls,
        ["hl.unbind(\"SUPER + Z\")", "hl.unbind(\"CTRL + A\")"]
    );
    assert_eq!(
        Job {
            context,
            settings: json!({"globalShortcuts":{}})
        }
        .run(&AtomicBool::new(true))
        .unwrap_err(),
        "the connection was closed"
    );
}
