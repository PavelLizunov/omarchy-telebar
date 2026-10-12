"""Keep Omarchy's palette explicit: QtQuick exports Color since Qt 6.12."""
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent


class ThemeNamespace(unittest.TestCase):
    def test_palette_cannot_resolve_to_qtquick_color(self):
        for directory in ("app", "shell"):
            for path in (ROOT / directory).glob("*.qml"):
                source = path.read_text()
                with self.subTest(path=str(path.relative_to(ROOT))):
                    self.assertIsNone(re.search(r"(?<![\w.])Color\.", source))
                    if "Commons.Color." in source:
                        self.assertIn("import qs.Commons as Commons", source)


if __name__ == "__main__":
    unittest.main()
