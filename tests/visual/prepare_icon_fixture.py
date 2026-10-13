#!/usr/bin/python3
"""Prepare a new actual-icon fixture with explicit inert I/O/window boundaries."""
from pathlib import Path
import hashlib
import json
import re
import sys

SOURCE = Path(__file__).resolve().parents[2]
NATIVE = Path('/usr/share/omarchy/shell')
MAX_BYTES = 2 * 1024 * 1024


def read(path):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > MAX_BYTES:
        raise ValueError('Unsafe or oversized fixture input: ' + str(path))
    return path.read_bytes()


def prepare(output):
    if not output.is_absolute() or output.exists() or output.is_symlink():
        raise ValueError('Fixture output must be a new absolute directory')
    files = {}
    provenance = []

    def admit(relative, path, transform=None):
        original = read(path)
        payload = transform(original) if transform else original
        files[relative] = payload
        provenance.append({'input':str(path),'inputSha256':hashlib.sha256(original).hexdigest(),
                           'output':relative,'outputSha256':hashlib.sha256(payload).hexdigest(),
                           'transform':transform.__name__ if transform else 'unchanged'})

    for relative in ['app/RingMark.qml','shell/BarWidget.qml','shell/BarMenu.qml','assets/omagram.svg',
                     'tests/visual/RingMarkConsumer.qml','tests/visual/BarIconConsumer.qml',
                     'tests/visual/tst_ring_mark.qml','tests/visual/tst_bar_icon.qml']:
        admit(relative, SOURCE/relative)

    def inert_io(data):
        old = b'import Quickshell.Io\n'
        if data.count(old) != 1:
            raise ValueError('Native Commons I/O contract changed')
        return data.replace(old,b'import "InertIo"\n')

    for name in ['Color.qml','Style.qml','Util.qml','Border.qml','BorderGeometry.js','qmldir']:
        admit('imports/qs/Commons/'+name,NATIVE/'Commons'/name,
              inert_io if name in ('Color.qml','Style.qml') else None)
    # QML-preview can load native Quickshell ahead of same-name test modules.
    # Bind Commons I/O to this unique relative module instead of presumed shadowing.
    for name in ['Process.qml','FileView.qml','StdioCollector.qml']:
        admit('imports/qs/Commons/InertIo/'+name,SOURCE/'tests/visual/imports/inert/Quickshell/Io'/name)
    files['imports/qs/Commons/InertIo/qmldir'] = b'module InertIo\nProcess 1.0 Process.qml\nFileView 1.0 FileView.qml\nStdioCollector 1.0 StdioCollector.qml\n'

    def qualified_color(data):
        text = data.decode()
        if text.count('import qs.Commons\n') != 1:
            raise ValueError('Native UI Commons contract changed')
        text = text.replace('import qs.Commons\n','import qs.Commons\nimport qs.Commons as Host\n')
        return re.sub(r'(?<![A-Za-z0-9_.])Color\.', 'Host.Color.', text).encode()

    names = ['BarIconButton','WidgetButton','OpticalGlyph','BarWidget']
    for name in names:
        admit('imports/qs/Ui/'+name+'.qml',NATIVE/'Ui'/(name+'.qml'),qualified_color)
    files['imports/qs/Ui/qmldir'] = ('module qs.Ui\n'+''.join(name+' 1.0 '+name+'.qml\n' for name in names)+'PopupCard 1.0 PopupCard.qml\n').encode()
    # Window placement is not in scope: never construct real popup/keyboard surfaces.
    files['imports/qs/Ui/PopupCard.qml'] = b'''import QtQuick
Item {
  property var anchorItem: null
  property var owner: null
  property var bar: null
  property bool open: false
  property int padding: 0
  property int contentWidth: 1
  property int contentHeight: 1
  visible: open
  function fittedContentWidth(value) { return value }
  function fittedContentHeight(value) { return value }
}
'''
    files['shell/Panel.qml'] = b'''import QtQuick
Item {
  property var bar: null
  property var settings: ({})
  property var anchorItem: null
  property var hostWidget: null
  property var omagram: null
  property bool opened: false
  property bool popoutSwitchClosing: false
  function toggle() { opened = !opened }
  function open() { opened = true }
  function close() { opened = false }
  function closeForPopoutSwitch() { opened = false }
}
'''
    files['imports/qs/shell.qml'] = b'import QtQuick\nItem {}\n'
    # These imports are needed by dynamic QtTest only, not the static MCP renderer.
    for name in ['Quickshell.qml','IpcHandler.qml','qmldir']:
        admit('inert/Quickshell/'+name,SOURCE/'tests/visual/imports/inert/Quickshell'/name)
    if len(files) > 32 or sum(map(len,files.values())) > MAX_BYTES:
        raise ValueError('Fixture inventory exceeds bounds')
    output.mkdir(mode=0o700)
    for relative,payload in files.items():
        path = output/relative
        path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(payload)
    (output/'fixture-inputs.json').write_text(json.dumps({'inputs':provenance,
        'actualBarAndMarkUnchanged':True,'inertIoBoundExplicitly':True,
        'inertPanelPopupOnly':True,'nativeGeometryHandlersPreserved':True,
        'limits':['No compositor/popup placement/screen-reader or production-service acceptance']},indent=2)+'\n')


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('Usage: prepare_icon_fixture.py <new-absolute-fixture>')
    prepare(Path(sys.argv[1]))
