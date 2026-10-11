//! Synthetic media only: no microphone/camera, working messages or profiles.
use std::{collections::BTreeMap, fs, os::unix::fs::PermissionsExt, time::Duration};
#[test]
fn real_ffmpeg_prepares_synthetic_voice_and_round_video() {
    let root = std::env::temp_dir().join(format!("telebar-media-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let voice = root.join("voice.ogg");
    let source = root.join("source.mp4");
    let note = root.join("note.mp4");
    for args in [
        vec![
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000",
            "-t",
            "1",
            "-ac",
            "1",
            "-c:a",
            "libopus",
            voice.to_str().unwrap(),
        ],
        vec![
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=80x60:r=10",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=330:sample_rate=48000",
            "-t",
            "1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            source.to_str().unwrap(),
        ],
    ] {
        let mut command = vec!["-hide_banner", "-loglevel", "error", "-nostdin"];
        command.extend(args);
        let out = telebar_runtime::process::run(
            "/usr/bin/ffmpeg",
            &command,
            &BTreeMap::new(),
            &[],
            Duration::from_secs(10),
            64 * 1024,
        )
        .unwrap();
        assert!(out.ok(), "synthetic fixture failed");
    }
    let (seconds, waveform) =
        telebar_runtime::media::prepare_voice(voice.to_str().unwrap()).unwrap();
    assert_eq!(seconds, 1);
    assert!(!waveform.is_empty());
    let seconds = telebar_runtime::media::prepare_video_note(
        source.to_str().unwrap(),
        note.to_str().unwrap(),
    )
    .unwrap();
    assert!((0.9..=1.2).contains(&seconds));
    assert_eq!(
        telebar_runtime::media::probe(note.to_str().unwrap()),
        [1, 480, 480]
    );
    let job = || telebar_runtime::sticker_cache::Job::Note {
        home: root.clone(),
        path: note.to_str().unwrap().into(),
        roots: vec![root.to_str().unwrap().into()],
    };
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let telebar_runtime::commands::Action::Reply(animation) = job().run(&cancel).unwrap() else {
        panic!("animation")
    };
    let path = animation["path"].as_str().unwrap();
    assert_eq!(animation["fps"], 20);
    assert_eq!(telebar_runtime::media::probe(path)[1..], [240, 240]);
    assert_eq!(fs::read(path).unwrap().get(8..12), Some(&b"WEBP"[..]));
    let telebar_runtime::commands::Action::Reply(cached) = job().run(&cancel).unwrap() else {
        panic!("cache")
    };
    assert_eq!(cached, animation);
    let entries = fs::read_dir(root.join(".cache/omagram/notes"))
        .unwrap()
        .collect::<std::io::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        entries.len(),
        1,
        "owned source/partial containers must be removed"
    );
    assert!(
        job()
            .run(&std::sync::atomic::AtomicBool::new(true))
            .is_err()
    );
    fs::remove_dir_all(root).unwrap();
}
