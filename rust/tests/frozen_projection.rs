//! Exact bounded projection corpus; numeric representation stays in serde_json.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
};

fn replay(name: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let metadata: Value =
        serde_json::from_slice(&fs::read(root.join(format!("{name}.json"))).unwrap()).unwrap();
    let compressed = fs::read(root.join(format!("{name}.jsonl.gz"))).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&compressed)),
        metadata["compressed_sha256"].as_str().unwrap()
    );
    let maximum = metadata["limits"]["uncompressed_bytes"].as_u64().unwrap();
    assert!(maximum <= 64 * 1024 * 1024);
    let mut child = Command::new("/usr/bin/timeout")
        .args(["15s", "/usr/bin/gzip", "-cd", "--"])
        .arg(root.join(format!("{name}.jsonl.gz")))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .unwrap()
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    if bytes.len() as u64 > maximum {
        child.kill().unwrap();
    }
    assert!(child.wait().unwrap().success());
    assert_eq!(
        bytes.len() as u64,
        metadata["uncompressed_bytes"].as_u64().unwrap()
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        metadata["uncompressed_sha256"].as_str().unwrap()
    );
    let mut inputs = 0;
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let case: Value = serde_json::from_slice(line).unwrap();
        let roots: Vec<String> = case["roots"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect();
        let actual = telebar_runtime::projection::project(
            case["operation"].as_str().unwrap_or(""),
            &case["value"],
            &roots,
        )
        .unwrap();
        assert_eq!(actual, case["expected"], "{name} input {inputs}");
        inputs += 1;
    }
    assert_eq!(inputs, metadata["inputs"].as_u64().unwrap());
}

#[test]
fn frozen_baseline_21136_inputs_match() {
    replay("parity-corpus");
}
#[test]
fn frozen_valid_command_18_inputs_match() {
    replay("valid-command-corpus");
}
