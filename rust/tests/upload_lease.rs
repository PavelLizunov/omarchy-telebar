//! Snapshot cleanup happens only before dispatch or on certain rejection.
use std::{fs, os::unix::fs::PermissionsExt};
use telebar_runtime::upload_lease::Lease;
#[test]
fn stale_input_is_removed_but_submitted_or_uncertain_input_survives() {
    let root = std::env::temp_dir().join(format!("telebar-upload-lease-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    for (name, submitted, code, kept) in [
        ("stale", false, 0, false),
        ("reject", true, 400, false),
        ("unknown", true, 408, true),
        ("server", true, 500, true),
        ("accepted", true, 0, true),
    ] {
        let directory = root.join(name);
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("input"), b"synthetic").unwrap();
        let mut lease = Lease::new(root.clone());
        lease.add(format!("{name}/input"));
        if submitted {
            lease.submitted();
        }
        if code != 0 {
            lease.rejected(code);
        }
        drop(lease);
        assert_eq!(directory.join("input").exists(), kept, "{name}");
    }
    // Real Engine expiry is not native cancellation. Unsent generated input
    // cleans on admission loss; marking dispatched before peer retirement keeps it.
    for dispatched in [false, true] {
        let name = if dispatched {
            "engine-submitted"
        } else {
            "engine-stale"
        };
        let directory = root.join(name);
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("input"), b"synthetic").unwrap();
        let mut lease = Lease::new(root.clone());
        lease.add(format!("{name}/input"));
        let mut engine = telebar_runtime::engine::Engine::new("default".into());
        engine.accounts.insert(
            "default".into(),
            telebar_runtime::engine::Account {
                client: 41,
                generation: 1,
                state: Default::default(),
                ready: true,
            },
        );
        let request = telebar_runtime::wire::Request::parse(
            b"{\"id\":1,\"cmd\":\"message.sendFile\",\"args\":{}}",
        )
        .unwrap();
        let mut lease = Some(lease);
        let effects =
            engine.request_using(7, request, std::time::Instant::now(), &mut |_, _, _| {
                Ok(telebar_runtime::commands::Action::Native {
                    query: serde_json::json!({"@type":"sendMessage"}),
                    answer: telebar_runtime::commands::Answer::Upload(lease.take().unwrap(), false),
                })
            });
        if dispatched {
            engine.native_dispatched(&effects.native[0].query);
        }
        engine.retire_peer(7);
        assert_eq!(directory.join("input").exists(), dispatched);
    }
    // Retained recordings own the same lease through admission and explicit
    // outcomes; stale worker completions drop only generated unsubmitted files.
    for stale in [false, true] {
        let name = if stale {
            "recording-stale"
        } else {
            "recording-owned"
        };
        let directory = root.join(name);
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("input"), b"synthetic").unwrap();
        let mut input = Lease::new(root.clone());
        input.add(format!("{name}/input"));
        let mut engine = telebar_runtime::engine::Engine::new("default".into());
        let mut state = telebar_runtime::state::State::default();
        state
            .retained
            .reserve(telebar_runtime::recorded_send::Recording {
                token: "a".repeat(32),
                kind: "voice".into(),
                path: "/fixture/original.ogg".into(),
                source: None,
                chat: 42,
                args: serde_json::json!({}),
                content: None,
                status: telebar_runtime::recorded_send::Status::Preparing,
                input: None,
            })
            .unwrap();
        engine.accounts.insert(
            "default".into(),
            telebar_runtime::engine::Account {
                client: 41,
                generation: 1,
                state,
                ready: true,
            },
        );
        let request = telebar_runtime::wire::Request::parse(
            b"{\"id\":1,\"cmd\":\"recording.retry\",\"args\":{}}",
        )
        .unwrap();
        let effects =
            engine.request_using(7, request, std::time::Instant::now(), &mut |_, _, _| {
                Ok(telebar_runtime::commands::Action::BridgeOperation(
                    telebar_runtime::bridge::Operation::Test,
                ))
            });
        let token = effects.bridge_operations[0].0;
        if stale {
            engine.retire_peer(7);
        }
        let effects=engine.job_complete(token,Ok(telebar_runtime::commands::Action::Recorded {token:"a".repeat(32),content:serde_json::json!({"@type":"inputMessageVoiceNote","voice_note":{"voice_note":{"path":directory.join("input")}}}),options:serde_json::Value::Null,input:Some(input)}),std::time::Instant::now());
        if stale {
            assert!(effects.native.is_empty());
            assert!(!directory.join("input").exists());
        } else {
            engine.native_dispatched(&effects.native[0].query);
            engine.retire_peer(7);
            drop(engine);
            assert!(directory.join("input").exists());
        }
    }
    fs::remove_dir_all(root).unwrap();
}
