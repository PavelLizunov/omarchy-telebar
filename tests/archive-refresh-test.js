const assert = require('node:assert/strict');
const { loadLogic } = require('./qml-logic');
const Model = loadLogic('Model');
const Navigation = loadLogic('MainNavigation');
const chat = (id, list, order, text = 'Before') => ({ id, title: 'Synthetic ' + id,
  positions: { [list]: { order: String(order), pinned: false } },
  lastMessage: { id: 1, date: 1792350000, text }, unread: 0 });
const main = Array.from({ length: 500 }, (_, i) => chat(i + 1, 'main', 1000 - i));
const archive = Array.from({ length: 41 }, (_, i) => chat(-100 - i, 'archive', 100 - i));
let known = Model.mergeChatUpdates(main, archive, '');
assert.equal(known.length, 541, 'A full main list must not evict archived chats');
assert.equal(Model.chatsIn(known, 'main').length, 500);
assert.equal(Model.chatsIn(known, 'archive').length, 41);
known = Model.upsertKnown(known, { ...archive[0], unread: 2 });
assert.equal(Model.chatsIn(known, 'main').length, 500);
assert.equal(Model.chatsIn(known, 'archive')[0].unread, 2);
assert.ok(Model.mergeChatUpdates([], Array.from({ length: 6000 }, (_, i) => chat(i, 'main', i + 1)), '').length <= 5000);

function scene() {
  const app = { listKey: 'main', activeAccount: 'first', accountRevision: 1,
    auth: { state: 'ready' }, loadedLists: {}, chats: main, pendingChats: {} };
  const requests = [];
  const service = { request(cmd, args, callback) { requests.push({ cmd, args, callback }); } };
  return { app, requests, select(key) { Navigation.selectList({ omagram: app, service }, key); } };
}
const s = scene();
s.select('archive');
assert.equal(s.app.listKey, 'archive');
assert.deepEqual(s.requests.map(r => r.cmd), ['chats.list', 'chats.load']);
s.requests[0].callback({ ok: true, result: { chats: archive } });
assert.equal(Model.chatsIn(s.app.chats, 'archive').length, 41, 'Hydrate cached archive even if TDLib emits no updateNewChat');
s.requests[1].callback({ ok: false, code: 404 });
assert.equal(s.requests[2].cmd, 'chats.list', 'Fully-loaded list still needs a snapshot');
s.requests[2].callback({ ok: true, result: { chats: [{ ...archive[0], unread: 3, lastMessage: { id: 2, text: 'New cached data' } }] } });
assert.equal(Model.findChat(s.app.chats, archive[0].id).lastMessage.text, 'New cached data');
s.select('archive');
assert.equal(s.requests.length, 4, 'Revisit refreshes cache without another native load');
const streamed = { ...archive[0], unread: 4, lastMessage: { id: 3, text: 'Newest stream' } };
s.app.pendingChats[streamed.id] = streamed;
s.requests[3].callback({ ok: true, result: { chats: archive } });
s.app.chats = Model.mergeChatUpdates(s.app.chats, Object.values(s.app.pendingChats), '');
assert.equal(Model.findChat(s.app.chats, streamed.id).lastMessage.id, 3);

for (const invalidate of [a => { a.activeAccount = 'second'; }, a => { a.accountRevision++; }, a => { a.loadedLists = {}; }]) {
  const stale = scene(); stale.select('archive'); invalidate(stale.app);
  stale.requests[0].callback({ ok: true, result: { chats: archive } });
  stale.requests[1].callback({ ok: true, result: {} });
  assert.equal(stale.app.chats.length, 500, 'Stale account/snapshot must not hydrate this view');
  assert.equal(stale.requests.length, 2);
}
const flushed = scene(); flushed.select('archive');
flushed.requests[0].callback({ ok: true, result: { chats: archive } });
flushed.requests[1].callback({ ok: true, result: {} });
flushed.app.chats = Model.mergeChatUpdates(flushed.app.chats, [streamed], '');
flushed.requests[2].callback({ ok: true, result: { chats: archive } });
assert.equal(Model.findChat(flushed.app.chats, streamed.id).lastMessage.id, 3, 'A late snapshot must not roll back already-flushed stream data');
const retry = scene(); retry.select('archive');
retry.requests[1].callback({ ok: false, code: 500 });
retry.select('archive');
assert.equal(retry.requests.filter(r => r.cmd === 'chats.load').length, 2, 'Failed native load must remain retryable');
console.log('Archive refresh checks passed: shared-cache capacity, hydration, revisit, stream precedence, stale-account guards, retry');
