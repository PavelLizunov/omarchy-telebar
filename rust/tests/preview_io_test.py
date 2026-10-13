#!/usr/bin/python3
"""Finite source-overlay controls; native rendering is a separate consumer check."""
import importlib.util
from pathlib import Path
import tempfile

spec = importlib.util.spec_from_file_location("preview_io", Path(__file__).with_name("preview_io.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
service = "import QtQuick\nimport Quickshell.Io\nItem {\n Process { running: true; command: [\"foreign\"] }\n Process { running: false }\n}\n"
default = 'property string socketPath: client.runtimeDir.charAt(0) === "/" ? client.runtimeDir + "/omagram/omagram.sock" : ""'
client = "import Quickshell.Io\nItem { " + default + "\n Component { Socket { parser: SplitParser {} } } }\n"
blank = module.rewrite("app/TelebarClient.qml", client)
assert blank.replace('property string socketPath: ""', default) == client
assert "Socket { parser: SplitParser {} }" in blank and 'omagram.sock' not in blank
adapted = module.rewrite("shell/TelebarService.qml", service)
assert adapted.count("PreviewIo.Process {") == 2 and "running: true" in adapted
emoji = "import Quickshell.Io\nItem { FileView {} FileView {} FileView {} }"
assert module.rewrite("app/EmojiPanel.qml", emoji).count("PreviewIo.FileView {") == 3
for path, text in (("shell/NewService.qml", service), ("shell/TelebarService.qml", service.replace(" Process {", " Process  {", 1)),
                   ("shell/TelebarService.qml", service.replace(" Process { running: false }", "")),
                   ("app/NewFile.qml", "Item { FileView {} }"),
                   ("app/TelebarClient.qml", client.replace(default, 'property string socketPath: "/foreign"'))):
    try:
        module.rewrite(path, text)
    except ValueError:
        pass
    else:
        raise AssertionError("unsafe or changed source admitted: " + path)
with tempfile.TemporaryDirectory(prefix="telebar-preview-io-check-") as name:
    root = Path(name)
    source = root / "source"
    for part in ("app", "shell", "assets", "rust/tests"):
        (source / part).mkdir(parents=True, exist_ok=True)
    (source / "shell/TelebarService.qml").write_text(service)
    (source / "app/TelebarClient.qml").write_text(client)
    (source / "app/EmojiPanel.qml").write_text(emoji)
    for fixture in ("MainConsumer.qml", "MainSendConsumer.qml", "ServiceConsumer.qml"):
        (source / "rust/tests" / fixture).write_text("import QtQuick\nItem {}\n")
    records = module.assemble(source, root / "preview")
    assert sum(v["adapted"] for v in records.values()) == 3
    assert (source / "shell/TelebarService.qml").read_text() == service
    assert not (root / "preview/bin").exists()
    (source / "shell/New.qml").write_text("Item { Process {} }")
    try:
        module.assemble(source, root / "rejected")
    except ValueError:
        assert not (root / "rejected").exists()
    else:
        raise AssertionError("unreviewed operation created output")
print("Preview IO: exact I/O type bindings/blank default socket, native Socket unchanged, five refusal controls, admission before output PASS")
