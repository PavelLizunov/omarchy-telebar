//! Real finite upload worker and Engine continuation without native sends.
use serde_json::json;
use std::{
    fs,
    io::Read,
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};
use telebar_runtime::{
    commands::Action,
    engine::{Account, Engine},
    jobs::{Pool, Work},
    state::State,
    wire::Request,
};
fn setup() -> (std::path::PathBuf, Engine) {
    let root = std::env::temp_dir().join(format!("telebar-jobs-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let mut e = Engine::new("default".into());
    e.accounts.insert(
        "default".into(),
        Account {
            client: 1,
            generation: 1,
            state: State::default(),
            ready: true,
        },
    );
    (root, e)
}
fn request(id: i32) -> Request {
    Request::parse(
        json!({"id":id,"cmd":"message.sendFile","args":{"chatId":42}})
            .to_string()
            .as_bytes(),
    )
    .unwrap()
}
#[test]
fn admitted_upload_pins_bytes_and_disconnect_prevents_send() {
    let (root, mut engine) = setup();
    let selected = root.join("chosen.txt");
    fs::write(&selected, b"original chosen bytes").unwrap();
    let args = json!({"chatId":42,"path":selected,"caption":"synthetic"});
    let work = Work::prepare_upload(
        "message.sendFile",
        &args,
        &State::default(),
        root.clone(),
        vec![root.join("database")],
        0.0,
        &mut |_| None,
    )
    .unwrap();
    // Rename/replacement cannot change the descriptor admitted for this request.
    fs::rename(&selected, root.join("old.txt")).unwrap();
    fs::write(&selected, b"replacement private bytes").unwrap();
    let mut work = Some(work);
    let mut effects = engine.request_using(7, request(1), Instant::now(), &mut |_, _, _| {
        Ok(Action::Work(work.take().unwrap()))
    });
    assert!(effects.replies.is_empty() && effects.native.is_empty());
    let (token, work) = effects.work.pop().unwrap();
    let mut pool = Pool::default();
    pool.start(token, work).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let done = loop {
        let mut done = pool.poll(&|t| engine.job_current(t));
        if let Some(done) = done.pop() {
            break done;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    let effects = engine.job_complete(done.token, done.result, Instant::now());
    assert_eq!(effects.native.len(), 1);
    let query = &effects.native[0].query;
    let path = query["input_message_content"]["document"]["document"]["path"]
        .as_str()
        .unwrap();
    assert_eq!(
        std::path::Path::new(path).file_name().unwrap(),
        "chosen.txt"
    );
    let mut bytes = Vec::new();
    fs::File::open(path)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes, b"original chosen bytes");
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let work = Work::prepare_upload(
        "message.sendFile",
        &args,
        &State::default(),
        root.clone(),
        vec![],
        0.0,
        &mut |_| None,
    )
    .unwrap();
    let mut work = Some(work);
    let effects = engine.request_using(8, request(2), Instant::now(), &mut |_, _, _| {
        Ok(Action::Work(work.take().unwrap()))
    });
    let (token, work) = effects.work.into_iter().next().unwrap();
    pool.start(token, work).unwrap();
    engine.retire_peer(8);
    let done = loop {
        let mut done = pool.poll(&|t| engine.job_current(t));
        if let Some(done) = done.pop() {
            break done;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    let effects = engine.job_complete(done.token, done.result, Instant::now());
    assert!(effects.native.is_empty() && effects.replies.is_empty());
    // Deadline retirement precedes helper completion, including reused IDs.
    let work = Work::prepare_upload(
        "message.sendFile",
        &args,
        &State::default(),
        root.clone(),
        vec![],
        0.0,
        &mut |_| None,
    )
    .unwrap();
    let mut work = Some(work);
    let at = Instant::now();
    let effect = engine.request_using(10, request(4), at, &mut |_, _, _| {
        Ok(Action::Work(work.take().unwrap()))
    });
    let (token, work) = effect.work.into_iter().next().unwrap();
    pool.start(token, work).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    let done = loop {
        let mut done = pool.poll(&|t| engine.job_current(t));
        if let Some(done) = done.pop() {
            break done;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    };
    let late = engine.job_complete(
        done.token,
        done.result,
        at + telebar_runtime::lifecycle::REQUEST_TIMEOUT,
    );
    assert!(late.native.is_empty());
    // The first synthetic send is also outstanding; expiry retires both.
    assert_eq!(late.replies.len(), 2);
    assert!(late.replies.iter().all(|(_, reply)| reply["ok"] == false));
    // Original message and server permission are both checked before any save.
    let media = root.join("files");
    fs::create_dir(&media).unwrap();
    fs::set_permissions(&media, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(media.join("source"), b"synthetic export").unwrap();
    engine.accounts.get_mut("default").unwrap().state.roots = vec![media.to_str().unwrap().into()];
    let args = json!({"chatId":42,"messageId":7,"fileId":3,"fileName":"photo.txt"});
    let req = Request::parse(
        json!({"id":3,"cmd":"file.save","args":args})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    let mut effects = engine.request_using(9, req, Instant::now(), &mut |_, args, _| {
        telebar_runtime::export::prepare_save(args, root.clone())
    });
    for mut event in [
        json!({"@type":"file","id":3,"local":{"is_downloading_completed":true,"path":media.join("source")}}),
        json!({"@type":"message","chat_id":42,"id":7,"content":{"@type":"messageDocument","document":{"@type":"document","document":{"@type":"file","id":3}}}}),
        json!({"@type":"messageProperties","can_be_saved":true}),
    ] {
        let native = effects.native.pop().unwrap();
        event["@extra"] = native.query["@extra"].clone();
        event["@client_id"] = json!(1);
        effects = engine.receive(event);
    }
    assert_eq!(effects.work.len(), 1);
    let (token, work) = effects.work.pop().unwrap();
    pool.start(token, work).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    let done = loop {
        let mut done = pool.poll(&|t| engine.job_current(t));
        if let Some(done) = done.pop() {
            break done;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    };
    let effects = engine.job_complete(done.token, done.result, Instant::now());
    assert_eq!(effects.replies[0].1["ok"], true);
    assert_eq!(
        fs::read(root.join("Downloads/photo.txt")).unwrap(),
        b"synthetic export"
    );
    let second = telebar_runtime::files::save(
        media.join("source").to_str().unwrap(),
        &[media.to_str().unwrap().into()],
        telebar_runtime::files::Save {
            home: root.clone(),
            name: "photo.txt".into(),
        },
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    assert!(matches!(second, Action::Reply(_)));
    assert_eq!(
        fs::read(root.join("Downloads/photo (2).txt")).unwrap(),
        b"synthetic export"
    );
    drop(pool);
    fs::remove_dir_all(root).unwrap();
}
