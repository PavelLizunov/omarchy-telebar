//! Mixed album groups through actual worker/Engine; no native sends. Exact
//! per-group input owners retain accepted/uncertain siblings and clean unsent.
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
use telebar_runtime::{
    commands::Action,
    engine::{Account, Effects, Engine},
    jobs::{Pool, Work},
    state::State,
    wire::Request,
};
fn prepared(root: &Path, engine: &mut Engine, id: u64) -> Effects {
    let args = json!({"chatId":42,"paths":[root.join("photo-a.png"),root.join("document.txt"),root.join("photo-b.png")],"caption":"synthetic album","replyToMessageId":99,"topicId":7,"silent":true});
    let work = Work::prepare_upload(
        "message.sendFiles",
        &args,
        &State::default(),
        root.into(),
        vec![root.join("database")],
        0.0,
        &mut |_| None,
    )
    .unwrap();
    let mut work = Some(work);
    let request = Request::parse(
        json!({"id":id,"cmd":"message.sendFiles","args":args})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    let sent = engine.request_using(7, request, Instant::now(), &mut |_, _, _| {
        Ok(Action::Work(work.take().unwrap()))
    });
    let (token, work) = sent.work.into_iter().next().unwrap();
    let mut pool = Pool::default();
    pool.start(token, work).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(done) = pool.poll(&|t| engine.job_current(t)).pop() {
            return engine.job_complete(done.token, done.result, Instant::now());
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn paths(native: &Value) -> Vec<String> {
    let contents = native["input_message_contents"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| vec![native["input_message_content"].clone()]);
    contents
        .into_iter()
        .map(|v| {
            if v["photo"].is_object() {
                v["photo"]["photo"]["path"].as_str().unwrap().into()
            } else {
                v["document"]["document"]["path"].as_str().unwrap().into()
            }
        })
        .collect()
}
#[test]
fn mixed_album_acceptance_failure_and_uncertainty_remain_group_owned() {
    let root = std::env::temp_dir().join(format!("telebar-upload-groups-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    for n in ["photo-a.png", "photo-b.png", "document.txt"] {
        fs::write(root.join(n), b"synthetic chosen bytes").unwrap();
    }
    for (case, mode) in [
        (0, "success"),
        (1, "sibling-rejection"),
        (2, "uncertain"),
        (3, "unsubmitted"),
    ] {
        let mut engine = Engine::new("default".into());
        engine.accounts.insert(
            "default".into(),
            Account {
                client: 41,
                generation: 1,
                state: State::default(),
                ready: true,
            },
        );
        let sent = prepared(&root, &mut engine, case + 1);
        assert_eq!(sent.native.len(), 2);
        let album = &sent.native[0].query;
        let document = &sent.native[1].query;
        assert_eq!(album["@type"], "sendMessageAlbum");
        assert_eq!(document["@type"], "sendMessage");
        assert_eq!(album["reply_to"]["message_id"], 99);
        assert!(document["reply_to"].is_null());
        assert_eq!(album["topic_id"]["forum_topic_id"], 7);
        assert_eq!(album["options"]["disable_notification"], true);
        assert_eq!(
            album["input_message_contents"][0]["caption"]["text"],
            "synthetic album"
        );
        assert!(album["input_message_contents"][1]["caption"].is_null());
        assert!(document["input_message_content"]["caption"].is_null());
        let album_paths = paths(album);
        let doc_paths = paths(document);
        assert_eq!(album_paths.len(), 2);
        assert_eq!(doc_paths.len(), 1);
        if mode == "unsubmitted" {
            engine.retire_peer(7);
            assert!(
                album_paths
                    .iter()
                    .chain(&doc_paths)
                    .all(|p| !Path::new(p).exists())
            );
            continue;
        }
        for native in &sent.native {
            engine.native_dispatched(&native.query).unwrap();
        }
        let first = engine.receive(
            json!({"@client_id":41,"@extra":album["@extra"],"@type":"messages","messages":[]}),
        );
        assert!(first.replies.is_empty());
        let done=match mode {"success"=>engine.receive(json!({"@client_id":41,"@extra":document["@extra"],"@type":"message","chat_id":42,"id":7})),"sibling-rejection"=>engine.receive(json!({"@client_id":41,"@extra":document["@extra"],"@type":"error","code":400,"message":"synthetic document rejected"})),_=>{engine.retire_peer(7);Effects::default()}};
        if mode == "success" {
            assert_eq!(done.replies[0].1["ok"], true);
            assert_eq!(done.replies[0].1["result"]["groups"], 2);
        } else if mode == "sibling-rejection" {
            assert_eq!(done.replies[0].1["error"], "synthetic document rejected");
            assert!(!Path::new(&doc_paths[0]).exists());
        }
        assert!(album_paths.iter().all(|p| Path::new(p).exists()));
        if mode == "uncertain" {
            assert!(Path::new(&doc_paths[0]).exists());
        }
        drop(engine);
    }
    for n in ["photo-a.png", "photo-b.png", "document.txt"] {
        assert_eq!(fs::read(root.join(n)).unwrap(), b"synthetic chosen bytes");
    }
    fs::remove_dir_all(root).unwrap();
}
