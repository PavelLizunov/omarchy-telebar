#!/usr/bin/python3
"""File exports through the real isolated IPC loop and synthetic TDLib replies."""
import pathlib
import unittest
from unittest import mock
from daemon_test import Harness


class ExportContract(Harness):
    def setUp(self):
        super().setUp()
        self.conn = self.connect()
        self.sign_in(self.conn)
        files = self.root / "export-files"
        files.mkdir(mode=0o700)
        self.path = files / "original.bin"
        self.path.write_bytes(b"synthetic export bytes")
        self.daemon.state.files_root = (str(files),)
        patch = mock.patch.object(self.d, "DOWNLOADS", self.root / "Downloads")
        patch.start()
        self.addCleanup(patch.stop)
        self.file = {"@type": "file", "id": 5, "local": {
            "path": str(self.path), "is_downloading_completed": True}}

    def export(self, rid, content, allowed=True, chat=42, message=7):
        before = {kind: self.sent_count(kind) for kind in ("getFile", "getMessage", "getMessageProperties")}
        self.send(self.conn, {"id": rid, "cmd": "file.save", "args": {
            "chatId": 42, "messageId": 7, "fileId": 5, "fileName": "export.bin"}})
        self.answer(self.next_query("getFile", before["getFile"]), self.file)
        self.answer(self.next_query("getMessage", before["getMessage"]), {
            "@type": "message", "chat_id": chat, "id": message, "content": content})
        if chat == 42 and message == 7 and allowed is not None:
            self.answer(self.next_query("getMessageProperties", before["getMessageProperties"]), {
                "@type": "messageProperties", "can_be_saved": allowed})
        return self.read(self.conn, lambda v: v.get("id") == rid)

    def test_context_required_before_file_lookup(self):
        for rid, args in enumerate(({}, {"chatId": 42}, {"messageId": 7}), 1):
            before = self.sent_count("getFile")
            answer = self.request(self.conn, rid, "file.save", fileId=5, **args)
            self.assertFalse(answer["ok"])
            self.assertEqual(self.sent_count("getFile"), before)
        self.assertFalse((self.root / "Downloads").exists())

    def test_all_supported_media_require_explicit_permission(self):
        examples = [
            {"@type": "messagePhoto", "photo": {"@type": "photo", "sizes": [{"@type": "photoSize", "width": 960, "height": 640, "photo": {"@type": "file", "id": 5}}]}},
            {"@type": "messageDocument", "document": {"@type": "document", "document": {"@type": "file", "id": 5}}},
            {"@type": "messageVideo", "video": {"@type": "video", "video": {"@type": "file", "id": 5}}},
            {"@type": "messageAudio", "audio": {"@type": "audio", "audio": {"@type": "file", "id": 5}}},
            {"@type": "messageVoiceNote", "voice_note": {"@type": "voiceNote", "voice": {"@type": "file", "id": 5}}},
            {"@type": "messageVideoNote", "video_note": {"@type": "videoNote", "video": {"@type": "file", "id": 5}}},
            {"@type": "messageAnimation", "animation": {"@type": "animation", "animation": {"@type": "file", "id": 5}}},
            {"@type": "messageSticker", "sticker": {"@type": "sticker", "sticker": {"@type": "file", "id": 5}}},
        ]
        for index, content in enumerate(examples):
            with self.subTest(kind=content["@type"]):
                self.assertFalse(self.export(10 + index * 2, content, False)["ok"])
                answer = self.export(11 + index * 2, content)
                self.assertTrue(answer["ok"], answer)
                self.assertEqual(pathlib.Path(answer["result"]["path"]).read_bytes(), self.path.read_bytes())

    def test_copy_image_rejects_wrong_returned_file_before_message_checks(self):
        before = self.sent_count("getFile")
        checks = self.sent_count("getMessage")
        with mock.patch.object(self.d.safe, "has_tool", return_value=True):
            self.send(self.conn, {"id": 38, "cmd": "file.copyImage", "args": {
                "chatId": 42, "messageId": 7, "fileId": 5}})
            self.answer(self.next_query("getFile", before), dict(self.file, id=6))
            answer = self.read(self.conn, lambda v: v.get("id") == 38)
        self.assertFalse(answer["ok"])
        self.assertEqual(self.sent_count("getMessage"), checks)

    def test_thumbnail_cannot_replace_original_media(self):
        video = {"@type": "messageVideo", "video": {
            "@type": "video", "video": {"@type": "file", "id": 6},
            "thumbnail": {"@type": "thumbnail", "format": {"@type": "thumbnailFormatJpeg"},
                          "width": 120, "height": 90, "file": {"@type": "file", "id": 5}}}}
        before = self.sent_count("getMessageProperties")
        self.assertFalse(self.export(39, video, allowed=None)["ok"])
        self.assertEqual(self.sent_count("getMessageProperties"), before)
        self.assertFalse((self.root / "Downloads").exists())

    def test_membership_context_and_unknown_permission_fail_closed(self):
        document = {"@type": "messageDocument", "document": {"@type": "document", "document": {"@type": "file", "id": 5}}}
        self.assertFalse(self.export(40, document, chat=43, allowed=None)["ok"])
        self.assertFalse(self.export(41, document, message=8, allowed=None)["ok"])
        wrong = {"@type": "messageDocument", "document": {"@type": "document", "document": {"@type": "file", "id": 6}}}
        self.assertFalse(self.export(42, wrong, allowed=None)["ok"])
        self.assertFalse(self.export(43, document, allowed="true")["ok"])
        self.assertFalse((self.root / "Downloads").exists())


if __name__ == "__main__":
    unittest.main()
