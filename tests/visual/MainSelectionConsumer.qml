import QtQuick
import QtTest
import qs.Commons
import "../../app" as App

// Actual Main subtree with synthetic history, unshown surfaces and no runtime I/O.
Rectangle {
  id: fixture
  width: 1000
  height: 760
  color: app.background
  property alias app: app
  property var surface: null
  property var messageText: null
  property var chatView: null
  property bool settled: false
  readonly property bool ready: settled
  property string failure: ""
  property int phase: 0
  TestEvent { id: pointer }
  function find(item, predicate) {
    if (predicate(item)) return item
    for (var child of item.children || []) {
      var result = find(child, predicate)
      if (result) return result
    }
    return null
  }
  FixtureApp { id: model }
  App.Main { id: app; autoStart: false; surfaceVisible: false; socketPath: "" }
  Component.onCompleted: {
    Style.cornerRadius = 0
    fixture.surface = app.contentSurface
    if (!surface || surface.visible || app.autoStart || app.socketPath !== "")
      throw new Error("Main selection fixture must not map a surface or connect")
    surface.contentItem.parent = fixture
    surface.contentItem.width = Qt.binding(function () { return fixture.width })
    surface.contentItem.height = Qt.binding(function () { return fixture.height })
    app.accounts = model.accounts
    app.chats = model.chats
    app.meId = model.meId
    app.openChatId = 101
    app.messages = { "101": [{ id: 70, chatId: 101, date: 1790726000, outgoing: false,
      content: { kind: "text", text: "Select fragment, not the entire message.\nThe full window must keep native text selection.", entities: [] }, reactions: [] }] }
    app.auth = { state: "ready" }
    settle.start()
  }
  Timer {
    id: settle
    interval: 25
    repeat: true
    property int ticks: 0
    onTriggered: {
      if (++ticks > 80) { stop(); fixture.failure = "Main selection readiness timeout"; return }
      fixture.messageText = fixture.find(surface.contentItem, function (item) { return item.objectName === "message-text-70" })
      fixture.chatView = fixture.find(surface.contentItem, function (item) { return typeof item.focusComposer === "function" && typeof item.syncRows === "function" })
      if (!messageText || !chatView || messageText.width <= 0 || !messageText.visible) return
      var a = messageText.positionToRectangle(7)
      var b = messageText.positionToRectangle(15)
      if (fixture.phase === 0) {
        pointer.mousePress(messageText, a.x + 1, a.y + a.height / 2, Qt.LeftButton, Qt.NoModifier, 0)
        fixture.phase++
        return
      }
      if (fixture.phase <= 8) {
        pointer.mouseMove(messageText, a.x + (b.x - a.x) * fixture.phase / 8 + 1, a.y + a.height / 2, 0, Qt.LeftButton, Qt.NoModifier)
        fixture.phase++
        return
      }
      pointer.mouseRelease(messageText, b.x + 1, b.y + b.height / 2, Qt.LeftButton, Qt.NoModifier, 0)
      if (messageText.selectedText !== "fragment" || !messageText.activeFocus) {
        stop()
        fixture.failure = "Actual Main pointer selection failed: " + messageText.selectedText
        throw new Error(fixture.failure)
      }
      fixture.settled = true
      stop()
    }
  }
}
