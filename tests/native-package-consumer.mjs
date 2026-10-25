// Exact native package/shim/EOF and compatible disposable socket consumer.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import net from 'node:net';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { sha, verify } from '../scripts/native-ui-generation.mjs';

function lines(stream) {
  const queue = []; let buffer = '', waiting = null, ended = false;
  stream.on('data', chunk => {
    buffer += chunk.toString(); assert(buffer.length < 2 * 1024 * 1024);
    while (buffer.includes('\n')) { const split = buffer.indexOf('\n'); queue.push(JSON.parse(buffer.slice(0, split))); buffer = buffer.slice(split + 1); }
    if (waiting && queue.length) { const resolve = waiting; waiting = null; resolve(queue.shift()); }
  });
  stream.on('end', () => { ended = true; if (waiting) { const resolve = waiting; waiting = null; resolve(null); } });
  return () => new Promise((resolve, reject) => {
    if (queue.length) return resolve(queue.shift()); if (ended) return reject(Error('Stream ended before response'));
    const timer = setTimeout(() => { waiting = null; reject(Error('Bounded line deadline')); }, 5000);
    waiting = value => { clearTimeout(timer); value === null ? reject(Error('Stream ended')) : resolve(value); };
  });
}
function child(argv, environment) {
  const process = spawn(argv[0], argv.slice(1), { env: environment, stdio: ['pipe', 'pipe', 'pipe'] });
  const completed = once(process, 'exit'); let errors = '';
  process.stderr.on('data', data => { errors += data.toString(); assert(errors.length <= 1024 * 1024); });
  return { process, completed, get errors() { return errors; } };
}
async function wait(owner, milliseconds) {
  let timer;
  try { return await Promise.race([owner.completed, new Promise((_, reject) => { timer = setTimeout(() => reject(Error('Owned child did not retire')), milliseconds); })]); }
  finally { clearTimeout(timer); }
}
const packageRoot = process.argv[2];
assert(process.env.TELEBAR_ISOLATED_PACKAGE === '1');
assert(Object.keys(os.networkInterfaces()).every(name => name === 'lo'));
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'telebar-native-package-')); fs.chmodSync(root, 0o700);
const env = { PATH: '/usr/bin', HOME: root, LANG: 'C.UTF-8' }; let proxy, runtime, socket;
try {
  const manifest = JSON.parse(fs.readFileSync(path.join(packageRoot, 'runtime-candidate.json')));
  assert(manifest.runtime === 'rust' && manifest.cutoverReady === true);
  if (manifest.uiGeneration) assert.equal(verify(packageRoot).uiGeneration, manifest.uiGeneration);
  for (const [name, digest] of Object.entries(manifest.sourceHashes)) assert.equal(sha(fs.readFileSync(path.join(packageRoot, name))), digest);
  const marker = path.join(root, 'unexpected-startup'), poison = path.join(root, 'bash-env');
  fs.writeFileSync(poison, `touch ${marker}\nexit 71\n`);
  for (const [name, args] of [['omagram', ['--invalid']], ['omagramd', ['--invalid']], ['omagram-build-tdlib', ['--bad']], ['omagram-menu-install', ['bad']]]) {
    const result = spawnSync(path.join(packageRoot, 'bin', name), args, { env: { ...env, BASH_ENV: poison }, timeout: 4000 });
    assert.equal(result.status, 1, name); assert(!fs.existsSync(marker));
  }
  proxy = child([path.join(packageRoot, 'bin/omagram-proxy')], env);
  const nextProxy = lines(proxy.process.stdout); proxy.process.stdin.write('{"secret":"0123456789abcdef0123456789abcdef"}\n');
  const ready = await nextProxy(); assert(ready.event === 'ready' && ready.port > 0 && ready.port <= 65535);
  assert.equal(fs.realpathSync(`/proc/${proxy.process.pid}/exe`), path.join(packageRoot, 'bin/telebar-runtime'));
  proxy.process.stdin.end(); assert.deepEqual(await wait(proxy, 4000), [0, null]);
  const data = path.join(root, '.local/share/omagram'); fs.mkdirSync(data, { recursive: true, mode: 0o700 });
  fs.writeFileSync(path.join(data, 'accounts.json'), JSON.stringify({ active: 'other', accounts: [{ id: 'other', name: 'First', phone: 'fixture', meId: 7 }, { id: 'default', name: 'Second' }] }), { mode: 0o600 });
  runtime = child([path.join(packageRoot, 'bin/telebar-runtime'), '--inert-no-library', root], env);
  const nextStartup = lines(runtime.process.stdout); assert.equal((await nextStartup()).production_profiles, false);
  socket = net.createConnection(path.join(root, 'migration.sock')); await once(socket, 'connect');
  const next = lines(socket); const captured = [];
  async function request(id, cmd, args = {}) {
    socket.write(JSON.stringify({ id, cmd, args }) + '\n');
    for (let count = 0; count < 50; count++) { const result = await next(); captured.push(result); if (result.id === id) return result; }
    throw Error('Response frame budget');
  }
  const hello = await request(1, 'hello'); assert(hello.ok);
  assert.equal(hello.result.activeAccount, 'other'); assert.equal(hello.result.meId, 7);
  assert.deepEqual(hello.result.accounts.map(row => row.id), ['other', 'default']); assert.equal(hello.result.auth.state, 'noLibrary');
  const required = ['version', 'auth', 'meId', 'chats', 'open', 'folders', 'mainPosition', 'settings', 'globalStatus', 'calls', 'connection', 'accounts', 'activeAccount', 'accountRevision', 'recordedSend'];
  assert.deepEqual(Object.keys(hello.result).sort(), required.sort());
  for (const [id, cmd, args] of [[20, 'auth.phone', { phone: '+12025550123' }], [21, 'proxies.list', {}], [22, 'account.logout', {}]]) assert.equal((await request(id, cmd, args)).error, 'Telegram is not available');
  assert.equal((await request(23, 'hello')).result.auth.state, 'noLibrary');
  const window = (await request(2, 'hello', { window: true })).result;
  assert.deepEqual(Object.keys(window).sort(), [...required, 'allChats', 'stories'].sort()); assert.deepEqual(window.allChats, []);
  assert.equal((await request(3, 'account.switch', { id: 'default' })).result.activeAccount, 'default');
  assert.equal((await request(4, 'chat.history', { chatId: 42 })).error, 'not signed in');
  const changed = await request(6, 'settings.set', { settings: { globalShortcuts: { 'global.panel': 'SUPER + P' } } }); assert(changed.ok); assert.equal(changed.result.globalStatus['global.panel'], 'unavailable');
  assert.equal((await request(7, 'hello')).result.settings.globalShortcuts['global.panel'], 'SUPER + P');
  assert(captured.some(frame => frame.event === 'settings' && frame.globalStatus['global.panel'] === 'unavailable'));
  const ordered = [];
  socket.write([[8, 'settings.set', { settings: { globalShortcuts: { 'global.openWindow': 'CTRL + O' } } }], [9, 'settings.quiet', { quiet: true }], [10, 'shortcuts.apply', {}]].map(([id, cmd, args]) => JSON.stringify({ id, cmd, args }) + '\n').join(''));
  for (let count = 0; count < 50; count++) { const frame = await next(); captured.push(frame); ordered.push(frame); if (frame.id === 10) break; }
  const replies = ordered.filter(frame => [8, 9, 10].includes(frame.id)); assert.deepEqual(replies.map(frame => frame.id), [8, 9, 10]); assert(replies.every(frame => frame.ok));
  assert.equal(replies[0].result.settings.quiet, false); assert.equal(replies[1].result.settings.quiet, true); assert.deepEqual(replies[2].result, replies[1].result);
  assert(ordered.findIndex(frame => frame.id === 8) < ordered.findIndex(frame => frame.event === 'settings' && frame.settings.globalShortcuts['global.openWindow'] === 'CTRL + O'));
  assert((await request(24, 'diagnostics.ui', { window: true, pending: 3, password: 'PRIVATE-CONTENT', files: 'PRIVATE-CONTENT', width: -1, height: 1e100 })).ok);
  assert.equal((await request(25, 'diagnostics.ui', { pending: 99 })).error, 'diagnostic reports are rate limited');
  const diagnostic = (await request(26, 'diagnostics.status')).result;
  assert.equal(diagnostic.clients, 1); assert.equal(diagnostic.account_count, 2); assert.equal(diagnostic.pending_ipc, 0); assert.equal(diagnostic.process.pid, runtime.process.pid); assert.equal(diagnostic.migration_complete, false);
  assert(diagnostic.ui.some(row => row.surface === 'window' && JSON.stringify(row.metrics) === '{"pending":3}'));
  assert(!JSON.stringify(diagnostic).includes('PRIVATE-CONTENT')); assert((await request(27, 'hello')).ok); assert((await request(5, 'app.quit')).ok);
  socket.end(); assert.deepEqual(await wait(runtime, 7000), [0, null]); assert(!fs.existsSync(path.join(root, 'migration.sock')));
  assert(!fs.existsSync(path.join(data, 'database')));
  const trace = fs.readFileSync(path.join(root, '.local/state/omagram/diagnostics/trace.jsonl'), 'utf8'); assert(!trace.includes('PRIVATE-CONTENT'));
  assert(trace.split('\n').filter(Boolean).some(line => JSON.parse(line).event === 'ui_sample'));
  console.log(JSON.stringify({ nativeShims: true, ownerEofShutdown: true, bashEnvIgnored: true, noLibrarySocket: true, frames: captured.length, profileOpened: false, parametersSent: false }));
} finally {
  socket?.destroy();
  for (const owner of [proxy, runtime]) if (owner && owner.process.exitCode === null && owner.process.signalCode === null) { owner.process.kill('SIGTERM'); await wait(owner, 7000); }
  fs.rmSync(root, { recursive: true, force: true });
}
