//! Permission-gated PNG preparation without contacting Wayland.
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    sync::atomic::AtomicBool,
    time::Instant,
};
use telebar_runtime::{
    clipboard,
    commands::{Action, Answer},
    engine::{Account, Engine},
    jobs::Work,
    state::State,
    wire::Request,
};
#[test]
fn copying_requires_original_photo_permission_and_owned_bounded_bytes() {
    let root = std::env::temp_dir().join(format!("telebar-copy-image-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("photo.png");
    fs::write(&path, b"\x89PNG\r\n\x1a\nsynthetic").unwrap();
    let context = clipboard::Context {
        runtime: root.clone(),
        databases: vec![],
        environment: BTreeMap::new(),
    };
    let roots = vec![root.to_str().unwrap().into()];
    let mut engine = Engine::new("default".into());
    engine.accounts.insert(
        "default".into(),
        Account {
            client: 1,
            generation: 1,
            state: State::new(roots.clone()),
            ready: true,
        },
    );
    let args = json!({"chatId":42,"messageId":7,"fileId":3});
    let req = Request::parse(
        json!({"id":1,"cmd":"file.copyImage","args":args})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    let mut effects = engine.request_using(7, req, Instant::now(), &mut |_, args, _| {
        telebar_runtime::export::prepare_copy(args, context.clone())
    });
    for mut event in [
        json!({"@type":"file","id":3,"local":{"is_downloading_completed":true,"path":path}}),
        json!({"@type":"message","chat_id":42,"id":7,"content":{"@type":"messagePhoto","photo":{"@type":"photo","sizes":[{"@type":"photoSize","width":10,"height":10,"photo":{"@type":"file","id":3}}]}}}),
        json!({"@type":"messageProperties","can_be_saved":true}),
    ] {
        assert!(effects.work.is_empty());
        let native = effects.native.pop().unwrap();
        event["@extra"] = native.query["@extra"].clone();
        event["@client_id"] = json!(1);
        effects = engine.receive(event);
    }
    assert_eq!(effects.work.len(), 1);
    let (token, work) = effects.work.pop().unwrap();
    let Work::CopyImage {
        path: source,
        roots: allowed,
        context: ctx,
    } = work
    else {
        panic!("copy job")
    };
    let offer = clipboard::prepare_copy(&source, &allowed, ctx, &AtomicBool::new(false)).unwrap();
    let done = engine.job_complete(token, Ok(offer), Instant::now());
    assert_eq!(done.clipboard.len(), 1);
    assert!(done.replies.is_empty());
    assert!(engine.job_current(token));
    assert!(
        clipboard::prepare_copy(&source, &allowed, context.clone(), &AtomicBool::new(true))
            .is_err()
    );
    symlink(&path, root.join("link.png")).unwrap();
    assert!(
        clipboard::prepare_copy(
            root.join("link.png").to_str().unwrap(),
            &roots,
            context,
            &AtomicBool::new(false)
        )
        .is_err()
    );
    let gate = telebar_runtime::export::Gate::Permission(
        source,
        None,
        Some(clipboard::Context {
            runtime: root.clone(),
            databases: vec![],
            environment: BTreeMap::new(),
        }),
    );
    assert!(
        gate.complete(&json!({"can_be_saved":false}), &State::new(roots))
            .is_err()
    );
    let Action::Native {
        answer: Answer::Export(_),
        ..
    } = telebar_runtime::export::prepare_copy(
        &args,
        clipboard::Context {
            runtime: root.clone(),
            databases: vec![],
            environment: BTreeMap::new(),
        },
    )
    .unwrap()
    else {
        panic!("gate")
    };
    fs::remove_dir_all(root).unwrap();
}
