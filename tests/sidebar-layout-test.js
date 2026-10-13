#!/usr/bin/env node
// Actual display helper and Main bindings; no compositor, settings or Telegram.
"use strict"
const assert = require("assert")
const fs = require("fs")
const path = require("path")
const vm = require("vm")
const root = path.join(__dirname, "..")
const context = {}
const model = fs.readFileSync(path.join(root, "app/Model.js"), "utf8").replace(/^\.pragma library\s*$/m, "")
vm.runInNewContext(model, context)
assert.strictEqual(typeof context.sidebarMaximumWidth, "function")
for (const [width, expected] of [[249, 209], [320, 280], [640, 300], [900, 405], [1100, 495], [4000, 1200]]) {
  assert.strictEqual(context.sidebarMaximumWidth(width, 40), expected)
  assert(context.sidebarMaximumWidth(width, 40) <= width - 40)
  if (width >= 220) assert(context.sidebarMaximumWidth(width, 40) >= 180)
}
for (const width of [0, 100, 179, 200]) {
  const maximum = context.sidebarMaximumWidth(width, 40)
  assert(maximum >= 0 && maximum <= width)
}
const main = fs.readFileSync(path.join(root, "app/Main.qml"), "utf8")
assert(main.includes("Model.sidebarMaximumWidth(mainScope.width, Style.space(40))"))
assert(main.includes("Layout.minimumWidth: Math.min(omagram.chatListWidth, mainScope.maximumChatListWidth)"))
assert(main.includes("maximumWidth: mainScope.maximumChatListWidth"))
assert(main.includes("onToggleRequested: omagram.toggleChatList(chatList.compact)"))
console.log("PASS: six width contracts, four sub-breakpoint bounds, actual Main list/drag/toggle bindings")
