// Load the actual extracted QML logic and its imports in a Node VM with inert bindings.
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
function loadLogic(name, globals = {}) {
  const file = path.join(__dirname, "../app", name + ".js");
  const source = fs.readFileSync(file, "utf8");
  const context = vm.createContext({ ...globals });
  for (const match of source.matchAll(/^\.import "([^"]+)" as (\w+)$/gm)) {
    context[match[2]] = loadLogic(path.basename(match[1], ".js"), globals);
  }
  vm.runInContext(source.replace(/^\.(pragma|import).*$/gm, ""), context, { filename: file });
  return context;
}
module.exports = { loadLogic };
