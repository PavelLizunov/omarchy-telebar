import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const agents = fs.readFileSync(path.join(root, 'AGENTS.md'), 'utf8');
const guide = fs.readFileSync(path.join(root, 'TESTING.md'), 'utf8');
const preflight = path.join(root, 'scripts/test-host-preflight.sh');
assert(agents.includes('[TESTING.md](TESTING.md)'));
assert(agents.includes('scripts/test-host-preflight.sh'));
for (const name of ['README.md', 'tests/visual/README.md', 'rust/src/authorization.rs',
  'app/RoundVideoViewer.qml', 'tests/visual/tst_video_first_frame.qml']) {
  assert(fs.statSync(path.join(root, name)).isFile(), name);
}
for (const required of ['omarchy-test', 'VM107', 'sudo -n -u dsh ssh',
  'telegramdesktop/tdesktop', 'tdlib/td', 'cold first open', 'painted pixels',
  'source-checked, guest-tested, installed, loaded and user-accepted']) {
  assert(guide.includes(required), required);
}
assert(!/password\s*[:=]|192\.168\./i.test(guide), 'Access uses configured aliases, not copied addresses/passwords');
assert.equal(spawnSync('bash', ['-n', preflight]).status, 0);
const invalid = spawnSync('bash', [preflight, 'unexpected'], { encoding: 'utf8' });
assert.equal(invalid.status, 2, 'Invalid invocation fails before SSH');
assert(invalid.stderr.includes('Usage:'));
console.log('Testing workflow links, reference/acceptance contract and preflight syntax passed; not remote-runtime acceptance');
