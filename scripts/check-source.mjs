#!/usr/bin/env node
// Current-source/runtime contracts; negative-package guards remain intentional.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
let files = 0;
function walk(directory) {
  for (const name of fs.readdirSync(directory).sort()) {
    if (name === '.git') continue;
    const file = path.join(directory, name), stat = fs.lstatSync(file);
    assert(!stat.isSymbolicLink(), 'Source symlink: ' + file);
    if (stat.isDirectory()) walk(file);
    else {
      assert(stat.isFile(), 'Special source entry: ' + file);
      assert(!/\.(?:py|pyc|pyo)$/i.test(name), 'Retired source file: ' + file);
      files++;
      if (/\.(?:md|qml|js|mjs|sh|yml|json)$/.test(name) || path.dirname(file) === path.join(root, 'bin')) {
        const text = fs.readFileSync(file, 'utf8');
        assert(!/^#![^\n]*python|^\s*(?:run:\s*)?(?:\/usr\/bin\/)?python\d?\b/im.test(text), 'Retired executable invocation: ' + file);
        if (file.endsWith('.md')) for (const match of text.matchAll(/\]\(([^)]+)\)/g)) {
          const target = match[1]; if (target.includes('://') || target.startsWith('#')) continue;
          assert(fs.existsSync(path.resolve(path.dirname(file), target.split('#')[0])), 'Broken guide link: ' + file + ' ' + target);
        }
      }
    }
  }
}
walk(root);
const manifest = JSON.parse(fs.readFileSync(path.join(root, 'manifest.json')));
assert.equal(manifest.id, 'io.github.pavellizunov.telebar');
assert.deepEqual(manifest.kinds, ['service', 'bar-widget', 'overlay']);
for (const name of Object.values(manifest.entryPoints)) {
  assert(!path.isAbsolute(name) && !name.split('/').includes('..'));
  assert(fs.statSync(path.join(root, name)).isFile());
}
for (const [name, mode] of Object.entries({ omagram: '--launch', omagramd: '--daemon', 'omagram-menu-install': '--menu', 'omagram-build-tdlib': '--build-tdlib', 'omagram-proxy': '--proxy' })) {
  const body = fs.readFileSync(path.join(root, 'bin', name), 'utf8');
  assert(body.startsWith('#!/usr/bin/bash -p\n')); assert(body.includes('telebar-runtime" ' + mode));
}
const mark = fs.readFileSync(path.join(root, 'app/RingMark.qml'), 'utf8');
const svg = fs.readFileSync(path.join(root, 'assets/omagram.svg'), 'utf8');
const contour = mark.match(/PathSvg\s*\{\s*path:\s*"([^"]+)"/);
assert(contour && svg.includes(contour[1]), 'QML/SVG mark must share its original contour');
console.log('PASS: native source/entry contracts, current guide links and exact mark; ' + files + ' regular files');
