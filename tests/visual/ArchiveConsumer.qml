import QtQuick
import QtTest
import qs.Commons
import "../../app" as App
import "../../app/Model.js" as Model

// Actual Main and transport handlers, synthetic list replies and no native I/O.
Rectangle {
  id: fixture
  width: 1000
  height: 760
  color: app.background
  property bool ready: false
  property var client: null
  property var sidebar: null
  property var surface: null
  property var archived: []
  property string failure: ""
  property int phase: 0
  property int themeRadius: 0
  TestResult { id: lookup }
  function require(value, reason) {
    if (!value) { fixture.failure = reason; throw new Error(reason) }
  }
  function find(item, predicate) {
    if (predicate(item)) return item
    var children = item.children || item.data || []
    for (var i = 0; i < children.length; i++) {
      var found = find(children[i], predicate)
      if (found) return found
    }
    return null
  }
  function chat(id, list, order) {
    return { id: id, title: list === "archive" ? "Archived conversation " + (-id) : "Synthetic conversation " + id,
      kind: "private", userId: id, unread: 0, markedUnread: false, muted: true, lists: [list],
      positions: { [list]: { order: String(order), pinned: false } },
      lastMessage: { id: 1, date: 1792350000, text: "Previously cached preview", outgoing: false } }
  }
  QtObject {
    id: wire
    property bool connected: true
    function flush() {}
    function write(line) {
      var request = JSON.parse(line)
      var reply = { id: request.id, ok: true, result: {} }
      if (request.cmd === "chats.list") reply.result.chats = request.args.list === "archive" ? fixture.archived : []
      if (request.cmd === "chats.load") reply = { id: request.id, ok: false, code: 404, error: "All chats loaded" }
      Qt.callLater(function () { fixture.client.handleLine(JSON.stringify(reply)) })
    }
  }
  App.Main { id: app; autoStart: false; surfaceVisible: false; socketPath: "" }
  Component.onCompleted: {
    Style.cornerRadius = themeRadius
    surface = app.contentSurface
    require(surface && !surface.visible && !app.autoStart && app.socketPath === "", "No native surface or account I/O")
    surface.contentItem.parent = fixture
    surface.contentItem.width = Qt.binding(function () { return fixture.width })
    surface.contentItem.height = Qt.binding(function () { return fixture.height })
    client = lookup.findChild(app, "window-transport")
    require(client !== null, "Actual Main transport required")
    client.sock = wire
    var main = [], archive = []
    for (var i = 1; i <= 500; i++) main.push(chat(i, "main", 1000 - i))
    for (var j = 1; j <= 41; j++) archive.push(chat(-j, "archive", 100 - j))
    archived = archive
    app.applyAccountSnapshot({ activeAccount: "default", accountRevision: 1, auth: { state: "ready" }, allChats: main })
    settle.start()
  }
  Timer {
    id: settle
    interval: 30
    repeat: true
    property int ticks: 0
    onTriggered: {
      if (++ticks > 100) { stop(); fixture.require(false, "Archive consumer deadline"); return }
      fixture.require(!surface.visible && !app.windowFocused, "Unshown native consumer only")
      if (!sidebar) sidebar = fixture.find(surface.contentItem, function (item) { return typeof item.buildRows === "function" })
      if (!sidebar) return
      if (phase === 0) {
        sidebar.listSelected("archive")
        phase = 1
        return
      }
      if (phase === 1) {
        if (app.chats.length !== 541 || sidebar.rows.length !== 41) return
        var updated = Object.assign({}, archived[0], { unread: 3,
          lastMessage: { id: 2, date: 1792352100, text: "New archive update received", outgoing: false } })
        client.handleLine(JSON.stringify({ event: "chat", account: "default", chat: updated }))
        phase = 2
        return
      }
      var first = sidebar.rows[0].chat
      if (first.unread !== 3 || first.lastMessage.id !== 2) return
      fixture.require(Model.chatsIn(app.chats, "main").length === 500, "Archive must not evict main chats")
      fixture.require(sidebar.rows.length === 41, "All archived rows remain after streamed update")
      fixture.require(!!fixture.find(surface.contentItem, function (item) { return item.text === "New archive update received" && item.visible }), "Visible archive preview must refresh")
      fixture.ready = true
      stop()
    }
  }
}
