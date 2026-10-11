#!/usr/bin/python3
"""Replay the frozen synthetic oracle without importing the application runtime."""
import gzip
import hashlib
import json
import pathlib
import subprocess
import sys


def main():
    if len(sys.argv) != 2:
        raise SystemExit("Usage: replay-corpus.py <exact-built-Rust-runtime>")
    directory = pathlib.Path(__file__).resolve().parent
    metadata = json.loads((directory / "parity-corpus.json").read_bytes())
    compressed = (directory / "parity-corpus.jsonl.gz").read_bytes()
    assert hashlib.sha256(compressed).hexdigest() == metadata["compressed_sha256"]
    with gzip.open(directory / "parity-corpus.jsonl.gz", "rb") as source:
        raw = source.read(metadata["limits"]["uncompressed_bytes"] + 1)
    assert len(raw) == metadata["uncompressed_bytes"]
    assert hashlib.sha256(raw).hexdigest() == metadata["uncompressed_sha256"]
    cases = [json.loads(line) for line in raw.split(b"\n") if line]
    assert len(cases) == metadata["inputs"]
    requests = b"".join(json.dumps({k: v for k, v in case.items() if k != "expected"}, ensure_ascii=False).encode() + b"\n" for case in cases)
    process = subprocess.run([sys.argv[1], "--project"], input=requests, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=metadata["limits"]["runtime_seconds"], check=True)
    answers = [json.loads(line) for line in process.stdout.split(b"\n") if line]
    assert len(answers) == len(cases)
    differences = [i for i, (case, answer) in enumerate(zip(cases, answers)) if case["expected"] != answer]
    print(json.dumps({"inputs": len(cases), "differences": len(differences), "oracle": "frozen-synthetic-corpus", "application_imports": False}))
    if differences:
        print(json.dumps({"first_difference_cases": differences[:5]}))
    return 1 if differences else 0


if __name__ == "__main__":
    sys.exit(main())
