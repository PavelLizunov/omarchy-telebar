//! Synthetic profile conversion, actual helper, native lease/outcome boundaries.
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use telebar_runtime::{
    commands::{Action, Answer},
    engine::{Account, Engine},
    profile_photo::Job,
    state::State,
    wire::Request,
};
fn converted(root: &std::path::Path) -> Action {
    let source = telebar_runtime::uploads::admit(&json!(root.join("synthetic.ppm")), &[]).unwrap();
    Job {
        source,
        runtime: root.into(),
    }
    .run(&AtomicBool::new(false))
    .unwrap()
}
#[test]
fn profile_photo_conversion_is_private_and_unknown_native_outcome_keeps_input() {
    let root = std::env::temp_dir().join(format!("telebar-profile-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let mut ppm = b"P6\n12 8\n255\n".to_vec();
    ppm.extend([128, 64, 32].repeat(12 * 8));
    fs::write(root.join("synthetic.ppm"), ppm).unwrap();
    let source = telebar_runtime::uploads::admit(&json!(root.join("synthetic.ppm")), &[]).unwrap();
    assert!(
        Job {
            source,
            runtime: root.clone()
        }
        .run(&AtomicBool::new(true))
        .is_err()
    );
    assert!(!root.join("omagram").exists());
    let Action::Native {
        answer: Answer::ProfilePhoto(lease),
        query,
    } = converted(&root)
    else {
        panic!("native")
    };
    let path = lease.path();
    assert_eq!(query["photo"]["@type"], "inputChatPhotoStatic");
    let bytes = fs::read(&path).unwrap();
    assert!(bytes.starts_with(&[255, 216]));
    assert!(bytes.ends_with(&[255, 217]));
    assert_eq!(
        telebar_runtime::media::probe(path.to_str().unwrap())[1..],
        [8, 8]
    );
    drop(lease);
    assert!(!path.exists());
    for (dispatch, event, keep) in [
        (false, None, false),
        (true, Some(json!({"@type":"ok"})), false),
        (
            true,
            Some(json!({"@type":"error","code":400,"message":"rejected"})),
            false,
        ),
        (
            true,
            Some(json!({"@type":"error","code":500,"message":"unknown"})),
            true,
        ),
        (true, None, true),
    ] {
        let mut engine = Engine::new("default".into());
        engine.accounts.insert(
            "default".into(),
            Account {
                client: 41,
                generation: 1,
                ready: true,
                state: State::default(),
            },
        );
        let mut action = Some(converted(&root));
        let path = match action.as_ref().unwrap() {
            Action::Native {
                answer: Answer::ProfilePhoto(l),
                ..
            } => l.path(),
            _ => panic!("photo"),
        };
        let now = Instant::now();
        let request = Request::parse(br#"{"id":1,"cmd":"profile.setPhoto","args":{}}"#).unwrap();
        let admitted =
            engine.request_using(7, request, now, &mut |_, _, _| Ok(action.take().unwrap()));
        assert_eq!(admitted.native.len(), 1);
        let query = &admitted.native[0].query;
        if dispatch {
            engine.native_dispatched(query).unwrap();
        }
        if let Some(mut e) = event {
            e["@extra"] = query["@extra"].clone();
            e["@client_id"] = json!(41);
            let done = engine.receive_at(e, now, &mut |_| None);
            assert_eq!(done.replies.len(), 1);
        } else {
            engine.expire(now + Duration::from_secs(181));
        }
        assert_eq!(path.exists(), keep, "dispatch={dispatch}, keep={keep}");
    }
    // A selected network playlist must not make ffmpeg fetch external resources.
    fs::write(
        root.join("playlist.m3u"),
        b"#EXTM3U\nhttps://example.test/private\n",
    )
    .unwrap();
    let source = telebar_runtime::uploads::admit(&json!(root.join("playlist.m3u")), &[]).unwrap();
    assert!(
        Job {
            source,
            runtime: PathBuf::from(&root)
        }
        .run(&AtomicBool::new(false))
        .is_err()
    );
    fs::remove_dir_all(root).unwrap();
}
