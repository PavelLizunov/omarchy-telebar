#!/usr/bin/python3
"""python3 tests/media_test.py -- omagram_media without a microphone, camera or ffmpeg run."""
import base64
import json
import os
import pathlib
import shutil
import struct
import sys
import tempfile
import time
import types
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "bin"))
import omagram_media as media  # noqa: E402
import omagram_state as model  # noqa: E402
import plugin_safety as safe  # noqa: E402


class Waveform(unittest.TestCase):
    def test_round_trip_with_the_decoder_the_window_uses(self):
        levels = [0, 31, 5, 17, 9] * 20
        encoded = base64.b64encode(media.waveform_bytes(levels)).decode()
        self.assertEqual(model.waveform(encoded, bars=100), levels)

    def test_values_are_clamped(self):
        encoded = base64.b64encode(media.waveform_bytes([40, -3, 12])).decode()
        self.assertEqual(model.waveform(encoded, bars=3), [31, 0, 12])

    def test_levels_follow_the_loudness(self):
        quiet = struct.pack("<400h", *([100] * 400))
        loud = struct.pack("<400h", *([-30000] * 400))
        self.assertEqual(media.levels_from_pcm(quiet + loud, samples=2), [0, 31])
        self.assertEqual(media.levels_from_pcm(b"", samples=10), [])
        self.assertEqual(len(media.levels_from_pcm(quiet, samples=100)), 100)
        self.assertEqual(media.levels_from_pcm(struct.pack("<3h", 0, 0, 0), samples=100), [0, 0, 0])

    def test_pcm_storage_preserves_signed_samples_and_bucket_boundaries(self):
        values = [0, 100, -100, 32767, -32768, 1200, -1200, 7] * 15
        for count in (1, 3, 8, 113, len(values)):
            pcm = struct.pack(f"<{count}h", *values[:count]) + b"\xff"
            for samples in (1, 3, 100):
                buckets = min(samples, count)
                peaks = [max(abs(v) for v in values[i * count // buckets:(i + 1) * count // buckets])
                         for i in range(buckets)]
                expected = [round(31 * p / (max(peaks) or 1)) for p in peaks]
                self.assertEqual(media.levels_from_pcm(pcm, samples), expected)
        tail = struct.pack("<113h", *([0] * 112 + [32767]))
        self.assertEqual(media.levels_from_pcm(tail), [0] * 99 + [31],
                         "The final PCM sample must contribute to the final bucket")


class Probing(unittest.TestCase):
    def test_recording_duration_is_finite_and_nonnegative(self):
        with mock.patch.object(media.safe, "tool", return_value="/usr/bin/ffprobe"), \
                mock.patch.object(media.safe, "run") as run:
            run.return_value = types.SimpleNamespace(ok=True, text=lambda: "12.6\n")
            self.assertEqual(media.duration_of("/synthetic/recording"), 12.6)
            for text in ("nan", "inf", "-inf", "-1", "N/A", ""):
                with self.subTest(text=text):
                    run.return_value = types.SimpleNamespace(ok=True, text=lambda: text)
                    with self.assertRaises(safe.UnsafeError):
                        media.duration_of("/synthetic/recording")

    def probe(self, result):
        with mock.patch.object(media.safe, "tool", return_value="/usr/bin/ffprobe"), \
                mock.patch.object(media.safe, "run", return_value=result):
            return media.probe_media("/tmp/clip.mp4")

    def test_length_and_size_for_telegrams_player(self):
        video = json.dumps({"streams": [{"codec_type": "audio"}, {"codec_type": "video", "width": 1280, "height": 720}],
                            "format": {"duration": "12.6"}})
        self.assertEqual(self.probe(types.SimpleNamespace(ok=True, text=lambda: video)), (13, 1280, 720))
        music = json.dumps({"streams": [{"codec_type": "audio"}], "format": {"duration": "201.2"}})
        self.assertEqual(self.probe(types.SimpleNamespace(ok=True, text=lambda: music)), (201, 0, 0))

    def test_what_cannot_be_read_is_zero_and_the_file_still_goes(self):
        odd = json.dumps({"format": {"duration": "nan"}, "streams": [{"codec_type": "video", "width": -3, "height": True}]})
        for result in (types.SimpleNamespace(ok=False, text=lambda: ""), types.SimpleNamespace(ok=True, text=lambda: "not json"),
                       types.SimpleNamespace(ok=True, text=lambda: "[]"), types.SimpleNamespace(ok=True, text=lambda: odd)):
            self.assertEqual(self.probe(result), (0, 0, 0))
        with mock.patch.object(media.safe, "tool", side_effect=media.safe.UnsafeError("no ffprobe")):
            self.assertEqual(media.probe_media("/tmp/clip.mp4"), (0, 0, 0))


class Recordings(unittest.TestCase):
    def setUp(self):
        self.root = pathlib.Path(tempfile.mkdtemp(prefix="omagram-media-", dir=safe.runtime_dir()))
        self.addCleanup(shutil.rmtree, self.root, True)
        for name, value in (("REC", self.root / "rec"), ("SENT", self.root / "sent")):
            patch = mock.patch.object(media, name, value)
            patch.start()
            self.addCleanup(patch.stop)

    def test_only_regular_files_in_the_recording_directory(self):
        rec = media.rec_dir()
        self.assertEqual(os.stat(rec).st_mode & 0o777, 0o700)
        good = media.new_path("note", ".mp4")
        good.write_bytes(b"recorded")
        self.assertEqual(media.recorded_file(str(good)), good)
        outside = self.root / "outside.mp4"
        outside.write_bytes(b"x")
        link = rec / "link.mp4"
        link.symlink_to(outside)
        empty = rec / "empty.mp4"
        empty.write_bytes(b"")
        (rec / "dir.mp4").mkdir()
        for bad in (str(outside), str(link), str(empty), str(rec / "dir.mp4"), str(rec / "missing.mp4"),
                    "note.mp4", str(rec) + "/../outside.mp4", str(good) + "\0", None, 5):
            with self.assertRaises((safe.UnsafeError, OSError), msg=repr(bad)):
                media.recorded_file(bad)

    def test_remove_and_clean_touch_only_the_recording_directory(self):
        rec = media.rec_dir()
        old = rec / "note-old.mp4"
        new = rec / "note-new.mp4"
        voice = rec / "voice-old.ogg"   # a voice message recorded here before may still be on screen
        for f in (old, new, voice):
            f.write_bytes(b"x")
        past = time.time() - 2 * media.STALE_SECONDS
        os.utime(old, (past, past))
        os.utime(voice, (past, past))
        outside = self.root / "keep.txt"
        outside.write_bytes(b"x")
        media.clean_stale()
        self.assertEqual((old.exists(), new.exists(), voice.exists()), (False, True, True))
        media.remove(outside)
        self.assertTrue(outside.exists())
        media.remove(new)
        media.remove(new)   # already gone: no error
        self.assertFalse(new.exists())

    def test_what_you_send_is_shown_from_where_it_is_kept(self):
        import omagram_state as model
        import omagram_td as td
        self.assertIn(str(td.SENT), td.MEDIA_ROOTS)
        self.assertIn(str(td.REC), td.MEDIA_ROOTS)
        self.assertNotIn(str(td.DATABASE), td.MEDIA_ROOTS)
        sent = str(td.SENT) + "/voice-0123.ogg"
        self.assertEqual(model.local_path(sent, td.MEDIA_ROOTS), sent)
        self.assertEqual(model.local_path(str(td.DATABASE) + "/db.sqlite", td.MEDIA_ROOTS), "")

    def test_commands_are_argument_lists_of_absolute_tools(self):
        with mock.patch.object(safe, "tool", lambda name: pathlib.Path("/usr/bin") / name):
            voice = media.voice_argv(self.root / "rec" / "voice-1.ogg")
            note = media.video_note_argv("/in.mp4", "/out.mp4")
        self.assertEqual(voice[0], "/usr/bin/ffmpeg")
        for part in ("pulse", "libopus", "ogg"):
            self.assertIn(part, voice)
        self.assertEqual(voice[-1], str(self.root / "rec" / "voice-1.ogg"))
        self.assertEqual((note[0], note[-1]), ("/usr/bin/ffmpeg", "/out.mp4"))
        self.assertIn(str(media.NOTE_MAX_SECONDS), note)
        self.assertTrue(any(f"scale={media.NOTE_SIZE}:{media.NOTE_SIZE}" in part for part in note))

    def test_a_profile_photo_becomes_a_centred_square_jpeg(self):
        with mock.patch.object(safe, "tool", lambda name: pathlib.Path("/usr/bin") / name):
            argv = media.profile_photo_argv("/in.png", "/out.jpg")
        self.assertEqual((argv[0], argv[argv.index("-i") + 1], argv[-1]), ("/usr/bin/ffmpeg", "/in.png", "/out.jpg"))
        self.assertIn("crop='min(iw,ih)':'min(iw,ih)'", argv[argv.index("-vf") + 1])
        self.assertEqual((argv[argv.index("-frames:v") + 1], argv[argv.index("-c:v") + 1]), ("1", "mjpeg"))


class NoteAnimations(unittest.TestCase):
    """Round videos as the quick view shows them: a silent animated WebP made once and kept."""

    def setUp(self):
        self.root = pathlib.Path(tempfile.mkdtemp(prefix="omagram-notes-", dir=safe.runtime_dir()))
        self.addCleanup(shutil.rmtree, self.root, True)
        for patch in (mock.patch.object(media, "NOTE_ANIMATIONS", self.root / "notes"),
                      mock.patch.object(safe, "tool", lambda name: pathlib.Path("/usr/bin") / name)):
            patch.start()
            self.addCleanup(patch.stop)
        self.source = self.root / "video_notes" / "note.mp4"
        self.source.parent.mkdir()
        self.source.write_bytes(bytes(4) + b"ftypmp42" + b"x" * 100)
        self.runs = []

    def fake_run(self, picture=b"RIFF" + bytes(4) + b"WEBPVP8X", ok=True):
        def run(argv, **kw):
            self.runs.append((argv, kw))
            if ok and picture:
                pathlib.Path(argv[-1]).write_bytes(picture)
            return types.SimpleNamespace(ok=ok, text=lambda: "")
        return run

    def test_the_command_is_an_argument_list_that_leaves_the_sound_out(self):
        argv = media.note_animation_argv("/in.mp4", "/out.webp")
        self.assertEqual((argv[0], argv[-1]), ("/usr/bin/ffmpeg", "/out.webp"))
        self.assertLess(argv.index("-t"), argv.index("-i"), "no more than a video message's length is read")
        self.assertEqual(argv[argv.index("-t") + 1], str(media.NOTE_MAX_SECONDS))
        for part in ("-an", "libwebp_anim", "webp"):
            self.assertIn(part, argv)
        self.assertEqual(argv[argv.index("-loop") + 1], "0")
        graph = argv[argv.index("-vf") + 1]
        self.assertIn(f"fps={media.NOTE_ANIMATION_RATE}", graph)
        self.assertIn(f"scale={media.NOTE_ANIMATION_SIDE}:{media.NOTE_ANIMATION_SIDE}", graph)

    def test_made_once_kept_and_named_after_the_file(self):
        with mock.patch.object(safe, "run", self.fake_run()):
            first = media.make_note_animation(str(self.source))
            again = media.make_note_animation(str(self.source))
        self.assertEqual(first, again)
        self.assertEqual(len(self.runs), 1, "the second time it is the kept one")
        self.assertEqual(self.runs[0][1]["timeout"], media.NOTE_ANIMATION_TIMEOUT)
        self.assertEqual(first.parent, self.root / "notes")
        self.assertEqual(oct(os.stat(self.root / "notes").st_mode & 0o777), "0o700")
        self.assertEqual([p.name for p in (self.root / "notes").iterdir()], [first.name], "no half-made file is left")
        self.source.write_bytes(b"y" * 300)
        os.utime(self.source, (2e9, 2e9))
        self.assertNotEqual(media.note_animation_path(str(self.source)), first, "a file downloaded again is new")

    def test_a_failed_or_empty_conversion_leaves_nothing_behind(self):
        for run in (self.fake_run(ok=False), self.fake_run(picture=b"")):
            with mock.patch.object(safe, "run", run):
                with self.assertRaises(safe.UnsafeError):
                    media.make_note_animation(str(self.source))
        self.assertEqual(list((self.root / "notes").iterdir()), [])

    def test_only_a_regular_file_of_a_round_videos_size(self):
        self.assertEqual(media.NOTE_ANIMATION_SOURCE_MAX, 64 * 1024 * 1024)
        patch = mock.patch.object(media, "NOTE_ANIMATION_SOURCE_MAX", 1024)
        patch.start()
        self.addCleanup(patch.stop)
        big = self.root / "video_notes" / "big.mp4"
        with open(big, "wb") as f:
            f.truncate(media.NOTE_ANIMATION_SOURCE_MAX + 1)
        empty = self.root / "video_notes" / "empty.mp4"
        empty.write_bytes(b"")
        folder = self.root / "video_notes" / "folder.mp4"
        folder.mkdir()
        with mock.patch.object(safe, "run", self.fake_run()):
            for bad in (big, empty, folder, self.root / "missing.mp4"):
                with self.assertRaises((safe.UnsafeError, OSError), msg=bad.name):
                    media.make_note_animation(str(bad))
        self.assertEqual(self.runs, [])

    def test_kept_pictures_stay_under_their_ceilings_least_recently_wanted_first(self):
        notes = self.root / "notes"
        notes.mkdir(mode=0o700)
        for n in range(6):
            picture = notes / f"{n:064x}.webp"
            picture.write_bytes(b"w" * 10)
            os.utime(picture, (1000 + n, 1000 + n))
        crashed = notes / "making-0123456789abcdef.webp"
        crashed.write_bytes(b"w")
        os.utime(crashed, (5, 5))
        (notes / "making-fedcba9876543210.webp").write_bytes(b"w")   # being made right now
        (notes / "keep.txt").write_bytes(b"x")
        with mock.patch.object(media, "NOTE_ANIMATIONS_FILES", 4):
            media.trim_note_animations()
        self.assertEqual(sorted(p.name for p in notes.iterdir()),
                         sorted([f"{n:064x}.webp" for n in (2, 3, 4, 5)] + ["making-fedcba9876543210.webp", "keep.txt"]))


class ClipboardImage(unittest.TestCase):
    def test_png_is_preserved_and_other_supported_images_offer_real_png(self):
        png = media.PNG_HEADER + b"synthetic PNG"
        with mock.patch.object(safe, "run") as run, mock.patch.object(safe, "tool", side_effect=lambda n: "/usr/bin/" + n):
            self.assertEqual(media.clipboard_png(png), png)
            run.assert_not_called()
            for original, decoder in [(b"\xff\xd8\xffsynthetic JPEG", "mjpeg"), (b"RIFF0000WEBPsynthetic", "webp")]:
                run.return_value = safe.Result(0, png, b"", False, False)
                self.assertEqual(media.clipboard_png(original), png)
                argv = run.call_args.args[0]
                self.assertEqual(argv[:5], ["/usr/bin/prlimit", "--as=1073741824", "--cpu=10", "--", "/usr/bin/ffmpeg"])
                self.assertIn(decoder, argv)
                self.assertEqual(argv[argv.index("-protocol_whitelist") + 1], "pipe")
                self.assertEqual(argv[argv.index("-max_pixels") + 1], str(media.CLIPBOARD_PIXELS_MAX))
                self.assertEqual(run.call_args.kwargs, {"input": original, "timeout": 10, "max_output": media.CLIPBOARD_IMAGE_MAX})
                self.assertEqual(argv[-1], "pipe:1")

    def test_empty_oversized_unsupported_and_failed_conversion_fail_closed(self):
        self.assertEqual(media.CLIPBOARD_IMAGE_MAX, 10 * 1024 * 1024)
        with mock.patch.object(safe, "run") as run, mock.patch.object(safe, "tool", side_effect=lambda n: "/usr/bin/" + n), \
             mock.patch.object(media, "CLIPBOARD_IMAGE_MAX", 64):
            for data in [b"", b"not an image", media.PNG_HEADER + b"x" * 64]:
                with self.assertRaises(safe.UnsafeError): media.clipboard_png(data)
            run.assert_not_called()
            for result in [safe.Result(1, b"", b"secret decoder error", False, False),
                           safe.Result(0, b"not PNG", b"", False, False),
                           safe.Result(0, media.PNG_HEADER, b"", True, False),
                           safe.Result(0, media.PNG_HEADER, b"", False, True)]:
                run.return_value = result
                with self.assertRaises(safe.UnsafeError): media.clipboard_png(b"\xff\xd8\xffJPEG")


if __name__ == "__main__":
    unittest.main(verbosity=1)
