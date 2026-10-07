import QtQuick
import QtTest
import qs.Commons
import "../../app/Model.js" as Model

TestCase {
  id: tests
  name: "MessageContrastAndPresence"
  when: windowShown
  visible: true
  width: 800
  height: 760
  Component { id: mainScene; Preview {} }
  Component { id: quickScene; QuickConsumer {} }
  function init() { failOnWarning(/.*/) }
  function cleanup() { Style.fontBaseSize = 12; Style.cornerRadius = 0 }
  function contrast(ink, ground, label) {
    var ratio = Model.colorContrast(ink, ground)
    verify(ratio >= 4.5 - 0.02, label + " contrast " + ratio)
    console.info("CONTRAST " + label + " " + ratio.toFixed(3))
  }
  function test_main_data() {
    var data = []
    for (var palette of ["installed", "light", "dark"]) for (var radius of [0, 8]) data.push({ tag: palette + radius, palette: palette, radius: radius })
    return data
  }
  function test_main(data) {
    var view = createTemporaryObject(mainScene, tests, { width: 700, height: 650, themePalette: data.palette, themeRadius: data.radius, scene: "compact", groupChat: true })
    tryCompare(view, "ready", true)
    for (var id of [1, 2, 3]) {
      var bubble = findChild(view, "message-bubble-" + id)
      var body = findChild(view, "message-text-" + id)
      var meta = findChild(view, "message-metadata-" + id)
      verify(bubble && body && meta)
      compare(bubble.radius, data.radius)
      contrast(body.color, bubble.color, "body")
      contrast(meta.color, bubble.color, "time")
      contrast(body.selectedTextColor, body.selectionColor, "selected fragment")
      if (id !== 2) {
        var name = findChild(view, "message-sender-" + id)
        verify(name.visible)
        contrast(name.color, bubble.color, "sender")
        verify(name.font.bold)
      }
    }
  }
  function test_quick_data() {
    return [{ tag: "installed0", installedPalette: true, lightTheme: false, radius: 0 }, { tag: "light8", installedPalette: false, lightTheme: true, radius: 8 }, { tag: "dark0", installedPalette: false, lightTheme: false, radius: 0 }]
  }
  function test_quick(data) {
    var view = createTemporaryObject(quickScene, tests, { width: 430, height: 530, fontSize: 15, installedPalette: data.installedPalette, lightTheme: data.lightTheme, themeRadius: data.radius, peerStatus: { state: "online" } })
    tryCompare(view, "ready", true)
    var title = findChild(view, "quick-chat-title")
    var online = findChild(view, "quick-chat-online-status")
    verify(online.visible)
    var a = title.mapToItem(view, title.width, title.height / 2)
    var b = online.mapToItem(view, 0, online.height / 2)
    verify(b.x >= a.x && b.x - a.x < 24, "Online must be beside the name")
    verify(Math.abs(a.y - b.y) < 3)
    for (var id of [1, 20, 21, 22, 23]) {
      var bubble = findChild(view, "quick-message-" + id)
      var body = findChild(view, "quick-message-text-" + id)
      var name = findChild(view, "quick-message-sender-" + id)
      var time = findChild(view, "quick-message-time-" + id)
      contrast(body.color, bubble.color, "quick body")
      contrast(name.color, bubble.color, "quick sender")
      contrast(time.color, bubble.color, "quick time")
      contrast(body.selectedTextColor, body.selectionColor, "quick selected fragment")
    }
    view.serviceModel.chats = view.serviceModel.chats.map(function (chat) { return chat.id === 101 ? Object.assign({}, chat, { status: { state: "recently" } }) : chat })
    tryCompare(online, "visible", false)
    var lastSeen = findChild(view, "quick-chat-peer-status")
    verify(lastSeen.visible)
    compare(lastSeen.text, "last seen recently")
  }
  function test_main_narrow_status() {
    var view = createTemporaryObject(mainScene, tests, { width: 403, height: 650, scene: "compact", peerStatus: { state: "online" }, themePalette: "installed" })
    tryCompare(view, "ready", true)
    var title = findChild(view, "chat-title")
    var online = findChild(view, "chat-online-status")
    verify(online.visible && online.width >= online.implicitWidth)
    verify(title.width > 0)
    contrast(online.color, view.color, "main online")
    contrast(findChild(view, "chat-peer-status").color, view.color, "main last seen")
    var a = title.mapToItem(view, title.width, title.height / 2)
    var b = online.mapToItem(view, 0, online.height / 2)
    verify(b.x >= a.x && b.x - a.x < 24, JSON.stringify({ a: a, b: b, titleWidth: title.width, onlineWidth: online.width }))
    verify(Math.abs(a.y - b.y) < 3)
  }
}
