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
    // Actual protected export/save and cache Driver routes use only explicitly
    // injected local file metadata. The synthetic ABI never reads the paths.
    let configure = |driver: &mut Driver, app: &mut Application, id: u64, path: &Path| {
        let mut action = Some(Action::Native {
            query: json!({"@type":"getOption","name":"telebarFixtureInput","fixture_file":{"@type":"file","id":100,"size":1,"local":{"@type":"localFile","path":path,"is_downloading_completed":true,"downloaded_size":1}}}),
            answer: Answer::Empty,
        });
        let effects = driver
            .request_with_local(
                7,
                request(id, "chat.history", json!({"chatId":101})),
                &mut |_, _, _| Some(Ok(action.take().unwrap())),
            )
            .unwrap();
        let frames = collect(driver, app, &home, id, effects);
        assert_eq!(reply(&frames, id)["ok"], true);
    };
    home.write(
        ".local/share/omagram/files/export.txt",
        b"synthetic export bytes",
        0o600,
    )
    .unwrap();
    let export = root.join(".local/share/omagram/files/export.txt");
    configure(&mut driver, &mut app, 40, &export);
    for rid in [41, 42] {
        let frames = command(
            &mut driver,
            &mut app,
            &home,
            rid,
            "file.save",
            json!({"chatId":101,"messageId":77,"fileId":100,"fileName":"saved.txt"}),
        );
        assert_eq!(reply(&frames, rid)["ok"], true);
        let target = reply(&frames, rid)["result"]["path"].as_str().unwrap();
        assert_eq!(fs::read(target).unwrap(), b"synthetic export bytes");
    }
    assert_eq!(fs::read(&export).unwrap(), b"synthetic export bytes");
    home.ensure_dir(".local/share/applications").unwrap();
    home.write(".local/share/applications/inert.desktop",b"[Desktop Entry]\nType=Application\nName=Inert read\nExec=/usr/bin/cat %f\nMimeType=text/plain;\n",0o600).unwrap();
    home.write(
        ".config/mimeapps.list",
        b"[Default Applications]\ntext/plain=inert.desktop\n",
        0o600,
    )
    .unwrap();
    app.clipboard_context.as_mut().unwrap().environment = std::collections::BTreeMap::from([
        ("HOME".into(), root.to_str().unwrap().into()),
        (
            "XDG_CONFIG_HOME".into(),
            root.join(".config").to_str().unwrap().into(),
        ),
        (
            "XDG_DATA_HOME".into(),
            root.join(".local/share").to_str().unwrap().into(),
        ),
        (
            "XDG_DATA_DIRS".into(),
            root.join(".local/share").to_str().unwrap().into(),
        ),
        ("XDG_CURRENT_DESKTOP".into(), "generic".into()),
        ("DE".into(), "generic".into()),
        ("WAYLAND_DISPLAY".into(), "fixture-no-socket".into()),
    ]);
    let opened = command(
        &mut driver,
        &mut app,
        &home,
        43,
        "file.open",
        json!({"fileId":100}),
    );
    assert_eq!(reply(&opened, 43)["ok"], true);
    let until = Instant::now() + Duration::from_secs(3);
    while app.diagnostic_runtime["children"].as_u64().unwrap_or(0) > 0 {
        assert!(Instant::now() < until);
        driver
            .tick_application(&mut app, &home, &mut |_| panic!("no startup"))
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    let compressed = telebar_runtime::process::run(
        "/usr/bin/gzip",
        &["-c"],
        &Default::default(),
        br#"{"v":"5.5","layers":[]}"#,
        Duration::from_secs(2),
        4096,
    )
    .unwrap();
    assert!(compressed.ok());
    home.write(
        ".local/share/omagram/files/sticker.tgs",
        &compressed.stdout,
        0o600,
    )
    .unwrap();
    let sticker = root.join(".local/share/omagram/files/sticker.tgs");
    configure(&mut driver, &mut app, 44, &sticker);
    let inflated = command(
        &mut driver,
        &mut app,
        &home,
        45,
        "sticker.lottie",
        json!({"fileId":100}),
    );
    assert_eq!(reply(&inflated, 45)["ok"], true);
    let cached = reply(&inflated, 45)["result"]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(fs::read(&cached).unwrap(), br#"{"v":"5.5","layers":[]}"#);
    let stats = command(&mut driver, &mut app, &home, 46, "storage.stats", json!({}));
    assert_eq!(reply(&stats, 46)["ok"], true);
    assert_eq!(reply(&stats, 46)["result"]["filesSize"], 19);
    assert_eq!(reply(&stats, 46)["result"]["stickerCacheSize"], 23);
    configure(&mut driver, &mut app, 47, Path::new(&note_path));
    let animated = command(
        &mut driver,
        &mut app,
        &home,
        48,
        "videonote.animation",
        json!({"fileId":100}),
    );
    assert_eq!(reply(&animated, 48)["ok"], true);
    let animation = reply(&animated, 48)["result"]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(reply(&animated, 48)["result"]["fps"], 20);
    assert_eq!(telebar_runtime::media::probe(&animation)[1..], [240, 240]);
    let clear = command(&mut driver, &mut app, &home, 49, "storage.clear", json!({}));
    assert_eq!(reply(&clear, 49)["ok"], true);
    assert_eq!(reply(&clear, 49)["result"]["remaining"], 42);
    assert!(!Path::new(&cached).exists());
    assert!(Path::new(&animation).exists() && Path::new(&note_path).exists());
    let listed = command(&mut driver, &mut app, &home, 50, "account.list", json!({}));
    assert_eq!(reply(&listed, 50)["ok"], true);
    assert_eq!(reply(&listed, 50)["result"]["active"], "default");
    assert_eq!(
        reply(&listed, 50)["result"]["accounts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let sound = command(
        &mut driver,
        &mut app,
        &home,
        51,
        "sounds.play",
        json!({"userId":900}),
    );
    assert_eq!(reply(&sound, 51)["ok"], true); // Actual private audio output has its separate consumer; no host PipeWire here.
    let status = command(&mut driver, &mut app, &home, 52, "bridge.status", json!({}));
    assert_eq!(reply(&status, 52)["ok"], true);
    assert_eq!(reply(&status, 52)["result"]["running"], false);
    assert_eq!(reply(&status, 52)["result"]["enabled"], false);
    assert_eq!(reply(&status, 52)["result"]["pid"], 0);
    let invalid = command(
        &mut driver,
        &mut app,
        &home,
        53,
        "bridge.set",
        json!({"enabled":"yes"}),
    );
    assert_eq!(
        reply(&invalid, 53)["error"],
        "enabled must be true or false"
    );
    let unavailable = command(&mut driver, &mut app, &home, 54, "bridge.test", json!({}));
    assert_eq!(
        reply(&unavailable, 54)["error"],
        "the bundled bridge is not available"
    );
    app.bridge.metadata = json!({"default":{"proxy_id":7,"previous_id":4}});
    let saved = command(&mut driver, &mut app, &home, 55, "bridge.status", json!({}));
    assert_eq!(reply(&saved, 55)["ok"], true);
    assert_eq!(reply(&saved, 55)["result"]["enabled"], true);
    assert_eq!(reply(&saved, 55)["result"]["running"], false);
    assert_eq!(app.bridge.metadata["default"]["previous_id"], 4);
    assert!(
        !root.join(".config/omagram/bridge.json").exists(),
        "status never persists metadata"
    );
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

#[test]
#[ignore = "requires reviewed private bridge namespace and explicit synthetic ping ABI"]
fn actual_bridge_test_route_owns_one_proxy_and_preserves_metadata() {
    assert_eq!(std::env::var("TELEBAR_ISOLATED_BRIDGE").as_deref(), Ok("1"));
    let root = Path::new("/tmp/telebar-bridge-driver");
    fs::create_dir(root).unwrap();
    let home = Storage::open(root).unwrap();
    let mut app = Application::load(&home).unwrap();
    app.bridge.context = Some(telebar_runtime::bridge::Context {
        binary: std::env::var_os("TELEBAR_TEST_PROXY_BINARY")
            .unwrap()
            .into(),
        environment: Default::default(),
    });
    let library = std::env::var_os("TELEBAR_TEST_INERT_ABI").unwrap();
    let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
    driver
        .register("default".into(), "Bridge fixture".into(), vec![])
        .unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    while !driver.runtime.engine.accounts["default"].ready {
        assert!(Instant::now() < until);
        driver
            .tick_application(&mut app, &home, &mut |_| panic!("no parameters"))
            .unwrap();
    }
    let answer = command(&mut driver, &mut app, &home, 70, "bridge.test", json!({}));
    assert_eq!(reply(&answer, 70)["ok"], true);
    assert_eq!(reply(&answer, 70)["result"]["seconds"], 0.125);
    let pid = app.bridge.running.as_ref().unwrap().helper.pid();
    let status = command(&mut driver, &mut app, &home, 71, "bridge.status", json!({}));
    assert_eq!(reply(&status, 71)["ok"], true);
    assert_eq!(reply(&status, 71)["result"]["pid"], pid);
    assert_eq!(reply(&status, 71)["result"]["running"], true);
    assert_eq!(reply(&status, 71)["result"]["enabled"], false);
    assert!(app.bridge.metadata.as_object().unwrap().is_empty());
    assert!(!root.join(".config/omagram/bridge.json").exists());
    app.bridge.busy.insert("default".into());
    let refused = command(&mut driver, &mut app, &home, 72, "bridge.test", json!({}));
    assert_eq!(
        reply(&refused, 72)["error"],
        "a bridge operation is already running"
    );
    assert_eq!(app.bridge.running.as_ref().unwrap().helper.pid(), pid);
    app.bridge.busy.clear();
    let mut action = Some(Action::Native {
        query: json!({"@type":"getOption","name":"telebarFixturePingRefuse"}),
        answer: Answer::Empty,
    });
    let effects = driver
        .request_with_local(
            7,
            request(73, "chat.history", json!({"chatId":101})),
            &mut |_, _, _| Some(Ok(action.take().unwrap())),
        )
        .unwrap();
    let configured = collect(&mut driver, &mut app, &home, 73, effects);
    assert_eq!(reply(&configured, 73)["ok"], true);
    let refused = command(&mut driver, &mut app, &home, 74, "bridge.test", json!({}));
    assert_eq!(reply(&refused, 74)["error"], "synthetic ping refused");
    assert!(app.bridge.busy.is_empty());
    assert_eq!(app.bridge.running.as_ref().unwrap().helper.pid(), pid);
    let again = command(&mut driver, &mut app, &home, 75, "bridge.test", json!({}));
    assert_eq!(reply(&again, 75)["ok"], true);
    assert_eq!(app.bridge.running.as_ref().unwrap().helper.pid(), pid);
    assert!(!root.join(".config/omagram/bridge.json").exists());
    app.bridge.running = None;
    assert_capture_reaped(pid);
    driver.close(Duration::from_secs(2)).unwrap();
    drop(driver);
    println!(
        "normal bridge.test synthetic success/refusal; one actual Rust proxy, metadata unchanged, child reaped; no external network PASS"
    );
    fs::remove_dir_all(root).unwrap();
}

struct VoiceGraph {
    environment: std::collections::BTreeMap<String, String>,
    _owners: Vec<telebar_runtime::process::Managed>,
}
impl VoiceGraph {
    fn start(root: &Path, runtime: &Path) -> Self {
        let environment = std::collections::BTreeMap::from([
            ("HOME".into(), root.to_str().unwrap().into()),
            ("XDG_RUNTIME_DIR".into(), runtime.to_str().unwrap().into()),
            ("XDG_CONFIG_HOME".into(), root.to_str().unwrap().into()),
            (
                "PULSE_SERVER".into(),
                format!("unix:{}/pulse/native", runtime.display()),
            ),
        ]);
        let exposed = PathBuf::from(std::env::var_os("TELEBAR_PRIVATE_ROOT").unwrap());
        let mut owners = vec![];
        for (tool, config, socket) in [
            ("/usr/bin/pipewire", "pipewire-voice.conf", "pipewire-0"),
            (
                "/usr/bin/pipewire-pulse",
                "pulse-private.conf",
                "pulse/native",
            ),
        ] {
            let config = exposed.join(config);
            let mut owner = telebar_runtime::process::Managed::start(
                tool,
                &["-c", config.to_str().unwrap()],
                &environment,
                &[],
                Duration::from_secs(30),
                65536,
            )
            .unwrap();
            let until = Instant::now() + Duration::from_secs(3);
            while !runtime.join(socket).exists() {
                assert!(Instant::now() < until, "private audio socket readiness");
                if let Some(output) = owner.poll().unwrap() {
                    panic!(
                        "private audio ended: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            owners.push(owner);
        }
        Self {
            environment,
            _owners: owners,
        }
    }
    fn run(&self, tool: &str, args: &[&str]) -> Vec<u8> {
        let output = telebar_runtime::process::run(
            tool,
            args,
            &self.environment,
            &[],
            Duration::from_secs(2),
            512 * 1024,
        )
        .unwrap();
        assert!(
            output.ok(),
            "private graph helper: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
    fn link_capture(&self) -> u32 {
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            assert!(
                Instant::now() < until,
                "private Pulse capture node readiness"
            );
            let dump: Value = serde_json::from_slice(&self.run("/usr/bin/pw-dump", &[])).unwrap();
            let node = dump.as_array().unwrap().iter().find(|node| {
                node["type"] == "PipeWire:Interface:Node"
                    && node["info"]["props"]["application.process.binary"] == "ffmpeg"
                    && node["info"]["props"]["media.class"] == "Stream/Input/Audio"
            });
            if let Some(node) = node {
                assert_eq!(
                    String::from_utf8(self.run("/usr/bin/pactl", &["get-default-source"]))
                        .unwrap()
                        .trim(),
                    "telebar-source",
                );
                let id = node["id"].as_u64().unwrap().to_string();
                let name = node["info"]["props"]["node.name"].as_str().unwrap();
                let pid = node["info"]["props"]["application.process.id"]
                    .as_u64()
                    .or_else(|| {
                        node["info"]["props"]["application.process.id"]
                            .as_str()?
                            .parse()
                            .ok()
                    })
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap();
                self.run("/usr/bin/pw-cli", &["set-param", &id, "PortConfig", "{ direction = Input mode = dsp format = { mediaType = audio mediaSubtype = raw format = F32P rate = 48000 channels = 1 position = [ MONO ] } }"]);
                let ports = String::from_utf8(self.run("/usr/bin/pw-link", &["-i"])).unwrap();
                if let Some(input) = ports
                    .lines()
                    .map(str::trim)
                    .find(|s| s.starts_with(&format!("{name}:")))
                {
                    self.run("/usr/bin/pw-link", &["telebar-source:capture_MONO", input]);
                    return pid;
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
fn assert_capture_reaped(pid: u32) {
    let mut status = 0;
    // SAFETY: this is the exact observed test-owned ffmpeg child. ECHILD checks
    // actual reaping; a /proc existence check alone would be hidden by Landlock.
    assert_eq!(
        unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}
#[test]
#[ignore = "requires reviewed private Pulse graph and explicit synthetic send ABI"]
fn actual_voice_driver_uses_only_the_private_synthetic_pulse_source() {
    assert_eq!(std::env::var("TELEBAR_PRIVATE_VOICE").as_deref(), Ok("1"));
    for family in [libc::AF_INET, libc::AF_INET6] {
        // SAFETY: inert negative-control socket is never connected.
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
    let root = PathBuf::from(std::env::var_os("TELEBAR_PRIVATE_ROOT").unwrap()).join("voice-home");
    fs::create_dir(&root).unwrap();
    let home = Storage::open(&root).unwrap();
    home.ensure_dir("runtime").unwrap();
    let runtime = root.join("runtime");
    let graph = VoiceGraph::start(&root, &runtime);
    let sources =
        String::from_utf8(graph.run("/usr/bin/pactl", &["list", "short", "sources"])).unwrap();
    assert!(sources.contains("telebar-source"));
    assert_eq!(sources.lines().count(), 1, "no host/device audio sources");
    graph.run("/usr/bin/pactl", &["set-default-source", "telebar-source"]);
    let session = telebar_runtime::session::Context::new(
        root.clone(),
        runtime.clone(),
        graph.environment.clone(),
        "en".into(),
    )
    .unwrap();
    let mut app = Application::load(&home).unwrap();
    session.configure_application(&mut app).unwrap();
    let library = std::env::var_os("TELEBAR_TEST_INERT_ABI").unwrap();
    let mut driver = Driver::open(Path::new(&library), "default".into(), "en".into()).unwrap();
    driver.capture_context = Some(telebar_runtime::capture::Context {
        home: root.clone(),
        runtime: runtime.clone(),
        environment: session.environment.clone(),
    });
    let roots =
        telebar_runtime::paths::AccountPaths::new(&root.join(".local/share/omagram"), "default")
            .unwrap()
            .media_roots(&runtime);
    driver
        .register("default".into(), "Private voice".into(), roots)
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
            .tick_application(&mut app, &home, &mut |_| panic!("no parameters"))
            .unwrap();
    }
    let start = command(
        &mut driver,
        &mut app,
        &home,
        60,
        "voice.start",
        json!({"chatId":101}),
    );
    assert_eq!(reply(&start, 60)["ok"], true);
    assert!(
        start
            .iter()
            .any(|f| f["event"] == "recording" && f["state"] == "voice" && f["chatId"] == 101)
    );
    let pid = graph.link_capture();
    let raw = files(&root.join(".local/share/omagram/sent"));
    assert_eq!(raw.len(), 1);
    let duplicate = command(
        &mut driver,
        &mut app,
        &home,
        61,
        "voice.start",
        json!({"chatId":101}),
    );
    assert_eq!(
        reply(&duplicate, 61)["error"],
        "a voice or video message is already being recorded"
    );
    assert_eq!(files(&root.join(".local/share/omagram/sent")), raw);
    driver
        .register("other".into(), "Other synthetic".into(), vec![])
        .unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    while !driver.runtime.engine.accounts["other"].ready {
        assert!(Instant::now() < until);
        driver
            .tick_application(&mut app, &home, &mut |_| panic!("no parameters"))
            .unwrap();
    }
    let wrong = Request::parse(
        json!({"id":67,"cmd":"voice.stop","account":"other","args":{"send":false}})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    let effects = driver
        .request_application(&mut app, &home, 7, wrong)
        .unwrap();
    let frames = collect(&mut driver, &mut app, &home, 67, effects);
    assert_eq!(
        reply(&frames, 67)["error"],
        "the recording belongs to another account"
    );
    assert_eq!(files(&root.join(".local/share/omagram/sent")), raw);
    std::thread::sleep(Duration::from_millis(1600));
    let sent = command(
        &mut driver,
        &mut app,
        &home,
        62,
        "voice.stop",
        json!({"send":true,"replyToMessageId":3,"silent":true}),
    );
    assert_eq!(reply(&sent, 62)["ok"], true, "{sent:?}");
    let content = &reply(&sent, 62)["result"]["content"];
    assert_eq!(content["kind"], "voice");
    assert!(content["media"]["duration"].as_u64().unwrap() >= 1);
    assert!(!content["media"]["waveform"].as_array().unwrap().is_empty());
    let playback = content["media"]["file"]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    let bytes = fs::read(&playback).unwrap();
    assert!(bytes.starts_with(b"OggS") && bytes.len() > 100);
    let info = telebar_runtime::process::run(
        "/usr/bin/ffprobe",
        &[
            "-v",
            "error",
            "-show_entries",
            "stream=codec_name,channels,sample_rate:format=duration",
            "-of",
            "json",
            &playback,
        ],
        &Default::default(),
        &[],
        Duration::from_secs(2),
        4096,
    )
    .unwrap();
    assert!(info.ok());
    let info: Value = serde_json::from_slice(&info.stdout).unwrap();
    assert_eq!(info["streams"][0]["codec_name"], "opus");
    assert_eq!(info["streams"][0]["channels"], 1);
    assert_eq!(info["streams"][0]["sample_rate"], "48000");
    assert!(
        info["format"]["duration"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap()
            >= 1.0
    );
    assert_capture_reaped(pid);
    assert!(
        driver.runtime.engine.accounts["default"]
            .state
            .retained
            .record
            .is_none()
    );
    assert!(
        telebar_runtime::recording_checkpoint::Checkpoint::open(&root, &runtime, "default")
            .unwrap()
            .load()
            .unwrap()
            .is_none()
    );
    let mut stats = Some(Action::Native {
        query: json!({"@type":"getOption","name":"telebarFixtureStats"}),
        answer: Answer::Field {
            wire: "count",
            native: "messages",
        },
    });
    let effects = driver
        .request_with_local(
            7,
            request(68, "chat.history", json!({"chatId":101})),
            &mut |_, _, _| Some(Ok(stats.take().unwrap())),
        )
        .unwrap();
    assert!(stats.is_none());
    let frames = collect(&mut driver, &mut app, &home, 68, effects);
    assert_eq!(reply(&frames, 68)["ok"], true);
    assert_eq!(
        reply(&frames, 68)["result"]["count"],
        1,
        "one synthetic send only"
    );
    let before = files(&root.join(".local/share/omagram/sent"));
    let start = command(
        &mut driver,
        &mut app,
        &home,
        63,
        "voice.start",
        json!({"chatId":101}),
    );
    assert_eq!(reply(&start, 63)["ok"], true);
    let pid = graph.link_capture();
    let owned = files(&root.join(".local/share/omagram/sent"))
        .into_iter()
        .find(|p| !before.contains(p))
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let stopped = command(
        &mut driver,
        &mut app,
        &home,
        64,
        "voice.stop",
        json!({"send":false}),
    );
    assert_eq!(reply(&stopped, 64)["ok"], true);
    assert!(
        stopped
            .iter()
            .any(|f| f["event"] == "recording" && f["state"] == "idle")
    );
    assert_capture_reaped(pid);
    assert!(!owned.exists());
    assert_eq!(files(&root.join(".local/share/omagram/sent")), before);
    let missing = command(
        &mut driver,
        &mut app,
        &home,
        65,
        "voice.stop",
        json!({"send":false}),
    );
    assert_eq!(
        reply(&missing, 65)["error"],
        "no voice message is being recorded"
    );
    let start = command(
        &mut driver,
        &mut app,
        &home,
        66,
        "voice.start",
        json!({"chatId":101}),
    );
    assert_eq!(reply(&start, 66)["ok"], true);
    let pid = graph.link_capture();
    let owned = files(&root.join(".local/share/omagram/sent"))
        .into_iter()
        .find(|p| !before.contains(p))
        .unwrap();
    driver.close(Duration::from_secs(2)).unwrap();
    assert_capture_reaped(pid);
    assert!(!owned.exists());
    drop(driver);
    telebar_runtime::upload_lease::recover_unsent(&home, ".local/share/omagram/sent").unwrap();
    assert_eq!(fs::read(&playback).unwrap(), bytes);
    assert!(
        raw[0].exists(),
        "sent raw recording remains conservative retention"
    );
    let audio_pids = graph
        ._owners
        .iter()
        .map(|owner| owner.pid())
        .collect::<Vec<_>>();
    drop(graph);
    for pid in audio_pids {
        assert_capture_reaped(pid);
    }
    println!(
        "private Pulse source; normal voice.start/stop; Opus/waveform; explicit stop/shutdown reap; synthetic native send; retained playback PASS"
    );
}
