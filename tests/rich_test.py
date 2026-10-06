#!/usr/bin/python3
"""Rich posts: pure normalization and the real daemon loop with fake TDLib."""
import sys
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "bin"))
import omagram_state as model
from daemon_test import Harness


def plain(text):
    return {"@type": "richTextPlain", "text": text}


def post(full=True):
    return {"@type": "richMessage", "is_full": full, "is_rtl": False, "blocks": [
        {"@type": "pageBlockParagraph", "text": {"@type": "richTextBold", "text": plain("Before 👋")}},
        {"@type": "pageBlockPhoto", "photo": {"sizes": [{"@type": "photoSize", "type": "x", "width": 960, "height": 640,
          "photo": {"@type": "file", "id": 55, "size": 1000, "local": {}, "remote": {}}}]},
         "caption": {"@type": "pageBlockCaption", "text": plain("Caption"), "credit": plain("")}},
        {"@type": "pageBlockParagraph", "text": {"@type": "richTextUrl", "text": plain("After"), "url": "https://example.org"}}
    ]}


class RichState(unittest.TestCase):
    def test_order_entities_media_preview_and_updates(self):
        c = model.content({"@type": "messageRichMessage", "message": post()})
        self.assertEqual(c["kind"], "rich")
        self.assertEqual([b["kind"] for b in c["blocks"]], ["text", "photo", "text", "text"])
        self.assertEqual(c["blocks"][1]["media"]["file"]["id"], 55)
        self.assertEqual(c["blocks"][0]["entities"], [{"type": "bold", "offset": 0, "length": 9}])
        self.assertEqual(c["blocks"][-1]["entities"][0]["url"], "https://example.org")
        self.assertEqual(model.preview_text(c), "Before 👋 Caption After")
        state = model.State("")
        updated = state.apply({"@type": "updateMessageContent", "chat_id": 42, "message_id": 27,
                               "new_content": {"@type": "messageRichMessage", "message": post()}})
        self.assertEqual(updated[0]["content"], c)

    def test_nested_utf16_offsets(self):
        rich = post()
        rich["blocks"] = [{"@type": "pageBlockParagraph", "text": {"@type": "richTexts", "texts": [
            plain("👋 "), {"@type": "richTextBold", "text": {"@type": "richTextItalic", "text": plain("hello")}}]}}]
        c = model.rich_message(rich)
        self.assertEqual(c["blocks"][0]["text"], "👋 hello")
        self.assertEqual([(e["type"], e["offset"], e["length"]) for e in c["blocks"][0]["entities"]],
                         [("italic", 3, 5), ("bold", 3, 5)])

    def test_lists_covers_and_media_spoilers(self):
        rich = post()
        rich["blocks"][1]["has_spoiler"] = True
        rich["blocks"] = [{"@type": "pageBlockList", "items": [{"label": "1.", "blocks": rich["blocks"]}]}]
        c = model.rich_message(rich)
        self.assertEqual(c["blocks"][0]["text"], "1. Before 👋")
        self.assertEqual(c["blocks"][0]["entities"][0]["offset"], 3)
        self.assertTrue(c["blocks"][1]["spoiler"])

    def test_bounds_malformed_unknown_and_untrusted_html(self):
        for raw in (None, True, [], "text", {"@type": "richMessage", "blocks": "bad"}):
            self.assertEqual(model.rich_message(raw)["blocks"], [])
        rich = post()
        nested = plain("too deep")
        for _ in range(100):
            nested = {"@type": "richTextBold", "text": nested}
        rich["blocks"] = [{"@type": "pageBlockParagraph", "text": nested},
                          {"@type": "pageBlockEmbedded", "html": "<script>not executable</script>"}]
        c = model.rich_message(rich)
        self.assertTrue(c["truncated"])
        self.assertEqual(c["blocks"][0]["text"], "Unsupported block")
        self.assertNotIn("html", c["blocks"][0])
        malformed = {"@type": "richMessage", "blocks": [{"@type": []}, {"@type": "pageBlockParagraph", "text": {"@type": [], "text": "\ud800"}}]}
        self.assertEqual(model.rich_message(malformed)["blocks"][1]["text"], "?")
        media = post()
        media["blocks"][1]["photo"]["sizes"][0]["photo"]["local"] = {"is_downloading_completed": True, "path": "/etc/passwd"}
        self.assertEqual(model.rich_message(media, "/synthetic/files")["blocks"][1]["media"]["file"]["path"], "")
        rich["blocks"] = [{"@type": "pageBlockParagraph", "text": plain("x" * 100000)}] * 1000
        c = model.rich_message(rich)
        self.assertLessEqual(len(c["text"]), model.TEXT_MAX)
        self.assertLessEqual(len(c["blocks"]), model.RICH_BLOCKS_MAX)


