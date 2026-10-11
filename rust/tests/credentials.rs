//! Admitted save and session restart with fake storage, no real keyring/TDLib.
use serde_json::json;
use std::{collections::BTreeMap, time::Instant};
use telebar_runtime::{
    authorization::Runtime, commands::Action, keyring::CredentialSave, wire::Request,
};
fn request() -> Request {
    Request::parse(br#"{"id":1,"cmd":"credentials.set","args":{"apiId":123,"apiHash":"0123456789abcdef0123456789abcdef"}}"#).unwrap()
}
#[test]
fn secret_redaction_order_and_stale_completion_are_explicit() {
    let args = json!({"apiId":123,"apiHash":"0123456789abcdef0123456789abcdef"});
    let save = CredentialSave::prepare(&args, BTreeMap::new()).unwrap();
    assert!(!format!("{save:?}").contains("0123456789"));
    assert!(
        CredentialSave::prepare(
            &json!({"apiId":true,"apiHash":args["apiHash"]}),
            BTreeMap::new()
        )
        .is_err()
    );
    assert!(
        CredentialSave::prepare(
            &json!({"apiId":"0123","apiHash":args["apiHash"]}),
            BTreeMap::new()
        )
        .is_err()
    );
    let mut writes = Vec::new();
    let saved = save
        .save_with(&mut |key, label, value| {
            writes.push((key.to_owned(), label.to_owned(), value.to_owned()));
            Ok(true)
        })
        .unwrap();
    assert!(matches!(saved, Action::CredentialsSaved));
    assert_eq!(
        writes.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(),
        ["api_id", "api_hash"]
    );
    let mut count = 0;
    assert!(
        save.save_with(&mut |_, _, _| {
            count += 1;
            Ok(false)
        })
        .is_err()
    );
    assert_eq!(count, 1);
    let mut runtime = Runtime::new("default".into());
    let now = Instant::now();
    runtime
        .register("default".into(), "Synthetic".into(), 1, vec![], now)
        .unwrap();
    runtime.metadata.get_mut("default").unwrap().auth = json!({"state":"needCredentials"});
    let admitted = runtime
        .engine
        .request_using(7, request(), now, &mut |_, _, _| {
            Ok(Action::Work(telebar_runtime::jobs::Work::Credentials(
                CredentialSave::prepare(&args, BTreeMap::new()).unwrap(),
            )))
        });
    let token = admitted.work[0].0;
    let mut complete = runtime.engine.job_complete(token, Ok(saved), now);
    assert_eq!(complete.replies[0].1["ok"], true);
    let (id, generation) = complete.credentials_saved.pop().unwrap();
    let mut calls = 0;
    let configured = runtime.credentials_saved(&id, generation, now, &mut |_| {
        calls += 1;
        Ok(None)
    });
    assert_eq!(calls, 1);
    assert_eq!(configured.replies[0].1["auth"]["state"], "starting");
    assert_eq!(configured.replies[1].1["auth"]["state"], "needCredentials");
    assert!(configured.native.is_empty());
    // Credential storage may have succeeded after disconnect; never revive the
    // retired IPC token or run session parameter work through stale completion.
    let admitted = runtime
        .engine
        .request_using(8, request(), now, &mut |_, _, _| {
            Ok(Action::Work(telebar_runtime::jobs::Work::Credentials(
                CredentialSave::prepare(&args, BTreeMap::new()).unwrap(),
            )))
        });
    runtime.engine.retire_peer(8);
    let done = runtime
        .engine
        .job_complete(admitted.work[0].0, Ok(Action::CredentialsSaved), now);
    assert!(done.replies.is_empty() && done.credentials_saved.is_empty());
    let configured = runtime.credentials_saved(&id, generation + 1, now, &mut |_| {
        panic!("stale generation must not load parameters")
    });
    assert!(configured.replies.is_empty());
    let admitted = runtime
        .engine
        .request_using(9, request(), now, &mut |_, _, _| {
            Ok(Action::Reply(json!({})))
        });
    assert!(admitted.credentials_saved.is_empty());
    let admitted = runtime
        .engine
        .request_using(9, request(), now, &mut |_, _, _| {
            Ok(Action::Work(telebar_runtime::jobs::Work::Credentials(
                CredentialSave::prepare(&args, BTreeMap::new()).unwrap(),
            )))
        });
    let expired = runtime.engine.job_complete(
        admitted.work[0].0,
        Ok(Action::CredentialsSaved),
        now + telebar_runtime::lifecycle::REQUEST_TIMEOUT,
    );
    assert!(expired.credentials_saved.is_empty() && expired.native.is_empty());
    let mut count = 0;
    assert!(
        save.save_with(&mut |_, _, _| {
            count += 1;
            Ok(count == 1)
        })
        .is_err()
    );
    assert_eq!(count, 2);
    runtime.metadata.get_mut(&id).unwrap().auth = json!({"state":"ready"});
    let configured = runtime.credentials_saved(&id, generation, now, &mut |_| {
        panic!("ready account must not restart")
    });
    assert!(configured.replies.is_empty());
}
