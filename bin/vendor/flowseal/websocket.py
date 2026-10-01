"""Bounded adaptation of Flowseal raw_websocket.py, MIT; see README.md.

Omagram uses system trust roots, normal hostname verification and Telegram hosts only.
"""
import asyncio
import base64
import hashlib
import os
import ssl
import struct

MAX_MESSAGE = 16 * 1024 * 1024
GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


def xor_mask(data, mask):
    expanded = (mask * (len(data) // 4 + 1))[:len(data)]
    return (int.from_bytes(data, "big") ^ int.from_bytes(expanded, "big")).to_bytes(len(data), "big")


def frame(opcode, data):
    if len(data) > MAX_MESSAGE:
        raise ValueError("WebSocket message too large")
    size = len(data)
    header = bytes((0x80 | opcode, 0x80 | size)) if size < 126 else (
        bytes((0x80 | opcode, 0xfe)) + struct.pack(">H", size) if size < 65536 else
        bytes((0x80 | opcode, 0xff)) + struct.pack(">Q", size))
    mask = os.urandom(4)
    return header + mask + xor_mask(data, mask)


class WebSocket:
    def __init__(self, reader, writer):
        self.reader, self.writer = reader, writer
        self.fragments = bytearray()
        self.fragmented = False

    @classmethod
    async def connect(cls, host, address=None):
        reader, writer = await asyncio.wait_for(asyncio.open_connection(
            address or host, 443, ssl=ssl.create_default_context(), server_hostname=host, limit=16384), 8)
        key = base64.b64encode(os.urandom(16)).decode()
        request = (f"GET /apiws HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                   f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: binary\r\n\r\n")
        try:
            writer.write(request.encode())
            await asyncio.wait_for(writer.drain(), 8)
            header = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 8)
            if len(header) > 16384:
                raise ValueError("WebSocket headers too large")
            lines = header.decode("ascii").split("\r\n")
            if lines[0].split()[1] != "101":
                raise ConnectionError("Telegram WebSocket upgrade refused")
            fields = dict((k.lower(), v.strip()) for k, v in (line.split(":", 1) for line in lines[1:] if ":" in line))
            expected = base64.b64encode(hashlib.sha1((key + GUID).encode()).digest()).decode()
            if (fields.get("sec-websocket-accept") != expected or fields.get("upgrade", "").lower() != "websocket"
                    or "upgrade" not in fields.get("connection", "").lower()):
                raise ValueError("invalid WebSocket upgrade response")
            return cls(reader, writer)
        except BaseException:
            writer.close()
            raise

    async def send(self, data, opcode=2):
        self.writer.write(frame(opcode, data))
        await asyncio.wait_for(self.writer.drain(), 30)

    async def recv(self):
        while True:
            header = await self.reader.readexactly(2)
            fin, opcode = bool(header[0] & 0x80), header[0] & 15
            if header[0] & 0x70 or header[1] & 0x80:
                raise ValueError("unsupported WebSocket frame flags")
            size = header[1] & 127
            if size == 126:
                size = struct.unpack(">H", await self.reader.readexactly(2))[0]
            elif size == 127:
                size = struct.unpack(">Q", await self.reader.readexactly(8))[0]
            if size > MAX_MESSAGE or (opcode >= 8 and (not fin or size > 125)):
                raise ValueError("invalid WebSocket frame length")
            data = await self.reader.readexactly(size)
            if opcode == 8:
                return None
            if opcode == 9:
                await self.send(data, 10)
                continue
            if opcode == 10:
                continue
            if opcode not in (0, 2) or (opcode == 0 and not self.fragmented) or (opcode == 2 and self.fragmented):
                raise ValueError("invalid WebSocket fragmentation")
            if not self.fragmented and fin:
                return data
            self.fragmented = True
            if len(self.fragments) + len(data) > MAX_MESSAGE:
                raise ValueError("WebSocket fragments too large")
            self.fragments.extend(data)
            if fin:
                result = bytes(self.fragments)
                self.fragments.clear()
                self.fragmented = False
                return result

    async def close(self):
        self.writer.close()
        try:
            await asyncio.wait_for(self.writer.wait_closed(), 2)
        except (OSError, asyncio.TimeoutError):
            pass