class RichDaemon(Harness):
    def test_full_request_and_failure(self):
        conn = self.connect()
        self.sign_in(conn)
        self.send(conn, {"id": 1, "cmd": "message.rich", "args": {"chatId": 42, "messageId": 27}})
        query = self.last_query("getFullRichMessage")
        self.assertEqual((query["chat_id"], query["message_id"]), (42, 27))
        self.answer(query, post())
        answer = self.read(conn, lambda v: v.get("id") == 1)
        self.assertTrue(answer["ok"])
        self.assertTrue(answer["result"]["full"])
        self.assertEqual(answer["result"]["blocks"][1]["media"]["file"]["id"], 55)
        before = self.sent_count("getFullRichMessage")
        self.send(conn, {"id": 2, "cmd": "message.rich", "args": {"chatId": 42, "messageId": 27}})
        query = self.next_query("getFullRichMessage", before)
        self.answer(query, {"@type": "error", "code": 400, "message": "message unavailable"})
        self.assertFalse(self.read(conn, lambda v: v.get("id") == 2)["ok"])

    def test_rich_photo_export_checks_membership_and_permissions(self):
        conn = self.connect()
        self.sign_in(conn)
        client = next(iter(self.daemon.clients.values()))
        self.daemon.checked_photo_action(client, 4, {"chatId": 42, "messageId": 27, "fileId": 55}, lambda: {"checked": True})
        query = self.last_query("getMessage")
        self.answer(query, {"@type": "message", "chat_id": 42, "id": 27,
                            "content": {"@type": "messageRichMessage", "message": post(False)}})
        query = self.last_query("getFullRichMessage")
        self.answer(query, post())
        query = self.last_query("getMessageProperties")
        self.answer(query, {"@type": "messageProperties", "can_be_saved": True})
        self.assertEqual(self.read(conn, lambda v: v.get("id") == 4)["result"], {"checked": True})
        before = self.sent_count("getMessage")
        self.daemon.checked_photo_action(client, 5, {"chatId": 42, "messageId": 27, "fileId": 999}, lambda: {})
        self.answer(self.next_query("getMessage", before), {"@type": "message", "chat_id": 42, "id": 27,
                    "content": {"@type": "messageRichMessage", "message": post()}})
        self.assertFalse(self.read(conn, lambda v: v.get("id") == 5)["ok"])
        before = self.sent_count("getMessage")
        before_properties = self.sent_count("getMessageProperties")
        self.daemon.checked_photo_action(client, 6, {"chatId": 42, "messageId": 27, "fileId": 55}, lambda: {"unexpected": True})
        self.answer(self.next_query("getMessage", before), {"@type": "message", "chat_id": 42, "id": 27,
                    "content": {"@type": "messageRichMessage", "message": post()}})
        self.answer(self.next_query("getMessageProperties", before_properties), {"@type": "messageProperties", "can_be_saved": False})
        self.assertFalse(self.read(conn, lambda v: v.get("id") == 6)["ok"])


if __name__ == "__main__":
    unittest.main()
