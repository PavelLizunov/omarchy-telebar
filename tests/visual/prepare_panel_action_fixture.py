#!/usr/bin/python3
"""Actual compact panel/controller/action fixture with inert window and I/O."""
from pathlib import Path
import importlib.util
import shutil
import re
import hashlib
import json
import sys

SOURCE = Path(__file__).resolve().parents[2]


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def prepare(output):
    # Reuse the owning actual-Main admission overlay and inert native imports.
    sidebar = module('sidebar_fixture', SOURCE/'tests/visual/prepare_sidebar_fixture.py')
    sidebar.prepare(output)
    ui = output/'imports/qs/Ui'
    for name in ('Panel', 'PanelController'):
        path = Path('/usr/share/omarchy/shell/Ui')/(name+'.qml')
        data = path.read_text()
        if name == 'Panel':
            data = data.replace('import qs.Commons\n','import qs.Commons\nimport qs.Commons as Host\n')
            data = re.sub(r'(?<![\w.])Color\.', 'Host.Color.', data)
        (ui/(name+'.qml')).write_text(data)
    with (ui/'qmldir').open('a') as file:
        file.write('Panel 1.0 Panel.qml\nPanelController 1.0 PanelController.qml\nKeyboardPanel 1.0 KeyboardPanel.qml\n')
    (ui/'KeyboardPanel.qml').write_text('''import QtQuick
Item {
  property var anchorItem: null
  property var owner: null
  property var bar: null
  property bool open: false
  property bool centerOnBar: false
  property var focusTarget: null
  property bool popoutSwitching: false
  property bool popoutSwitchClosing: false
  property int contentWidth: 460
  property int contentHeight: 560
  readonly property int verticalContentInset: 0
  width: contentWidth
  height: contentHeight
  function fittedContentWidth(value) { return value }
  function fittedContentHeight(value) { return value }
  function beginFocusPrime() {}
}
''')
    # Non-registering QtTest IPC adapter; MCP uses actual static types but no IPC server.
    for relative in ('Quickshell/Quickshell.qml','Quickshell/IpcHandler.qml','Quickshell/qmldir',
                     'Quickshell/Io/Socket.qml','Quickshell/Io/Process.qml','Quickshell/Io/FileView.qml',
                     'Quickshell/Io/SplitParser.qml','Quickshell/Io/StdioCollector.qml','Quickshell/Io/qmldir',
                     'Quickshell/Hyprland/Hyprland.qml','Quickshell/Hyprland/qmldir'):
        target = output/'inert'/relative
        target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(SOURCE/'tests/visual/imports/inert'/relative,target)
    ipc = output/'inert/Quickshell/IpcHandler.qml'
    ipc.write_text('import QtQuick\nItem { property string target: "" }\n')
    shutil.copyfile(ipc,output/'inert/Quickshell/Io/IpcHandler.qml')
    with (output/'inert/Quickshell/Io/qmldir').open('a') as file:
        file.write('IpcHandler 1.0 IpcHandler.qml\n')
    fixture = output/'tests/visual'
    for name in ('PanelActionConsumer.qml','tst_panel_action.qml'):
        shutil.copyfile(SOURCE/'tests/visual'/name,fixture/name)
    hashes = {str(path.relative_to(output)):hashlib.sha256(path.read_bytes()).hexdigest()
              for path in output.rglob('*') if path.is_file()}
    (output/'action-fixture-inputs.json').write_text(json.dumps({
        'fixtureHashes':hashes,'actualPanelAndQuickView':True,'actualController':True,
        'keyboardWindowBoundaryInert':True,'commandsAndRequestsInert':True,
        'limits':['No compositor positioning or physical production click acceptance']
    },indent=2)+'\n')


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('Usage: prepare_panel_action_fixture.py <new-absolute-fixture>')
    prepare(Path(sys.argv[1]))
