import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { prepare, sha } from '../scripts/native-ui-generation.mjs';

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'telebar-generation-'));
try {
  const source = path.join(root, 'native'); fs.mkdirSync(source);
  const files = {
    'manifest.json': JSON.stringify({ id: 'io.github.pavellizunov.telebar', keepLoaded: true, entryPoints: { service: 'shell/TelebarService.qml', barWidget: 'shell/BarWidget.qml', overlay: 'shell/TelebarQuickReply.qml' } }),
    'bin/telebar-runtime': Buffer.from([127, 69, 76, 70]),
    'app/RingMark.qml': 'import QtQuick\nItem {}\n',
    'shell/TelebarService.qml': 'Item { property string binDir: String(Qt.resolvedUrl("../bin/")) }\n',
    'shell/BarWidget.qml': 'import QtQuick\nItem {}\n',
    'shell/TelebarQuickReply.qml': 'import QtQuick\nItem {}\n',
    'assets/omagram.svg': '<svg/>\n'
  };
  for (const [name, data] of Object.entries(files)) { const target = path.join(source, name); fs.mkdirSync(path.dirname(target), { recursive: true }); fs.writeFileSync(target, data); }
  function metadata() { fs.writeFileSync(path.join(source, 'runtime-candidate.json'), JSON.stringify({ runtime: 'rust', cutoverReady: true, sourceHashes: Object.fromEntries(Object.keys(files).map(name => [name, sha(fs.readFileSync(path.join(source, name)))])) })); }
  metadata(); fs.writeFileSync(path.join(source, 'README.md'), 'Native fixture\n');
  const first = prepare(source, path.join(root, 'one')), second = prepare(source, path.join(root, 'two'));
  assert.equal(first.uiGeneration, second.uiGeneration);
  assert.equal(fs.readFileSync(path.join(root, 'one', first.uiGeneration, 'app/RingMark.qml'), 'utf8'), files['app/RingMark.qml']);
  assert(fs.readFileSync(path.join(root, 'one', first.uiGeneration, 'shell/TelebarService.qml'), 'utf8').includes('String(Qt.resolvedUrl("../../bin/"))'));
  assert(fs.readFileSync(path.join(root, 'one/bin/telebar-runtime')).equals(files['bin/telebar-runtime']));
  assert.throws(() => prepare(source, path.join(root, 'one')));
  fs.writeFileSync(path.join(source, 'app/RingMark.qml'), 'import QtQuick\nItem { width: 16 }\n'); metadata();
  assert.notEqual(prepare(source, path.join(root, 'changed')).uiGeneration, first.uiGeneration);
  for (const kind of ['corrupt', 'extra', 'link']) {
    const input = path.join(root, kind); fs.cpSync(source, input, { recursive: true });
    if (kind === 'corrupt') fs.writeFileSync(path.join(input, 'app/RingMark.qml'), 'corrupt');
    if (kind === 'extra') fs.writeFileSync(path.join(input, 'extra.txt'), 'extra');
    if (kind === 'link') fs.symlinkSync('/etc/passwd', path.join(input, 'assets/escape.svg'));
    const output = path.join(root, kind + '-out'); assert.throws(() => prepare(input, output)); assert(!fs.existsSync(output));
  }
  console.log('PASS: deterministic UI generation, service root, unchanged ELF and pre-output refusals');
} finally { fs.rmSync(root, { recursive: true, force: true }); }
