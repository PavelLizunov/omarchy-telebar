//! Native proxy continuation preserves previous identities and refuses races.
use serde_json::json;
use telebar_runtime::{
    bridge::{Answer, Mutation},
    commands::{Action, Answer as NativeAnswer},
};
#[test]
fn proxy_enable_disable_preserves_prior_proxy_and_unrelated_choice() {
    let proxy = json!({"@type":"proxy","server":"127.0.0.1","port":42,"type":{"@type":"proxyTypeMtproto","secret":"synthetic"}});
    let action = Answer::EnableList {
        proxy: proxy.clone(),
        entry: serde_json::Value::Null,
    }
    .complete(&json!({"proxies":[{"id":4,"is_enabled":true}]}))
    .unwrap();
    let Action::Native {
        query,
        answer: NativeAnswer::Bridge(answer),
    } = action
    else {
        panic!("native configure")
    };
    assert_eq!(query["@type"], "addProxy");
    assert_eq!(query["proxy"], proxy);
    let Action::BridgeMutation(Mutation::Enable(entry)) =
        answer.complete(&json!({"id":7})).unwrap()
    else {
        panic!("metadata handoff")
    };
    assert_eq!(entry, json!({"proxy_id":7,"previous_id":4}));
    let action = Answer::DisableList {
        entry: entry.clone(),
    }
    .complete(&json!({"proxies":[{"id":7,"is_enabled":true},{"id":4}]}))
    .unwrap();
    let Action::Native { query, .. } = action else {
        panic!("restore native")
    };
    assert_eq!(query, json!({"@type":"enableProxy","proxy_id":4}));
    assert!(matches!(
        Answer::DisableList { entry }
            .complete(&json!({"proxies":[{"id":8,"is_enabled":true}]}))
            .unwrap(),
        Action::BridgeMutation(Mutation::Disable)
    ));
    assert!(
        Answer::Enabled { previous: json!(4) }
            .complete(&json!({"id":0}))
            .is_err()
    );
}
