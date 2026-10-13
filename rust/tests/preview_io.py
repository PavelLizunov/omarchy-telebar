#!/usr/bin/python3
"""Build an explicit offscreen I/O overlay; never edit/deploy product sources.

Commons' local InertIo import does not intercept product Quickshell.Io imports.
Keep native Socket/SplitParser; adapt only reviewed I/O bindings and blank the
client's default socket until the fixture supplies its explicit private path.
"""
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys

ADAPTERS = {
    "PreviewIo/qmldir": "module PreviewIo\nProcess 1.0 Process.qml\nFileView 1.0 FileView.qml\n",
    "PreviewIo/Process.qml": """import QtQuick
Item {
  readonly property bool previewIoInert: true
  readonly property int executions: 0
  property bool running: false
  property var command: []
  property var environment: ({})
  property bool clearEnvironment: false
  property bool stdinEnabled: false
  property bool waitForEnd: false
  property var stdout: null
  property var stderr: null
  signal started()
  signal exited(int code)
  function write(s) {}
  function signal(sig) {}
}
""",
    "PreviewIo/FileView.qml": """import QtQuick
Item {
  readonly property bool previewIoInert: true
  property string path: ""
  property bool watchChanges: false
  property bool printErrors: false
  signal loaded()
  signal loadFailed()
  signal fileChanged()
  function reload() {}
  function text() { return "" }
}
""",
}


def rewrite(relative, text):
    if relative in ("app/OmagramClient.qml", "app/TelebarClient.qml"):
        default = 'property string socketPath: client.runtimeDir.charAt(0) === "/" ? client.runtimeDir + "/omagram/omagram.sock" : ""'
        if text.count(default) != 1:
            raise ValueError("client socket default contract changed")
        changed = text.replace(default, 'property string socketPath: ""')
        if changed.replace('property string socketPath: ""', default) != text:
            raise ValueError("non-default client mutation")
        return changed
    expected = {"shell/Service.qml": ("Process", 2),
                "shell/TelebarService.qml": ("Process", 2),
                "app/EmojiPanel.qml": ("FileView", 3)}
    match = re.findall(r"(?<![\w.])(Process|FileView)\s*\{", text)
    if relative not in expected:
        if match:
            raise ValueError("unreviewed Process/FileView consumer: " + relative)
        return text
    kind, count = expected[relative]
    if match != [kind] * count or text.count("import Quickshell.Io\n") != 1:
        raise ValueError("source I/O contract changed: " + relative)
    changed = text.replace("import Quickshell.Io\n", "import Quickshell.Io\nimport PreviewIo as PreviewIo\n")
    changed = changed.replace(kind + " {", "PreviewIo." + kind + " {")
    if changed.replace("import PreviewIo as PreviewIo\n", "").replace("PreviewIo." + kind + " {", kind + " {") != text:
        raise ValueError("non-import/type source mutation")
    if re.search(r"(?<![\w.])(Process|FileView)\s*\{", changed):
        raise ValueError("native I/O remained")
    return changed


def assemble(source, output):
    source, output = Path(source), Path(output)
    if not source.is_absolute() or not output.is_absolute() or output.exists() or output.is_symlink() or output.is_relative_to(source):
        raise ValueError("explicit source/new output required")
    inputs = []
    for directory in ("app", "shell", "assets"):
        for path in sorted((source / directory).rglob("*")):
            if path.is_symlink() or not (path.is_dir() or path.is_file()):
                raise ValueError("unsafe source entry")
            if path.is_file():
                if len(inputs) >= 256 or path.stat().st_size > 16 * 1024 * 1024:
                    raise ValueError("preview input budget")
                data = path.read_bytes()
                relative = path.relative_to(source).as_posix()
                changed = rewrite(relative, data.decode()).encode() if path.suffix == ".qml" else data
                inputs.append((relative, data, changed))
    # Review/admit every input before creating any output; this is an overlay,
    # not a packaged install. Python/runtime entry points are deliberately absent.
    output.mkdir(mode=0o700)
    hashes = {}
    for relative, before, after in inputs:
        path = output / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(after)
        hashes[relative] = {"sourceSha256": hashlib.sha256(before).hexdigest(),
                            "previewSha256": hashlib.sha256(after).hexdigest(),
                            "adapted": before != after}
    for relative, text in ADAPTERS.items():
        path = output / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    fixtures = output / "rust/tests"
    fixtures.mkdir(parents=True)
    for name in ("MainConsumer.qml", "MainSendConsumer.qml", "ServiceConsumer.qml"):
        shutil.copyfile(source / "rust/tests" / name, fixtures / name)
    (output / "preview-source.json").write_text(json.dumps(hashes, indent=2) + "\n")
    return hashes


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("Usage: preview_io.py <reviewed-source> <new-absolute-preview-root>")
    hashes = assemble(*sys.argv[1:])
    print(json.dumps({"files": len(hashes), "adapted": [p for p, v in hashes.items() if v["adapted"]],
                      "nativeSocketUnchanged": True, "productSourcesUnchanged": True}))
