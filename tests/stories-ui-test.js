const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const path = require("node:path");
const source = fs.readFileSync(path.join(__dirname, "../app/StoryViewer.qml"), "utf8");
const begin = source.indexOf("function downloadMedia()");
let brace = source.indexOf("{", begin), depth = 1, end = brace + 1;
for (; end < source.length; end++) {
  if (source[end] === "{") depth++;
  if (source[end] === "}" && --depth === 0) break;
}
const requests = [], applied = [];
const viewer = { visible: true, file: { id: 12, active: false }, url: "", serial: 1,
  downloadRequested: false, error: "", app: { setFile(file) { applied.push(file); } },
  client: { request(cmd, args, callback) { requests.push({ cmd, args, callback }); } } };
const ctx = vm.createContext({ viewer });
vm.runInContext("function downloadMedia() {" + source.slice(brace + 1, end) + "}", ctx);
ctx.downloadMedia();
ctx.downloadMedia();
assert.equal(requests.length, 1, "Only one request while the story is being downloaded");
assert.equal(requests[0].args.fileId, 12);
requests[0].callback({ ok: false, error: "Download failed" });
assert.equal(viewer.error, "Download failed");
viewer.serial++;
requests[0].callback({ ok: true, result: { id: 12 } });
assert.equal(applied.length, 0, "A previous story's download must not overwrite the current one");

const main = fs.readFileSync(path.join(__dirname, "../app/Main.qml"), "utf8");
const storyBinding = main.slice(main.indexOf("readonly property var storyChats:"), main.indexOf("Timer {", main.indexOf("readonly property var storyChats:")));
assert.ok(storyBinding.includes("if (!omagram.showStories) return []"), "Hidden stories must never populate the visible strip");
const openStories = main.slice(main.indexOf("function openStories("), main.indexOf("// Histories are kept", main.indexOf("function openStories(")));
assert.ok(openStories.includes("if (!omagram.showStories) return"), "Hidden stories cannot be opened by a caller");
const widthBinding = main.match(/Layout.minimumWidth: (Math\.min\(omagram\.chatListWidth, mainScope\.width \* 0\.45\))/);
assert.ok(widthBinding, "The chat list must have a hard width bound");
for (const width of [400, 640, 800, 1100, 1920]) {
  for (const requested of [72, 180, 300, 480, 1200]) {
    const pane = vm.runInNewContext(widthBinding[1], { omagram: { chatListWidth: requested }, mainScope: { width } });
    assert.ok(pane <= width * .45);
    assert.ok(width - pane - 6 > width / 2, "The chat keeps more than half the window");
  }
}
console.log("Story/UI checks passed: one download, visible error, stale response rejection, bounded chat-list width");
