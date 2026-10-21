#!/usr/bin/env node
// Reviewed actual QML with explicit inert I/O and window boundaries; not a sandbox.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { inventory, sha } from './native-ui-generation.mjs';
const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const host = '/usr/share/omarchy/shell';
const io = {
  'PreviewIo/qmldir': 'module PreviewIo\nProcess 1.0 Process.qml\nFileView 1.0 FileView.qml\n',
  'PreviewIo/Process.qml': `import QtQuick
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
`,
  'PreviewIo/FileView.qml': `import QtQuick
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
`
};
export function rewrite(name, text) {
  if (name === "shell/MediaWindow.qml") {
    if (text.split("  visible: true\n").length !== 2) throw Error("Media surface visibility contract changed");
    return text.replace("  visible: true\n", "  visible: false\n");
  }
  if (name === 'app/TelebarClient.qml') {
    const before = 'property string socketPath: client.runtimeDir.charAt(0) === "/" ? client.runtimeDir + "/omagram/omagram.sock" : ""';
    if (text.split(before).length !== 2) throw Error('Client default socket contract changed');
    return text.replace(before, 'property string socketPath: ""');
  }
  const expected = { 'shell/TelebarService.qml': ['Process', 2], 'app/EmojiPanel.qml': ['FileView', 3] };
  const found = [...text.matchAll(/(?<![\w.])(Process|FileView)\s*\{/g)].map(match => match[1]);
  if (!expected[name]) { if (found.length) throw Error('Unreviewed I/O consumer: ' + name); return text; }
  const [kind, count] = expected[name];
  if (found.length !== count || found.some(value => value !== kind) || text.split('import Quickshell.Io\n').length !== 2) throw Error('Product I/O contract changed: ' + name);
  return text.replace('import Quickshell.Io\n', 'import Quickshell.Io\nimport PreviewIo as PreviewIo\n').replaceAll(kind + ' {', 'PreviewIo.' + kind + ' {');
}
export function prepare(mode, output) {
  if (!['icon', 'action'].includes(mode) || !path.isAbsolute(output)) throw Error('Explicit mode and new absolute output required');
  try { fs.lstatSync(output); throw Error('Output already exists'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const files = new Map(), records = [];
  function admit(name, source, transform = value => value) {
    const stat = fs.lstatSync(source); if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 16 * 1024 * 1024) throw Error('Unsafe fixture input');
    const before = fs.readFileSync(source), after = Buffer.from(transform(before.toString())); files.set(name, after);
    records.push({ input: source, inputSha256: sha(before), output: name, outputSha256: sha(after) });
  }
  for (const directory of ['app', 'shell', 'assets']) for (const [name, data] of inventory(path.join(root, directory))) {
    const relative = directory + '/' + name;
    files.set(relative, name.endsWith('.qml') ? Buffer.from(rewrite(relative, data.toString())) : data);
    records.push({ input: path.join(root, relative), inputSha256: sha(data), output: relative, outputSha256: sha(files.get(relative)) });
  }
  for (const [name, data] of inventory(path.join(root, 'tests/visual'))) if (!name.startsWith('imports/') && /\.(qml|svg|js|txt)$/.test(name)) files.set('tests/visual/' + name, data);
  const inert = path.join(root, 'tests/visual/imports/inert');
  for (const [name, data] of inventory(inert)) files.set('inert/' + name, data);
  const inertIo = data => {
    if (data.split('import Quickshell.Io\n').length !== 2) throw Error('Commons I/O contract changed');
    return data.replace('import Quickshell.Io\n', 'import "InertIo"\n');
  };
  for (const name of ['Color.qml', 'Style.qml', 'Util.qml', 'Border.qml', 'BorderGeometry.js', 'qmldir']) admit('imports/qs/Commons/' + name, path.join(host, 'Commons', name), ['Color.qml', 'Style.qml'].includes(name) ? inertIo : name === 'qmldir' ? text => 'module qs.Commons\n' + text.split('\n').filter(line => /^singleton (Color|Style|Util|Border) /.test(line)).join('\n') + '\n' : undefined);
  for (const name of ['Process.qml', 'FileView.qml', 'StdioCollector.qml']) admit('imports/qs/Commons/InertIo/' + name, path.join(inert, 'Quickshell/Io', name));
  files.set('imports/qs/Commons/InertIo/qmldir', Buffer.from('module InertIo\nProcess 1.0 Process.qml\nFileView 1.0 FileView.qml\nStdioCollector 1.0 StdioCollector.qml\n'));
  function qualified(data) {
    if (data.split('import qs.Commons\n').length !== 2) throw Error('Native Commons import changed');
    return data.replace('import qs.Commons\n', 'import qs.Commons\nimport qs.Commons as Host\n').replace(/(?<![\w.])Color\./g, 'Host.Color.');
  }
  const controls = ['BarIconButton', 'WidgetButton', 'OpticalGlyph', 'BarWidget'];
  for (const name of controls) admit('imports/qs/Ui/' + name + '.qml', path.join(host, 'Ui', name + '.qml'), qualified);
  files.set('imports/qs/Ui/PopupCard.qml', Buffer.from(`import QtQuick
Item {
 property var anchorItem: null; property var owner: null; property var bar: null
 property bool open: false; property int padding: 0; property int contentWidth: 1; property int contentHeight: 1
 visible: open
 function fittedContentWidth(value) { return value }
 function fittedContentHeight(value) { return value }
}
`));
  if (mode === 'icon') files.set('shell/Panel.qml', Buffer.from(`import QtQuick
Item {
 property var bar: null; property var settings: ({}); property var anchorItem: null
 property var hostWidget: null; property var omagram: null; property bool opened: false
 property bool popoutSwitchClosing: false
 function toggle() { opened = !opened }
 function open() { opened = true }
 function close() { opened = false }
 function closeForPopoutSwitch() { opened = false }
}
`));
  else {
    admit('imports/qs/Ui/Panel.qml', path.join(host, 'Ui/Panel.qml'), qualified);
    admit('imports/qs/Ui/PanelController.qml', path.join(host, 'Ui/PanelController.qml'));
    controls.push('Panel', 'PanelController', 'KeyboardPanel');
    files.set('imports/qs/Ui/KeyboardPanel.qml', Buffer.from(`import QtQuick
Item {
 property var anchorItem: null; property var owner: null; property var bar: null
 property bool open: false; property bool centerOnBar: false; property var focusTarget: null
 property bool popoutSwitching: false; property bool popoutSwitchClosing: false
 property int contentWidth: 460; property int contentHeight: 560
 readonly property int verticalContentInset: 0
 width: contentWidth; height: contentHeight
 function fittedContentWidth(value) { return value }
 function fittedContentHeight(value) { return value }
 function beginFocusPrime() {}
}
`));
    const ipc = Buffer.from('import QtQuick\nItem { property string target: "" }\n');
    files.set('inert/Quickshell/IpcHandler.qml', ipc); files.set('inert/Quickshell/Io/IpcHandler.qml', ipc);
    files.set('inert/Quickshell/Io/qmldir', Buffer.concat([files.get('inert/Quickshell/Io/qmldir'), Buffer.from('IpcHandler 1.0 IpcHandler.qml\n')]));
  }
  files.set('imports/qs/Ui/qmldir', Buffer.from('module qs.Ui\n' + controls.map(name => name + ' 1.0 ' + name + '.qml\n').join('') + 'PopupCard 1.0 PopupCard.qml\n'));
  files.set('imports/qs/shell.qml', Buffer.from('import QtQuick\nItem {}\n'));
  for (const [name, data] of Object.entries(io)) files.set(name, Buffer.from(data));
  if (files.size > 256 || [...files.values()].reduce((sum, data) => sum + data.length, 0) > 16 * 1024 * 1024) throw Error('Fixture budget exceeded');
  fs.mkdirSync(output, { mode: 0o700 });
  for (const [name, data] of files) { const target = path.join(output, name); fs.mkdirSync(path.dirname(target), { recursive: true }); fs.writeFileSync(target, data); }
  const record = { mode, files: files.size, explicitIoInert: true, blankDefaultSocket: true, actualUiPreserved: true, inputs: records, limits: ['No host compositor, account I/O or device/physical-input acceptance'] };
  fs.writeFileSync(path.join(output, 'fixture-inputs.json'), JSON.stringify(record, null, 2) + '\n');
  return record;
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length !== 4) throw Error('Usage: node scripts/prepare-preview.mjs <icon|action> <new-fixture>');
  const { inputs, ...summary } = prepare(process.argv[2], process.argv[3]); console.log(JSON.stringify(summary, null, 2));
}
