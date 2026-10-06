#!/usr/bin/python3
"""Telebar identity and compatibility checks; no installation or live service access."""
import ast
import json
import pathlib
import re
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLUGIN_ID = "io.github.pavellizunov.telebar"


def constants(source):
    result = {}
    for node in ast.parse(source).body:
        if isinstance(node, ast.Assign):
            try:
                value = ast.literal_eval(node.value)
            except (ValueError, TypeError):
                continue
            for target in node.targets:
                if isinstance(target, ast.Name):
                    result[target.id] = value
    return result


class TelebarIdentity(unittest.TestCase):
    def test_manifest_and_consumers_agree(self):
        manifest = json.loads((ROOT / "manifest.json").read_text())
        self.assertEqual((manifest["id"], manifest["name"], manifest["author"], manifest["license"]),
                         (PLUGIN_ID, "Telebar", "PavelLizunov", "MIT"))
        self.assertEqual(manifest["barWidget"]["displayName"], "Telebar")
        self.assertIn("telebar", manifest["barWidget"]["aliases"])
        for path in manifest["entryPoints"].values():
            self.assertTrue((ROOT / path).is_file(), path)
        for path in ("bin/omagram", "bin/omagramd", "bin/omagram_settings.py", "bin/omagram-menu-install"):
            self.assertEqual(constants((ROOT / path).read_text())["PLUGIN_ID"], PLUGIN_ID, path)
        for path in ("shell/BarWidget.qml", "shell/QuickReply.qml", "shell/Panel.qml", "menu.jsonc"):
            text = (ROOT / path).read_text()
            self.assertIn(PLUGIN_ID, text, path)
            self.assertNotIn("reidenxerx.omagram", text, path)

    def test_account_and_runtime_compatibility(self):
        launcher = constants((ROOT / "bin/omagram").read_text())
        self.assertEqual((launcher["APP_ID"], launcher["DESKTOP_FILE"], launcher["DESKTOP_MARK"]),
                         ("omagram", "omagram.desktop", "X-Omagram-Managed"))
        self.assertEqual(launcher["LEGACY_PLUGIN_ID"], "reidenxerx.omagram")
        self.assertEqual(constants((ROOT / "bin/omagram_settings.py").read_text())["DESCRIPTION_PREFIX"], "Omagram: ")
        # Account paths/keyring and original copyright were not part of the rename.
        for path in ("bin/omagram_td.py", "LICENSE", "THIRD_PARTY_NOTICES.md"):
            base = subprocess.check_output(["git", "show", "HEAD:" + path], cwd=ROOT)
            self.assertEqual((ROOT / path).read_bytes(), base, path)

    def test_documented_identity_and_attribution(self):
        text = (ROOT / "README.md").read_text()
        self.assertIn("PavelLizunov/omarchy-telebar", text)
        self.assertIn("ReidenXerx/omarchy-omagram", text)
        self.assertIn("Do not run Telebar and Omagram together", text)
        self.assertNotIn("PavelLizunov/omarchy-tg", text)
        self.assertNotIn("retains the upstream plugin ID", text)
        # Component and machine identifiers intentionally retain the old name.
        for folder in ("app", "shell"):
            for path in (ROOT / folder).glob("*.qml"):
                for value in re.findall(r'"([^"\n]*)"', path.read_text()):
                    if re.search(r"\bOmagram\b", value):
                        self.fail(f"Old display name in {path.relative_to(ROOT)}: {value}")


if __name__ == "__main__":
    unittest.main()
