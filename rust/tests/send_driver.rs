//! Explicit local send oracle, actual Driver/worker/FFI/projection owners.
//! Requires the identity-preserving no-network/no-host-input fixture wrapper.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use telebar_runtime::{
    application::Application,
    commands::{Action, Answer},
    engine::Effects,
    native_driver::Driver,
    storage::Storage,
    wire::Request,
};
fn request(id: u64, command: &str, args: Value) -> Request {
    Request::parse(
        json!({"id":id,"cmd":command,"args":args})
            .to_string()
            .as_bytes(),
    )
    .unwrap()
}
fn collect(
    driver: &mut Driver,
    app: &mut Application,
    home: &Storage,
    id: u64,
    mut effects: Effects,
) -> Vec<Value> {
    let mut frames = Vec::new();
    let until = Instant::now() + Duration::from_secs(12);
    loop {
        frames.extend(effects.replies.into_iter().map(|(_, frame)| frame));
        if frames.iter().any(|f| f["id"] == id) {
            return frames;
        }
        assert!(
            Instant::now() < until,
            "finite actual Driver reply {id}: {frames:?}"
        );
        effects = driver
            .tick_application(app, home, &mut |_| {
                panic!("send fixture cannot prepare parameters")
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn command(
    driver: &mut Driver,
    app: &mut Application,
    home: &Storage,
    id: u64,
    cmd: &str,
    args: Value,
) -> Vec<Value> {
    let effects = driver
        .request_application(app, home, 7, request(id, cmd, args))
        .unwrap();
    collect(driver, app, home, id, effects)
}
fn reply(frames: &[Value], id: u64) -> &Value {
    frames.iter().find(|f| f["id"] == id).unwrap()
}
fn files(root: &Path) -> Vec<PathBuf> {
    if !root.exists() {
        return vec![];
    }
    let mut out = vec![];
    for item in fs::read_dir(root).unwrap() {
        let p = item.unwrap().path();
        if p.is_dir() {
            out.extend(files(&p));
        } else if p.file_name().unwrap() != telebar_runtime::upload_lease::JOURNAL
            && p.file_name().unwrap() != ".telebar-input.lock"
        {
            out.push(p);
        }
    }
    out.sort();
    out
}
#[test]
#[ignore = "requires explicit synthetic send-ABI library and identity-preserving confined fixture"]
fn actual_send_driver_acceptance_keeps_playback_inputs_and_originals() {
    assert_eq!(std::env::var("TELEBAR_PRIVATE_SEND").as_deref(), Ok("1"));
    for family in [libc::AF_INET, libc::AF_INET6] {
        // SAFETY: a negative-control socket is never connected and is closed
        // immediately if the enforced address-family contract is absent.
        let fd = unsafe { libc::socket(family, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0) };
        if fd >= 0 {
            unsafe { libc::close(fd) };
            panic!("fixture network denial missing");
        }
    }
    assert!(fs::File::open(std::env::var_os("TELEBAR_DENIED_FILE").unwrap()).is_err());
    assert!(
        std::os::unix::net::UnixStream::connect(std::env::var_os("TELEBAR_DENIED_SOCKET").unwrap())
            .is_err()
    );
    assert!(std::os::unix::net::UnixStream::connect("/run/user/1000/bus").is_err());
    assert!(fs::File::open("/dev/full").is_err());
    let root =
        PathBuf::from(std::env::var_os("TELEBAR_PRIVATE_ROOT").unwrap()).join("fixture-home");
    fs::create_dir(&root).unwrap();
    let home = Storage::open(&root).unwrap();
    home.ensure_dir("runtime/omagram/rec").unwrap();
    let runtime = root.join("runtime");
    let mut ppm = b"P6\n12 8\n255\n".to_vec();
    ppm.extend([128, 64, 32].repeat(12 * 8));
    home.write("selected.ppm", &ppm, 0o600).unwrap();
    home.write("photo-a.png", &ppm, 0o600).unwrap();
    home.write("photo-b.png", &ppm, 0o600).unwrap();
    home.write("document.txt", b"synthetic chosen document", 0o600)
        .unwrap();
    let source = runtime.join("omagram/rec/note.mp4");
    let out = telebar_runtime::process::run(
        "/usr/bin/ffmpeg",
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=80x60:r=10",
            "-t",
            "1.2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            source.to_str().unwrap(),
        ],
        &Default::default(),
        &[],
        Duration::from_secs(8),
        65536,
    )
    .unwrap();
    assert!(
        out.ok(),
        "synthetic source: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let source_bytes = fs::read(&source).unwrap();
    let mut app = Application::load(&home).unwrap();
    app.upload_context = Some((
        root.clone(),
        vec![root.join(".local/share/omagram/database")],
    ));
    app.recording_runtime = Some(runtime.clone());
    app.clipboard_context = Some(telebar_runtime::clipboard::Context {
        runtime: runtime.clone(),
        databases: vec![],
        environment: Default::default(),
    });
    let library = std::env::var_os("TELEBAR_TEST_INERT_ABI").unwrap();
    let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
    let roots =
        telebar_runtime::paths::AccountPaths::new(&root.join(".local/share/omagram"), "default")
            .unwrap()
            .media_roots(&runtime);
    driver
        .register("default".into(), "Synthetic".into(), roots)
        .unwrap();
    driver
        .runtime
        .engine
        .accounts
        .get_mut("default")
        .unwrap()
        .state
        .retained
        .attach(
            telebar_runtime::recording_checkpoint::Checkpoint::open(&root, &runtime, "default")
                .unwrap(),
        )
        .unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    while !driver.runtime.engine.accounts["default"].ready {
        assert!(Instant::now() < until);
        driver
            .tick_application(&mut app, &home, &mut |_| panic!("no credential startup"))
            .unwrap();
    }
    let text = command(
        &mut driver,
        &mut app,
        &home,
        1,
        "message.send",
        json!({"chatId":101,"text":"synthetic native acknowledgement","replyToMessageId":3,"topicId":7,"silent":true}),
    );
    assert_eq!(reply(&text, 1)["ok"], true);
    assert_eq!(reply(&text, 1)["result"]["content"]["kind"], "text");
    let refused = command(
        &mut driver,
        &mut app,
        &home,
        2,
        "message.send",
        json!({"chatId":101,"text":"fixture-reject"}),
    );
    assert_eq!(reply(&refused, 2)["error"], "synthetic send refused");
    let album = command(
        &mut driver,
        &mut app,
        &home,
        3,
        "message.sendFiles",
        json!({"chatId":101,"paths":[root.join("photo-a.png"),root.join("document.txt"),root.join("photo-b.png")],"caption":"mixed local groups","replyToMessageId":3,"silent":true}),
    );
    assert_eq!(reply(&album, 3)["ok"], true);
    assert_eq!(reply(&album, 3)["result"]["groups"], 2);
    let video = command(
        &mut driver,
        &mut app,
        &home,
        4,
        "message.sendFile",
        json!({"chatId":101,"path":source,"caption":"ordinary video"}),
    );
    assert_eq!(reply(&video, 4)["ok"], true);
    let content = &reply(&video, 4)["result"]["content"];
    assert_eq!(content["kind"], "video");
    assert_eq!(content["media"]["width"], 80);
    assert_eq!(content["media"]["height"], 60);
    let playable = content["media"]["file"]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(!playable.is_empty());
    assert_eq!(fs::read(&playable).unwrap(), source_bytes);
    let profile = command(
        &mut driver,
        &mut app,
        &home,
        5,
        "profile.setPhoto",
        json!({"path":root.join("selected.ppm")}),
    );
    assert_eq!(reply(&profile, 5)["ok"], true);
    assert!(!files(&runtime.join("omagram/rec")).iter().any(|p| {
        p.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("profile-")
    }));
    let note = command(
        &mut driver,
        &mut app,
        &home,
        6,
        "videonote.send",
        json!({"chatId":101,"path":source,"replyToMessageId":3,"silent":true}),
    );
    assert_eq!(reply(&note, 6)["ok"], true);
    assert_eq!(reply(&note, 6)["result"]["content"]["kind"], "videoNote");
    let note_path = reply(&note, 6)["result"]["content"]["media"]["file"]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(telebar_runtime::media::probe(&note_path)[1..], [480, 480]);
    assert!(
        driver.runtime.engine.accounts["default"]
            .state
            .retained
            .record
            .is_none()
    );
    assert_eq!(
        telebar_runtime::recording_checkpoint::Checkpoint::open(&root, &runtime, "default")
            .unwrap()
            .load()
            .unwrap()
            .map(|r| r.token),
        None
    );
    let mut updates = vec![];
    for _ in 0..3 {
        updates.extend(
            driver
                .tick_application(&mut app, &home, &mut |_| panic!("no startup"))
                .unwrap()
                .replies
                .into_iter()
                .map(|(_, f)| f),
        );
    }
    let all = text
        .iter()
        .chain(&album)
        .chain(&video)
        .chain(&note)
        .chain(&updates)
        .collect::<Vec<_>>();
    assert!(
        all.iter()
            .any(|f| f["event"] == "messageSent" && f["message"]["content"]["kind"] == "video")
    );
    assert!(
        all.iter()
            .any(|f| f["event"] == "messageSent" && f["message"]["content"]["kind"] == "videoNote")
    );
    for (id, key, expected) in [(10, "messages", 6), (11, "profile", 1), (12, "refused", 1)] {
        let mut action = Some(Action::Native {
            query: json!({"@type":"getOption","name":"telebarFixtureStats"}),
            answer: Answer::Field {
                wire: "count",
                native: key,
            },
        });
        let effects = driver
            .request_with_local(
                7,
                request(id, "chat.history", json!({"chatId":101})),
                &mut |_, _, _| Some(Ok(action.take().unwrap())),
            )
            .unwrap();
        assert!(action.is_none(), "actual test planner reached");
        let stats = collect(&mut driver, &mut app, &home, id, effects);
        assert_eq!(
            reply(&stats, id)["ok"],
            true,
            "fixture reply: {:?}",
            reply(&stats, id)
        );
        assert_eq!(
            reply(&stats, id)["result"]["count"],
            expected,
            "fixture actual dispatch count"
        );
    }
    let retained = files(&root.join(".local/share/omagram/sent"));
    assert_eq!(retained.len(), 5);
    driver.close(Duration::from_secs(2)).unwrap();
    drop(driver);
    telebar_runtime::upload_lease::recover_unsent(&home, ".local/share/omagram/sent").unwrap();
    assert_eq!(files(&root.join(".local/share/omagram/sent")), retained);
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert_eq!(fs::read(root.join("selected.ppm")).unwrap(), ppm);
    assert_eq!(
        fs::read(root.join("document.txt")).unwrap(),
        b"synthetic chosen document"
    );
    assert!(Path::new(&playable).exists() && Path::new(&note_path).exists());
}
