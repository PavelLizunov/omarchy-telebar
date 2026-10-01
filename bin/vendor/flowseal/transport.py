"""Adapted from Flowseal tg-ws-proxy _aes.py, tg_ws_proxy.py and bridge.py.

Copyright (c) 2026 Flowseal, MIT. Revision caa949bee0873d2b95dfb4fbeb1b7868b0ee3843.
Only AES-CTR, obfuscation handshake and packet splitting; no GUI, relay lists or updater.
"""
import ctypes
import hashlib
import os
import struct

ABRIDGED = b"\xef" * 4
INTERMEDIATE = b"\xee" * 4
SECURE = b"\xdd" * 4
MAX_PACKET = 16 * 1024 * 1024
RESERVED = (b"HEAD", b"POST", b"GET ", INTERMEDIATE, SECURE, b"\x16\x03\x01\x02")

_lib = ctypes.CDLL("/usr/lib/libcrypto.so.3")
_lib.EVP_CIPHER_CTX_new.restype = ctypes.c_void_p
_lib.EVP_CIPHER_CTX_free.argtypes = [ctypes.c_void_p]
_lib.EVP_aes_256_ctr.restype = ctypes.c_void_p
_lib.EVP_EncryptInit_ex.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
_lib.EVP_EncryptInit_ex.restype = ctypes.c_int
_lib.EVP_EncryptUpdate.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.POINTER(ctypes.c_int), ctypes.c_char_p, ctypes.c_int]
_lib.EVP_EncryptUpdate.restype = ctypes.c_int


class CTR:
    def __init__(self, key, iv):
        if len(key) != 32 or len(iv) != 16:
            raise ValueError("invalid AES-CTR key/iv length")
        self.ctx = _lib.EVP_CIPHER_CTX_new()
        if not self.ctx:
            raise RuntimeError("EVP allocation failed")
        if _lib.EVP_EncryptInit_ex(self.ctx, _lib.EVP_aes_256_ctr(), None, key, iv) != 1:
            self.close()
            raise RuntimeError("EVP initialization failed")

    def update(self, data):
        if len(data) > MAX_PACKET or not self.ctx:
            raise ValueError("invalid AES-CTR input")
        if not data:
            return b""
        length = ctypes.c_int()
        output = ctypes.create_string_buffer(len(data) + 16)
        if _lib.EVP_EncryptUpdate(self.ctx, output, ctypes.byref(length), bytes(data), len(data)) != 1:
            raise RuntimeError("EVP update failed")
        return output.raw[:length.value]

    def close(self):
        if getattr(self, "ctx", None):
            _lib.EVP_CIPHER_CTX_free(self.ctx)
            self.ctx = None

    def __del__(self):
        self.close()


def client_handshake(handshake, secret):
    if len(handshake) != 64 or len(secret) != 16:
        raise ValueError("invalid handshake size")
    key = hashlib.sha256(handshake[8:40] + secret).digest()
    plain = CTR(key, handshake[40:56]).update(handshake)
    tag = plain[56:60]
    dc = struct.unpack("<h", plain[60:62])[0]
    if tag not in (ABRIDGED, INTERMEDIATE, SECURE) or abs(dc) not in (1, 2, 3, 4, 5, 203):
        raise ValueError("invalid MTProto handshake")
    return dc, tag


def relay_handshake(tag, dc):
    while True:
        rnd = os.urandom(64)
        if rnd[0] != 0xef and rnd[:4] not in RESERVED and rnd[4:8] != b"\0" * 4:
            break
    encrypted = CTR(rnd[8:40], rnd[40:56]).update(rnd)
    tail = tag + struct.pack("<h", dc) + os.urandom(2)
    cipher_tail = bytes(tail[i] ^ encrypted[56 + i] ^ rnd[56 + i] for i in range(8))
    return rnd[:56] + cipher_tail


class CryptoCtx:
    def __init__(self, handshake, secret, relay):
        reverse = handshake[8:56][::-1]
        reverse_relay = relay[8:56][::-1]
        self.clt_dec = CTR(hashlib.sha256(handshake[8:40] + secret).digest(), handshake[40:56])
        self.clt_enc = CTR(hashlib.sha256(reverse[:32] + secret).digest(), reverse[32:])
        self.tg_enc = CTR(relay[8:40], relay[40:56])
        self.tg_dec = CTR(reverse_relay[:32], reverse_relay[32:])
        self.clt_dec.update(b"\0" * 64)
        self.tg_enc.update(b"\0" * 64)

    def close(self):
        for name in ("clt_dec", "clt_enc", "tg_enc", "tg_dec"):
            cipher = getattr(self, name, None)
            if cipher:
                cipher.close()


class MsgSplitter:
    def __init__(self, relay, tag):
        self.dec = CTR(relay[8:40], relay[40:56])
        self.dec.update(b"\0" * 64)
        self.tag = tag
        self.cipher = bytearray()
        self.plain = bytearray()

    def split(self, data):
        self.cipher.extend(data)
        self.plain.extend(self.dec.update(data))
        if len(self.cipher) > MAX_PACKET + 4:
            raise ValueError("MTProto packet buffer exceeded")
        offset, parts = 0, []
        while offset < len(self.plain):
            available = len(self.plain) - offset
            if self.tag == ABRIDGED:
                first = self.plain[offset]
                header = 4 if first in (0x7f, 0xff) else 1
                if available < header:
                    break
                size = (int.from_bytes(self.plain[offset + 1:offset + 4], "little") if header == 4 else first & 0x7f) * 4
            else:
                header = 4
                if available < header:
                    break
                size = struct.unpack_from("<I", self.plain, offset)[0] & 0x7fffffff
            if not 0 < size <= MAX_PACKET:
                raise ValueError("invalid MTProto packet length")
            if available < size + header:
                break
            parts.append(bytes(self.cipher[offset:offset + header + size]))
            offset += header + size
        if offset:
            del self.cipher[:offset]
            del self.plain[:offset]
        return parts

    def close(self):
        self.dec.close()
        self.cipher.clear()
        self.plain.clear()
