//! Actual bounded gzip/cache worker and synthetic native command boundaries.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use telebar_runtime::{
    commands::{Action, Answer},
    engine::{Account, Engine},
    jobs::Pool,
    process,
    state::State,
    sticker_cache::{self, Job},
    storage::Storage,
    wire::Request,
};
fn result(action: Action) -> Value {
    let Action::Reply(v) = action else {
        panic!("reply")
    };
    v
}
#[test]
fn cache_jobs_bound_inflation_and_only_clear_owned_stickers() {
    let root = std::env::temp_dir().join(format!("telebar-sticker-cache-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let store = Storage::open(&root).unwrap();
    let cancel = AtomicBool::new(false);
    let blob = process::run(
        "/usr/bin/gzip",
        &["-c"],
        &BTreeMap::new(),
        br#"{"v":"5.5","layers":[]}"#,
        Duration::from_secs(2),
        4096,
    )
    .unwrap();
    assert!(blob.ok());
    store
        .write("files/sticker.tgs", &blob.stdout, 0o600)
        .unwrap();
    store
        .write(".local/share/omagram/sent/keep", b"sent", 0o600)
        .unwrap();
    let roots = vec![root.join("files").to_str().unwrap().to_owned()];
    let source = root.join("files/sticker.tgs").to_str().unwrap().to_owned();
    let mut engine = Engine::new("default".into());
    engine.accounts.insert(
        "default".into(),
        Account {
            client: 1,
            generation: 1,
            ready: true,
            state: State::new(roots.clone()),
        },
    );
    let request =
        Request::parse(br#"{"id":1,"cmd":"sticker.lottie","args":{"fileId":3}}"#).unwrap();
    let effects = engine.request_using(7, request, Instant::now(), &mut |cmd, args, _| {
        sticker_cache::plan(cmd, args, root.clone()).unwrap()
    });
    assert_eq!(effects.native[0].query["@type"], "getFile");
    let effects=engine.receive(json!({"@type":"file","id":3,"@client_id":1,"@extra":effects.native[0].query["@extra"],"local":{"path":source,"is_downloading_completed":true}}));
    assert_eq!(effects.work.len(), 1);
    assert!(effects.replies.is_empty());
    let mut pool = Pool::default();
    for (token, work) in effects.work {
        pool.start(token, work).unwrap();
    }
    let until = Instant::now() + Duration::from_secs(3);
    let mut path = String::new();
    while Instant::now() < until {
        for done in pool.poll(&|_| true) {
            let effects = engine.job_complete(done.token, done.result, Instant::now());
            path = effects.replies[0].1["result"]["path"]
                .as_str()
                .unwrap()
                .into();
        }
        if !path.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(path.starts_with(root.join(".cache/omagram/lottie").to_str().unwrap()));
    assert_eq!(fs::read(&path).unwrap(), br#"{"v":"5.5","layers":[]}"#);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let stats = result(
        Job::Stats {
            home: root.clone(),
            native: json!({"filesSize":123}),
        }
        .run(&cancel)
        .unwrap(),
    );
    assert_eq!(
        stats["stickerCacheSize"],
        br#"{"v":"5.5","layers":[]}"#.len()
    );
    assert_eq!(stats["filesSize"], 123);
    symlink(
        root.join(".local/share/omagram/sent/keep"),
        root.join(".cache/omagram/lottie/foreign.json"),
    )
    .unwrap();
    store
        .write("files/bad.tgs", b"\x1f\x8bgarbage", 0o600)
        .unwrap();
    let bad = result(
        Job::Lottie {
            home: root.clone(),
            path: root.join("files/bad.tgs").to_str().unwrap().into(),
            roots: roots.clone(),
        }
        .run(&cancel)
        .unwrap(),
    );
    assert_eq!(bad, json!({"path":""}));
    store
        .write(
            "files/array.tgs",
            &process::run(
                "/usr/bin/gzip",
                &["-c"],
                &BTreeMap::new(),
                b"[]",
                Duration::from_secs(2),
                4096,
            )
            .unwrap()
            .stdout,
            0o600,
        )
        .unwrap();
    assert_eq!(
        result(
            Job::Lottie {
                home: root.clone(),
                path: root.join("files/array.tgs").to_str().unwrap().into(),
                roots
            }
            .run(&cancel)
            .unwrap()
        ),
        json!({"path":""})
    );
    // Highly compressible oversized input must stop before cache publication.
    let bomb = process::run(
        "/usr/bin/gzip",
        &["-c"],
        &BTreeMap::new(),
        &vec![b' '; 17 * 1024 * 1024],
        Duration::from_secs(3),
        1024 * 1024,
    )
    .unwrap();
    assert!(bomb.ok());
    store.write("files/bomb.tgs", &bomb.stdout, 0o600).unwrap();
    assert_eq!(
        result(
            Job::Lottie {
                home: root.clone(),
                path: root.join("files/bomb.tgs").to_str().unwrap().into(),
                roots: vec![root.join("files").to_str().unwrap().into()]
            }
            .run(&cancel)
            .unwrap()
        ),
        json!({"path":""})
    );
    assert_eq!(store.files(".cache/omagram/lottie", 8).unwrap().len(), 1);
    let cancelled = AtomicBool::new(true);
    assert!(
        Job::Clear {
            home: root.clone(),
            remaining: 0
        }
        .run(&cancelled)
        .is_err()
    );
    assert!(fs::metadata(&path).is_ok());
    let clear = result(
        Job::Clear {
            home: root.clone(),
            remaining: 42,
        }
        .run(&cancel)
        .unwrap(),
    );
    assert_eq!(clear, json!({"remaining":42}));
    assert!(
        fs::symlink_metadata(root.join(".cache/omagram/lottie/foreign.json"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(fs::metadata(&path).is_err());
    assert_eq!(
        fs::read(root.join(".local/share/omagram/sent/keep")).unwrap(),
        b"sent"
    );
    let stats = sticker_cache::plan("storage.stats", &json!({}), root.clone())
        .unwrap()
        .unwrap();
    let Action::Native {
        answer: Answer::Cache(answer),
        ..
    } = stats
    else {
        panic!("cache")
    };
    let Action::Work(_) = answer
        .complete(&json!({"files_size":-1}), &State::default())
        .unwrap()
    else {
        panic!("worker")
    };
    fs::remove_dir_all(root).unwrap();
}
