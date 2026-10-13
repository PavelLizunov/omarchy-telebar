#!/usr/bin/python3
"""One public MTProto req_pq_multi via the actual Rust proxy/normal TLS.

Explicit wrapper only: no home/session bus/credentials/TDLib/device access.
This is not Telegram authorization or a message send. Exactly one client and
one nonce/query, no retry. Native fixed DC2 target fallback is capped at four
16-second attempts; owner shutdown is always checked. Python is test-only.
"""
import hashlib
import json
import os
import selectors
import socket
import struct
import subprocess
import sys
import time

sys.path.insert(0, "/oracle")
from transport import CTR, INTERMEDIATE

assert os.environ.get("TELEBAR_PUBLIC_PREAUTH_PROXY") == "1"
binary = sys.argv[1]
process = subprocess.Popen([binary, "--proxy"], stdin=subprocess.PIPE,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
port = None
started = time.monotonic()
try:
    secret = bytes(range(16))  # Disposable transport secret, not user material.
    process.stdin.write((json.dumps({"secret": secret.hex()}) + "\n").encode())
    process.stdin.flush()
    with selectors.DefaultSelector() as ready:
        ready.register(process.stdout, selectors.EVENT_READ)
        assert ready.select(5), "finite proxy readiness"
        status = json.loads(process.stdout.readline())
    assert status["event"] == "ready"
    port = status["port"]
    with socket.create_connection(("127.0.0.1", port), timeout=3) as client:
        deadline = time.monotonic() + 70
        def read_exact(size):
            assert 0 < size <= 65536
            result = bytearray()
            while len(result) < size:
                client.settimeout(max(0.01, deadline - time.monotonic()))
                chunk = client.recv(size - len(result))
                if not chunk:
                    raise ConnectionError("native proxy closed before public preauthorization response")
                result.extend(chunk)
            return bytes(result)
        # Fixed inert entropy makes protocol construction reproducible. Only
        # the nonce is random; no authorized session/auth key is constructed.
        seed = bytearray(range(1, 65))
        header = bytearray(seed)
        header[56:60] = INTERMEDIATE
        header[60:62] = struct.pack("<h", 2)
        key = hashlib.sha256(seed[8:40] + secret).digest()
        iv = bytes(seed[40:56])
        encrypted = CTR(key, iv).update(header)
        handshake = bytes(seed[:56]) + encrypted[56:]
        client.sendall(handshake)
        up = CTR(key, iv)
        up.update(bytes(64))
        reverse = handshake[8:56][::-1]
        down = CTR(hashlib.sha256(reverse[:32] + secret).digest(), reverse[32:])
        nonce = os.urandom(16)
        body = struct.pack("<I", 0xbe7e8ef1) + nonce  # req_pq_multi
        message_id = int(time.time() * (1 << 32)) & ~3
        payload = bytes(8) + struct.pack("<QI", message_id, len(body)) + body
        client.sendall(up.update(struct.pack("<I", len(payload)) + payload))
        length = struct.unpack("<I", down.update(read_exact(4)))[0] & 0x7fffffff
        assert 40 <= length <= 65536, f"invalid bounded public response length {length}"
        reply = down.update(read_exact(length))
        assert reply[:8] == bytes(8), "response must remain unauthenticated"
        body_length = struct.unpack_from("<I", reply, 16)[0]
        assert 20 <= body_length <= len(reply) - 20
        constructor = struct.unpack_from("<I", reply, 20)[0]
        assert constructor == 0x05162463, f"expected resPQ, got {constructor:08x}"
        assert reply[24:40] == nonce, "response must match the one public request"
    process.stdin.close()
    assert process.wait(timeout=5) == 0
    output = process.stdout.read() + process.stderr.read()
    assert secret.hex().encode() not in output
    try:
        socket.create_connection(("127.0.0.1", port), timeout=0.5)
    except OSError:
        pass
    else:
        raise AssertionError("proxy listener survived owner shutdown")
    print(json.dumps({"actualRustProxy": True, "publicRemotePreauth": True,
                      "dc": 2, "queries": 1, "clients": 1, "retries": 0,
                      "response": "resPQ", "responseBytes": length,
                      "nonceMatched": True, "ownerShutdown": True,
                      "elapsedSeconds": round(time.monotonic() - started, 3),
                      "workingProfiles": False, "authorization": False,
                      "telegramMessagesSent": 0}))
except Exception:
    # Retain finite native diagnostics on failure; never dump packets/nonce or
    # the private configuration. A failed live check remains failed evidence.
    if not process.stdin.closed:
        process.stdin.close()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.terminate()
        process.wait(timeout=3)
    diagnostics = process.stdout.read() + process.stderr.read()
    assert secret.hex().encode() not in diagnostics
    sys.stderr.write(json.dumps({"failedElapsedSeconds": round(time.monotonic() - started, 3),
                                 "nativeDiagnostics": diagnostics.decode("utf-8", "replace")}) + "\n")
    raise
finally:
    if process.poll() is None:
        process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=2)
