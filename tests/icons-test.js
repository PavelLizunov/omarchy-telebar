const fs = require("node:fs"), vm = require("node:vm"), assert = require("node:assert/strict");
const c = vm.createContext({});
vm.runInContext(fs.readFileSync(__dirname + "/../app/Icons.js", "utf8"), c);
for (const name of Object.keys(c.paths)) {
  const svg = decodeURIComponent(c.source(name, "#aabbcc").split(",")[1]);
  assert.ok(svg.includes('viewBox="0 0 24 24"'));
  assert.ok(svg.includes('stroke="#aabbcc"'));
  assert.ok(!svg.includes("<script") && !svg.includes("href=") && !svg.includes("onload="));
}
assert.equal(c.nameFor(0xF0349), "search");
assert.equal(c.nameFor(String.fromCodePoint(0xF03E2)), "attach");
assert.equal(c.nameFor(0xF036C), "microphone");
assert.ok(decodeURIComponent(c.source("close", '\" onload=\"evil')).includes('stroke="#808080"'));
console.log("Original SVG icon checks passed:", Object.keys(c.paths).length, "drawings");
const qmlFiles = [...fs.readdirSync(__dirname + "/../app").filter(x => x.endsWith(".qml")).map(x => __dirname + "/../app/" + x),
  ...fs.readdirSync(__dirname + "/../shell").filter(x => x.endsWith(".qml")).map(x => __dirname + "/../shell/" + x)];
for (const file of qmlFiles) {
  const text = fs.readFileSync(file, "utf8");
  for (const match of text.matchAll(/0xF[0-9A-Fa-f]{4,5}\b/g)) {
    const code = Number(match[0]);
    if (code < 0xF0000) continue;
    assert.ok(c.legacy[code], `Unmapped action glyph ${match[0]} in ${file}`);
  }
}
