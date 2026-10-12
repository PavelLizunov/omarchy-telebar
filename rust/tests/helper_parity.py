#!/usr/bin/python3
"""Pure deterministic helper parity; requires the existing inert namespace."""
import hashlib
import importlib.machinery
import importlib.util
import json
import os
import pathlib
import random
import socket
import struct
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
assert os.statvfs(ROOT).f_flag & os.ST_RDONLY
assert all(name == "lo" for _, name in socket.if_nameindex())
assert os.environ.get("HOME") == "/home/worker"
sys.path.insert(0, str(ROOT / "bin"))
import plugin_safety
plugin_safety.home_dir = lambda: "/tmp/home"
from vendor.flowseal import transport as original
loader = importlib.machinery.SourceFileLoader("original_menu", str(ROOT / "bin/omagram-menu-install"))
spec = importlib.util.spec_from_loader(loader.name, loader)
menu = importlib.util.module_from_spec(spec)
loader.exec_module(menu)

cases = []
def case(operation, value, expected):
    cases.append({"operation": operation, "value": value, "expected": expected})
rng = random.Random(20261012)
for _ in range(80):
    key, iv, data = rng.randbytes(32), rng.randbytes(16), rng.randbytes(rng.randrange(0, 1500))
    case("proxy_crypto", {"kind": "ctr", "key": key.hex(), "iv": iv.hex(), "data": data.hex()}, original.CTR(key, iv).update(data).hex())
for dc in (1, -1, 2, -2, 3, 4, -4, 5, 203):
    for tag in (original.ABRIDGED, original.INTERMEDIATE, original.SECURE):
        seed, tail, secret = rng.randbytes(64), rng.randbytes(2), rng.randbytes(16)
        cipher = original.CTR(seed[8:40], seed[40:56]).update(seed)
        plain = tag + struct.pack("<h", dc) + tail
        relay = seed[:56] + bytes(plain[i] ^ cipher[56+i] ^ seed[56+i] for i in range(8))
        case("proxy_crypto", {"kind": "relay", "random": seed.hex(), "tail": tail.hex(), "tag": tag.hex(), "dc": dc}, relay.hex())
        plain = tag + struct.pack("<h", dc) + tail
        cipher = original.CTR(hashlib.sha256(seed[8:40] + secret).digest(), seed[40:56]).update(seed)
        handshake = seed[:56] + bytes(plain[i] ^ cipher[56+i] ^ seed[56+i] for i in range(8))
        actual_dc, actual_tag = original.client_handshake(handshake, secret)
        case("proxy_crypto", {"kind": "client", "handshake": handshake.hex(), "secret": secret.hex()}, {"dc": actual_dc, "tag": actual_tag.hex()})
        packet = (b"\x02" if tag == original.ABRIDGED else struct.pack("<I", 8)) + rng.randbytes(8)
        encryptor = original.CTR(relay[8:40], relay[40:56])
        encryptor.update(bytes(64))
        encrypted = encryptor.update(packet)
        for step in (1, 2, 3, 7, 99):
            splitter = original.MsgSplitter(relay, tag)
            expected = []
            for offset in range(0, len(encrypted), step):
                expected.extend(chunk.hex() for chunk in splitter.split(encrypted[offset:offset+step]))
            case("proxy_crypto", {"kind": "split", "relay": relay.hex(), "tag": tag.hex(), "data": encrypted.hex(), "step": step}, expected)
for raw in ("", "{}", '{"a":1,}', "[]", "// note\n{\n // x\n \"a\": [1,2,],\n}\n", '{"value":",}"}', '{"a":1} // inline', '{"a":1,\n}\n'):
    for kind, fn in (("strip", menu.strip_jsonc), ("valid", menu.valid), ("remove", menu.without_block)):
        case("menu_helper", {"kind": kind, "raw": raw}, fn(raw))
block = '\n  // >>> io.github.pavellizunov.telebar (managed by old-script) >>>\n  "telebar":{},\n  // <<< io.github.pavellizunov.telebar <<<\n'
for raw in ("{"+block+' "kept":1}\n', "{"+block+block+"}\n"):
    case("menu_helper", {"kind": "remove", "raw": raw}, menu.without_block(raw))
result = subprocess.run([sys.argv[1], "--project"], input=b"".join((json.dumps({k:v for k,v in case.items() if k != "expected"})+"\n").encode() for case in cases), stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15, check=True)
outputs = result.stdout.split(b"\n")
assert outputs[-1] == b""
assert len(outputs) == len(cases)+1
for index, (actual, item) in enumerate(zip(outputs, cases)):
    assert json.loads(actual) == item["expected"], (index, item, actual)
print(json.dumps({"helper_inputs": len(cases), "differences": 0, "network": False}))
