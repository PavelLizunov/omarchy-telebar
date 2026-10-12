#!/usr/bin/python3
"""Actual Rust proxy/owner-pipe tests; namespace must have no host network."""
import json
import os
import selectors
import socket
import subprocess
import sys
import time

assert os.environ.get("TELEBAR_ISOLATED_PROXY") == "1"
assert all(name == "lo" for _, name in socket.if_nameindex())
binary = sys.argv[1]
process = subprocess.Popen([binary, "--proxy"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
try:
    process.stdin.write(b'{"secret":"0123456789abcdef0123456789abcdef"}\n')
    process.stdin.flush()
    with selectors.DefaultSelector() as poll:
        poll.register(process.stdout, selectors.EVENT_READ)
        assert poll.select(5), "proxy readiness timeout"
        ready = json.loads(process.stdout.readline())
    assert ready["event"] == "ready"
    clients = [socket.create_connection(("127.0.0.1", ready["port"]), 2) for _ in range(32)]
    # Invalid handshake must never reach DNS/TLS or expose private config.
    clients[0].sendall(bytes(64))
    process.stdin.close()
    assert process.wait(timeout=5) == 0
    for client in clients:
        client.close()
    output = process.stdout.read() + process.stderr.read()
    assert b"0123456789abcdef" not in output
    try:
        socket.create_connection(("127.0.0.1", ready["port"]), 0.5)
    except OSError:
        pass
    else:
        raise AssertionError("listener survived owner EOF")
    print(json.dumps({"rust_proxy": True, "owner_eof_shutdown": True, "inert_connections": 32,
                      "telegram_network": False, "configuration_in_argv": False}))
finally:
    if process.poll() is None:
        process.terminate()
    process.wait(timeout=5)
