const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

function handler(file, marker) {
  const source = fs.readFileSync(path.join(__dirname, "..", file), "utf8");
  const start = source.indexOf(marker);
  assert.ok(start >= 0);
  const begin = source.indexOf("{", start);
  let depth = 1;
  for (let i = begin + 1; i < source.length; i++) {
    if (source[i] === "{") depth++;
    if (source[i] === "}" && --depth === 0) return source.slice(begin + 1, i);
  }
  throw new Error("Unclosed handler");
}

const callbacks = [];
const topics = { chat: { id: 10 }, loading: false, serial: 1, list: [], next: null, error: "",
  client: { request(cmd, args, cb) { callbacks.push({ cmd, args, cb }); } } };
const context = vm.createContext({ topics, Model: { mergeTopics(a, b) { return a.concat(b); } }, JSON });
vm.runInContext("function fetch(more) {" + handler("app/TopicList.qml", "function fetch(") + "}", context);
context.fetch(false);
callbacks.shift().cb({ ok: false, error: "Telegram temporarily unavailable" });
assert.equal(topics.error, "Telegram temporarily unavailable", "A load failure must not look like an empty forum");
assert.equal(topics.loading, false);
assert.equal(topics.retryMore, false);

context.fetch(false);
const old = callbacks.shift();
topics.chat = null;
old.cb({ ok: true, result: { topics: [{ id: 77 }] } });
assert.equal(topics.list.length, 0, "A response for a chat left behind must not populate the list");

topics.chat = { id: 20 };
topics.loading = false;
topics.next = { offsetDate: 1, offsetMessageId: 2, offsetTopicId: 3 };
context.fetch(true);
callbacks.shift().cb({ ok: false, error: "Page failed" });
assert.equal(topics.retryMore, true, "Retry must request the failed page, not the first page");
context.fetch(topics.retryMore);
callbacks.shift().cb({ ok: true, result: { topics: [{ id: 4 }],
  next: { offsetDate: 1, offsetMessageId: 2, offsetTopicId: 3 } } });
assert.equal(topics.next, null, "Repeated cursor must not request the same page forever");
console.log("Forum UI checks passed: visible errors, stale chat replies, pagination progress");

const model = vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(__dirname, "../app/Model.js"), "utf8").replace(/^\.pragma library\s*$/m, ""), model);
const read = [];
const omagram = { openChat: { id: -10, unread: 0, forum: true },
  openTopic: { id: 5, chatId: -10, unread: 4 }, windowFocused: true,
  openKey: "-10:5", messages: { "-10:5": [{ id: 9, outgoing: false }, { id: 10, outgoing: true }] },
  markRead(...args) { read.push(args); } };
const reading = vm.createContext({ omagram, Model: model });
vm.runInContext("function markOpenChatRead() {" + handler("app/Main.qml", "function markOpenChatRead(") + "}", reading);
reading.markOpenChatRead();
assert.deepEqual(JSON.parse(JSON.stringify(read)), [[-10, [9], 5, false]],
  "A topic must be read even when its parent chat has no unread messages");
read.length = 0;
omagram.windowFocused = false;
reading.markOpenChatRead();
assert.equal(read.length, 0, "An unfocused window must not mark a topic read");
console.log("Forum read checks passed: per-topic reading and focus guard");

const pendingHistory = [];
const quick = { opened: true, shownChatId: -10, historySerial: 2, history: [],
  service: { activeAccount: "default", request(cmd, args, cb) { pendingHistory.push(cb); } } };
const history = vm.createContext({ quick, Model: model, Qt: { callLater() {} }, messageList: { positionViewAtEnd() {} } });
vm.runInContext("function fetchHistory(chatId, fromMessageId, serial) {" + handler("shell/QuickView.qml", "function fetchHistory(") + "}", history);
for (const change of [() => quick.opened = false, () => quick.shownChatId = -20,
  () => quick.service.activeAccount = "work", () => quick.historySerial++]) {
  quick.opened = true;
  quick.shownChatId = -10;
  quick.service.activeAccount = "default";
  history.fetchHistory(-10, 0, quick.historySerial);
  change();
  pendingHistory.shift()({ ok: true, result: { messages: [{ id: 9, chatId: -10 }] } });
  assert.equal(quick.history.length, 0, "A stale history response must not populate or read the current chat");
}
console.log("Quick history checks passed: close, chat, account and generation guards");
