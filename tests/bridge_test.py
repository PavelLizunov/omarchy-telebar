"""Bundled transport/owner checks; no remote connections or production sessions."""
import asyncio
import os
import pathlib
import shutil
import socket
import sys
import tempfile
import unittest
import threading
import time
from unittest import mock

sys.dont_write_bytecode = True
BIN = pathlib.Path(__file__).resolve().parent.parent / "bin"
sys.path.insert(0, str(BIN))
import omagram_bridge
import omagram_diagnostics
import omagram_settings
import plugin_safety as safe
from vendor.flowseal.transport import CTR, MsgSplitter, SECURE, relay_handshake, client_handshake
from vendor.flowseal.websocket import WebSocket, frame, xor_mask


class Crypto(unittest.TestCase):
    def test_aes_ctr_nist_vector(self):
        key = bytes.fromhex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4")
        iv = bytes.fromhex("f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff")
        plain = bytes.fromhex("6bc1bee22e409f96e93d7e117393172a")
        expected = bytes.fromhex("601ec313775789a5b7a7f504bbf3d228")
        self.assertEqual(CTR(key, iv).update(plain), expected)

    def test_splitter_preserves_fragmented_ciphertext_and_bounds_lengths(self):
        relay = relay_handshake(SECURE, 2)
        enc = CTR(relay[8:40], relay[40:56])
        enc.update(b"\0" * 64)
        packet = b"\x08\0\0\0abcdefgh"
        data = enc.update(packet)
        splitter = MsgSplitter(relay, SECURE)
        self.assertEqual(splitter.split(data[:3]), [])
        self.assertEqual(splitter.split(data[3:]), [data])
        enc = CTR(relay[8:40], relay[40:56]); enc.update(b"\0" * 64)
        with self.assertRaises(ValueError):
            MsgSplitter(relay, SECURE).split(enc.update(b"\xff\xff\xff\x7f"))

    def test_wrong_secret_and_short_handshake_fail(self):
        with self.assertRaises(ValueError):
            client_handshake(b"x", b"s" * 16)
        with self.assertRaises(ValueError):
            client_handshake(b"x" * 64, b"s" * 16)

    def test_websocket_mask_roundtrip(self):
        data = b"payload" * 200
        encoded = frame(2, data)
        self.assertEqual(encoded[0], 0x82)
        self.assertEqual(xor_mask(encoded[8:], encoded[4:8]), data)


class WebSocketChecks(unittest.IsolatedAsyncioTestCase):
    async def test_fragment_accumulation_and_invalid_flags(self):
        class Writer:
            def write(self, data): pass
            async def drain(self): pass
        reader = asyncio.StreamReader()
        reader.feed_data(b"\x02\x03abc\x80\x03def")
        ws = WebSocket(reader, Writer())
        self.assertEqual(await ws.recv(), b"abcdef")
        reader.feed_data(b"\x82\x80")
        with self.assertRaises(ValueError):
            await ws.recv()


class Owner(unittest.TestCase):
    def test_start_stop_is_local_bounded_and_secrets_not_in_argv(self):
        root = pathlib.Path(tempfile.mkdtemp(prefix="omagram-bridge-test-", dir=safe.runtime_dir()))
        self.addCleanup(shutil.rmtree, root, True)
        with mock.patch.object(omagram_settings, "CONFIG", root):
            owner = omagram_bridge.Bridge(omagram_diagnostics.Diagnostics(enabled=False))
            self.addCleanup(owner.stop)
            port = owner.start()
            self.assertTrue(1 <= port <= 65535)
            cmd = pathlib.Path(f"/proc/{owner.proc.pid}/cmdline").read_bytes()
            self.assertNotIn(owner.secret.encode(), cmd)
            with socket.create_connection(("127.0.0.1", port), timeout=2) as connection:
                connection.sendall(b"x" * 64)
            owner.metadata["default"] = {"proxy_id": 1, "previous_id": 2}
            owner.save()
            text = (root / "bridge.json").read_text()
            self.assertNotIn(owner.secret, text)
            self.assertEqual((root / "bridge.json").stat().st_mode & 0o777, 0o600)
            proc = owner.proc
            owner.stop()
            self.assertIsNotNone(proc.poll())

    def test_bridge_survives_start_worker_and_exits_when_owner_pipe_closes(self):
        root = pathlib.Path(tempfile.mkdtemp(prefix="omagram-bridge-worker-", dir=safe.runtime_dir()))
        self.addCleanup(shutil.rmtree, root, True)
        with mock.patch.object(omagram_settings, "CONFIG", root):
            owner = omagram_bridge.Bridge(omagram_diagnostics.Diagnostics(enabled=False))
            self.addCleanup(owner.stop)
            errors = []
            def start():
                try: owner.start()
                except Exception as error: errors.append(error)
            worker = threading.Thread(target=start)
            worker.start(); worker.join(7)
            self.assertFalse(worker.is_alive())
            self.assertEqual(errors, [])
            time.sleep(.1)
            self.assertIsNone(owner.proc.poll(), "Spawning worker exit must not kill the proxy")
            owner.proc.stdin.close()
            owner.proc.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
