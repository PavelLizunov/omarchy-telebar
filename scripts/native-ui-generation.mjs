#!/usr/bin/env node
// Immutable QML generation from an exact admitted native package. No deployment.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

export const sha = data => crypto.createHash('sha256').update(data).digest('hex');
export function inventory(root, maximumFiles = 256, maximumBytes = 96 * 1024 * 1024) {
  const files = new Map();
  let total = 0;
  function walk(directory) {
    for (const name of fs.readdirSync(directory).sort()) {
      const input = path.join(directory, name), stat = fs.lstatSync(input);
      if (stat.isSymbolicLink() || (!stat.isDirectory() && !stat.isFile())) throw Error('Unsafe package entry');
      if (stat.isDirectory()) walk(input);
      else {
        if (files.size >= maximumFiles || stat.size > 64 * 1024 * 1024) throw Error('Input inventory budget exceeded');
        const data = fs.readFileSync(input); total += data.length;
        if (total > maximumBytes) throw Error('Input byte budget exceeded');
        files.set(path.relative(root, input).split(path.sep).join('/'), data);
      }
    }
  }
  walk(root); return files;
}
export function prepare(source, output) {
  const exists = name => { try { fs.lstatSync(name); return true; } catch (error) { if (error.code !== 'ENOENT') throw error; return false; } };
  if (!path.isAbsolute(source) || !path.isAbsolute(output) || fs.lstatSync(source).isSymbolicLink()
      || exists(output) || output === source || output.startsWith(source + path.sep)) throw Error('Ordinary source and new absolute output required');
  const files = inventory(source);
  const metadata = JSON.parse(files.get('runtime-candidate.json'));
  if (metadata.runtime !== 'rust' || metadata.cutoverReady !== true) throw Error('Checked launch-enabled native package required');
  const expected = new Set([...Object.keys(metadata.sourceHashes), 'runtime-candidate.json', 'README.md']);
  if (files.size !== expected.size || [...files.keys()].some(name => !expected.has(name))) throw Error('Exact package inventory differs');
  if (!files.get('bin/telebar-runtime').subarray(0, 4).equals(Buffer.from([127, 69, 76, 70]))) throw Error('Native ELF runtime required');
  for (const [name, digest] of Object.entries(metadata.sourceHashes)) if (sha(files.get(name)) !== digest) throw Error('Input hash differs: ' + name);
  const manifest = JSON.parse(files.get('manifest.json'));
  const entries = { service: 'shell/TelebarService.qml', barWidget: 'shell/BarWidget.qml', overlay: 'shell/TelebarQuickReply.qml' };
  if (manifest.id !== 'io.github.pavellizunov.telebar' || manifest.keepLoaded !== true
      || Object.keys(manifest.entryPoints).length !== 3 || Object.entries(entries).some(([name, value]) => manifest.entryPoints[name] !== value)) throw Error('Native service identity changed');
  const visual = [...files].filter(([name]) => /^(app|shell|assets)\//.test(name)).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0);
  const uiBytes = visual.reduce((sum, [, data]) => sum + data.length, 0);
  if (uiBytes > 16 * 1024 * 1024) throw Error('UI generation byte budget exceeded');
  const identity = sha(Buffer.concat(visual.flatMap(([name, data]) => [Buffer.from(name + '\0'), Buffer.from(sha(data), 'hex')])));
  const generation = 'ui-' + identity.slice(0, 16);
  for (const [name, value] of Object.entries(entries)) manifest.entryPoints[name] = generation + '/' + value;
  files.set('manifest.json', Buffer.from(JSON.stringify(manifest, null, 2) + '\n'));
  for (const [name, data] of visual) files.set(generation + '/' + name, data);
  const servicePath = generation + '/shell/TelebarService.qml';
  const old = 'String(Qt.resolvedUrl("../bin/"))', service = files.get(servicePath).toString();
  if (service.split(old).length !== 2) throw Error('Service launcher root contract changed');
  files.set(servicePath, Buffer.from(service.replace(old, 'String(Qt.resolvedUrl("../../bin/"))')));
  metadata.uiGeneration = generation; metadata.uiGenerationSha256 = identity;
  metadata.sourceHashes = Object.fromEntries([...files].filter(([name]) => !['runtime-candidate.json', 'README.md'].includes(name)).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).map(([name, data]) => [name, sha(data)]));
  files.set('runtime-candidate.json', Buffer.from(JSON.stringify(metadata, null, 2) + '\n'));
  fs.mkdirSync(output, { mode: 0o700 });
  for (const [name, data] of files) {
    const target = path.join(output, name); fs.mkdirSync(path.dirname(target), { recursive: true }); fs.writeFileSync(target, data);
    fs.chmodSync(target, exists(path.join(source, name)) ? fs.statSync(path.join(source, name)).mode & 0o777 : 0o644);
  }
  return { uiGeneration: generation, uiGenerationSha256: identity, files: files.size, visualFiles: visual.length, uiBytes, elfUnchanged: true, serviceLauncherRootPreserved: true, installsOrReloads: false };
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length !== 4) throw Error('Usage: node scripts/native-ui-generation.mjs <native-package> <new-output>');
  console.log(JSON.stringify(prepare(process.argv[2], process.argv[3]), null, 2));
}
