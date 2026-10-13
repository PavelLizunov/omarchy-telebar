//! Explicit bounded local hardware acceptance. Never constructs TDLib or sends.
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, sync::atomic::AtomicBool, time::Duration};
use telebar_runtime::{capture, commands::Action, launcher, media, playback, sound_playback};

#[test]
#[ignore = "explicit camera/microphone/output permission; local media only, no Telegram"]
fn fixed_release_inputs_capture_prepare_and_play_locally_without_native_clients() {
    assert_eq!(std::env::var("TELEBAR_PHYSICAL_MEDIA").as_deref(), Ok("1"));
    let root = std::env::temp_dir().join(format!("telebar-physical-media-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    // Full normal launcher context supplies the verified host runtime path.
    // session_environment alone intentionally omits HOME/XDG_RUNTIME_DIR.
    let environment = launcher::Context::discover().unwrap().environment;
    let context = capture::Context {
        home: root.clone(),
        runtime: root.clone(),
        environment: environment.clone(),
    };
    let cancel = AtomicBool::new(false);
    for kind in ["voice", "video"] {
        // Normal start, not start_with or a test-only lavfi input override.
        let mut recorder =
            capture::Capture::start(&context, "default".into(), 1, kind, 42).unwrap();
        let pid = recorder.pid();
        std::thread::sleep(Duration::from_secs(4));
        assert!(
            !recorder.poll(),
            "fixed physical recorder exited before stop: {kind}"
        );
        // Preparation stops capture; this has no Engine/native dispatch owner.
        let Action::PrepareRecorded { preparation, .. } = recorder
            .prepare_send(json!({"send":true}), root.clone(), json!({}))
            .unwrap()
        else {
            panic!("local capture checkpoint preparation failed: {kind}");
        };
        let Action::Recorded { content, input, .. } = preparation.run(&cancel).unwrap() else {
            panic!("local preparation failed: {kind}");
        };
        let field = if kind == "voice" {
            "voice_note"
        } else {
            "video_note"
        };
        let path = content[field][field]["path"].as_str().unwrap();
        let probe = media::probe(path);
        assert!((1..=6).contains(&probe[0]), "bounded local duration");
        assert!(fs::metadata(path).unwrap().len() > 100);
        if kind == "video" {
            assert_eq!(probe[1..], [480, 480]);
        } else {
            let start = playback::Start {
                selection: playback::Selection {
                    serial: 1,
                    chat: 42,
                    message: 1,
                    file: 1,
                    rate: 1.0,
                    environment: environment.clone(),
                },
                path: path.into(),
                roots: vec![root.to_str().unwrap().into()],
            };
            let (mut player, file) = start.spawn().unwrap();
            let playback_pid = player.pid();
            let until = std::time::Instant::now() + Duration::from_secs(6);
            let output = loop {
                if let Some(output) = player.poll().unwrap() {
                    break output;
                }
                assert!(std::time::Instant::now() < until, "bounded local playback");
                std::thread::sleep(Duration::from_millis(5));
            };
            assert!(output.ok());
            drop(file);
            assert!(!std::path::Path::new(&format!("/proc/{playback_pid}")).exists());
        }
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        let owned = path.to_owned();
        drop(input); // Never-submitted task-owned preparation may clean itself.
        assert!(!std::path::Path::new(&owned).exists());
        println!(
            "fixed physical {kind}: bounded capture/preparation/reaping PASS; native dispatches0"
        );
    }
    let Action::SoundReady(sound) = sound_playback::Job::prepare(
        root.clone(),
        123,
        &json!({"sounds":{"style":"pop"}}),
        environment,
    )
    .unwrap()
    .run(&cancel)
    .unwrap() else {
        panic!("local sound preparation");
    };
    let mut player = sound.spawn().unwrap();
    let until = std::time::Instant::now() + Duration::from_secs(5);
    let output = loop {
        if let Some(output) = player.poll().unwrap() {
            break output;
        }
        assert!(
            std::time::Instant::now() < until,
            "bounded local notification sound"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(output.ok());
    drop(sound);
    fs::remove_dir_all(root).unwrap();
    println!(
        "actual host ffplay/pw-cat output exited normally; audible/manual judgement pending; TDLib clients0/parameters0/messages0"
    );
}
