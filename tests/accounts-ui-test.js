// Exercise the actual QML JavaScript handlers without touching a live shell/session.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const root = path.join(__dirname, "..");

function body(file, marker) {
  const source = fs.readFileSync(path.join(root, file), "utf8");
  const start = source.indexOf(marker);
  assert.ok(start >= 0, marker);
  const brace = source.indexOf("{", start);
  let depth = 1;
  for (let end = brace + 1; end < source.length; end++) {
    if (source[end] === "{") depth++;
    if (source[end] === "}") {
      depth--;
      if (depth === 0) return source.slice(brace + 1, end);
    }
  }
  throw new Error("Unclosed handler: " + marker);
}

const client = { accountId: "first", accountGeneration: 1, nextId: 1, callbacks: {},
  metrics: { tx_requests: 0, rx_lines: 0, rx_bytes: 0, response_slow_count: 0, handler_slow_count: 0, parse_errors: 0 },
  measure(name, value) { this.metrics[name] = Math.max(this.metrics[name] || 0, value); },
  sock: { connected: true, write(value) { this.last = JSON.parse(value); }, flush() {} } };
const ctx = vm.createContext({ client, JSON });
vm.runInContext("function request(cmd,args,callback) {" + body("app/OmagramClient.qml", "function request(") + "}", ctx);
vm.runInContext("function handleLine(line) {" + body("app/OmagramClient.qml", "function handleLine(") + "}", ctx);
let applied = 0;
ctx.request("chat.history", {}, () => applied++);
assert.equal(client.sock.last.account, "first");
client.accountId = "second";
client.accountGeneration++;
ctx.handleLine(JSON.stringify({ id: 1, ok: true, result: {} }));
assert.equal(applied, 0, "Old account history must not update the new account");
assert.equal(Object.keys(client.callbacks).length, 0);
ctx.request("chat.history", {}, () => applied++);
const staleId = client.nextId - 1;
client.accountId = "first";
client.accountGeneration++;
client.accountId = "second";
client.accountGeneration++;
ctx.handleLine(JSON.stringify({ id: staleId, ok: true, result: {} }));
assert.equal(applied, 0, "Returning to the same account must not revive an old callback");
ctx.request("account.switch", { accountId: "first" }, () => applied++);
const switchId = client.nextId - 1;
assert.equal(client.sock.last.account, undefined);
client.accountId = "first";
client.accountGeneration++;
ctx.handleLine(JSON.stringify({ id: switchId, ok: true, result: {} }));
assert.equal(applied, 1, "Control responses must survive an account transition");

const omagram = { activeAccount: "first", connection: "original", files: {}, chats: [],
  refreshAccounts() {}, reloadAccount() {}, openFromService() {} };
const eventCtx = vm.createContext({ omagram, Qt: { quit() {} }, Model: {} });
vm.runInContext("function onEvent(name,e) {" + body("app/Main.qml", "function onEvent(") + "}", eventCtx);
eventCtx.onEvent("connection", { account: "second", state: "offline" });
eventCtx.onEvent("file", { account: "second", file: { id: 7, path: "other" } });
eventCtx.onEvent("me", { account: "second", meId: 77 });
assert.equal(omagram.connection, "original");
assert.equal(omagram.meId, undefined);
assert.equal(Object.keys(omagram.files).length, 0);
eventCtx.onEvent("connection", { account: "first", state: "ready" });
assert.equal(omagram.connection, "ready");
console.log("Account UI checks passed: stale callbacks, control responses, cross-account events");

let resets = 0;
const snapshotApp = { activeAccount: "first", accountRevision: 2,
  resetAccountView() { resets++; } };
const snapshots = vm.createContext({ omagram: snapshotApp, screen: { active: true }, chatFlush: { stop() {} } });
vm.runInContext("function applyAccountSnapshot(result) {" + body("app/Main.qml", "function applyAccountSnapshot(") + "}", snapshots);
snapshots.applyAccountSnapshot({ activeAccount: "second", accountRevision: 3, auth: { state: "password" },
  allChats: [], calls: [], stories: [] });
assert.equal(snapshotApp.activeAccount, "second");
assert.equal(snapshotApp.auth.state, "password");
assert.equal(resets, 1);
snapshots.applyAccountSnapshot({ activeAccount: "first", accountRevision: 2, auth: { state: "ready" } });
assert.equal(snapshotApp.activeAccount, "second", "Late snapshot cannot undo a newer switch");
assert.equal(snapshotApp.auth.state, "password");
console.log("Snapshot checks passed: auth screen and monotonic account revision");

const savedWidths = [];
const sidebarApp = { chatListWidth: 300 };
const sidebar = vm.createContext({ omagram: sidebarApp,
  service: { request(cmd, args) { savedWidths.push({ cmd, width: args.width }); } } });
vm.runInContext("function toggleChatList(compact) {" + body("app/Main.qml", "function toggleChatList(") + "}", sidebar);
sidebar.toggleChatList(false);
assert.equal(sidebarApp.chatListWidth, 72);
sidebar.toggleChatList(true);
assert.equal(sidebarApp.chatListWidth, 300);
assert.deepEqual(savedWidths.map(x => x.width), [72, 300]);
assert.ok(savedWidths.every(x => x.cmd === "settings.chatListWidth"));
console.log("Sidebar toggle checks passed: collapse, expand, saved widths");

const quitState = { quitting: false, stopped: false };
let quitCallback, quitCalls = 0, restartStopped = false;
const quitContext = vm.createContext({ service: quitState, restart: { stop() { restartStopped = true; } },
  console, client: { request(cmd, args, callback) { assert.equal(cmd, "app.quit"); quitCalls++; quitCallback = callback; } } });
vm.runInContext("function quit() {" + body("shell/Service.qml", "function quit(") + "}", quitContext);
quitContext.quit();
assert.equal(quitState.quitting, true);
assert.equal(quitState.stopped, false, "Do not kill the daemon before the request has been delivered");
assert.equal(restartStopped, true);
quitContext.quit();
assert.equal(quitCalls, 1);
quitCallback({ ok: false });
assert.equal(quitState.quitting, false, "A failed quit is retryable");
console.log("Quit delivery checks passed: no premature stop, idempotence, retry on error");
