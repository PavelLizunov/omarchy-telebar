//! Real foreground ffplay with dummy audio only; no Wayland/PipeWire/profile.
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};
use telebar_runtime::{
    commands::Action,
    engine::{Account, Engine},
    playback::Selection,
    process,
    state::State,
    wire::Request,
};
#[test]
fn playback_is_admitted_pinned_and_expired_selections_cannot_launch() {
    let root = std::env::temp_dir().join(format!("telebar-playback-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("voice.wav");
    let fixture = process::run(
        "/usr/bin/ffmpeg",
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=330:sample_rate=32000",
            "-t",
            "0.2",
            path.to_str().unwrap(),
        ],
        &BTreeMap::new(),
        &[],
        Duration::from_secs(5),
        4096,
    )
    .unwrap();
    assert!(fixture.ok());
    let mut engine = Engine::new("default".into());
    engine.accounts.insert(
        "default".into(),
        Account {
            client: 41,
            generation: 1,
            state: State::new(vec![root.to_str().unwrap().into()]),
            ready: true,
        },
    );
    let environment = BTreeMap::from([
        ("SDL_AUDIODRIVER".into(), "dummy".into()),
        ("SDL_VIDEODRIVER".into(), "dummy".into()),
    ]);
    let args = json!({"chatId":42,"messageId":7,"fileId":3});
    let req = |id| {
        Request::parse(
            json!({"id":id,"cmd":"media.play","args":args})
                .to_string()
                .as_bytes(),
        )
        .unwrap()
    };
    let now = Instant::now();
    let effects = engine.request_using(7, req(1), now, &mut |_, args, _| {
        Selection::prepare(args, 9, 1.5, environment.clone())
    });
    assert_eq!(effects.native[0].query["@type"], "downloadFile");
    let effects=engine.receive(json!({"@type":"file","id":3,"@client_id":41,"@extra":effects.native[0].query["@extra"],"local":{"path":path,"is_downloading_completed":true}}));
    assert_eq!(effects.playback.len(), 1, "{:?}", effects.replies);
    assert!(effects.replies.is_empty());
    let (token, start) = effects.playback.into_iter().next().unwrap();
    assert_eq!(engine.job_owner(token), Some(("default".into(), 1)));
    let (mut helper, source) = start.spawn().unwrap();
    let pid = helper.pid();
    fs::rename(&path, root.join("original.wav")).unwrap();
    fs::write(&path, b"replacement").unwrap();
    assert!(source.metadata().unwrap().len() > 11);
    let until = Instant::now() + Duration::from_secs(5);
    let output = loop {
        if let Some(output) = helper.poll().unwrap() {
            break output;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(output.ok(), "dummy-audio player failed");
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    let done = engine.job_complete(
        token,
        Ok(Action::Reply(json!({"startedAt":1}))),
        Instant::now(),
    );
    assert_eq!(done.replies[0].1["ok"], true);
    let effects = engine.request_using(7, req(2), now, &mut |_, args, _| {
        Selection::prepare(args, 10, 1.0, environment.clone())
    });
    let expired = engine.expire(now + telebar_runtime::lifecycle::REQUEST_TIMEOUT);
    assert_eq!(expired.replies[0].1["ok"], false);
    let late=engine.receive(json!({"@type":"file","id":3,"@client_id":41,"@extra":effects.native[0].query["@extra"],"local":{"path":path,"is_downloading_completed":true}}));
    assert!(late.playback.is_empty());
    assert!(Selection::prepare(&json!({"chatId":false}), 11, 1.0, environment).is_err());
    fs::remove_dir_all(root).unwrap();
}
