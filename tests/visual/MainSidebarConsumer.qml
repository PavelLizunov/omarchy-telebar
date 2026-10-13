import QtQuick
import qs.Commons
import "../../app" as App

// The complete Main consumer, with native windows unshown and no socket/start.
// Only its actual content subtree is reparented. All data is synthetic.
Rectangle {
  id: fixture
  width: 249
  height: 650
  color: "#101315"
  property int themeRadius: 0
  property bool requireExpanded: true
  property bool settled: false
  property string failure: ""
  readonly property bool ready: settled && failure === ""
  property var sidebar: null
  property var handle: null
  property var surface: null
  readonly property real actualListWidth: sidebar ? sidebar.width : 0
  readonly property bool actualCompact: sidebar ? sidebar.compact : true
  readonly property int requestedListWidth: app.chatListWidth
  property int phase: 0

  function require(condition, reason) {
    if (condition) return
    fixture.failure = reason
    throw new Error(reason)
  }
  function find(item, predicate) {
    if (predicate(item)) return item
    for (var child of item.children || []) {
      var found = find(child, predicate)
      if (found) return found
    }
    return null
  }
  FixtureApp { id: model }
  App.Main {
    id: app
    autoStart: false
    surfaceVisible: false
    socketPath: ""
  }
  Component.onCompleted: {
    Style.cornerRadius = themeRadius
    fixture.require(!app.autoStart && app.socketPath === "", "No application I/O permitted")
    fixture.surface = app.contentSurface
    fixture.require(surface && !surface.visible, "Native Main surface must stay unshown")
    surface.contentItem.parent = fixture
    surface.contentItem.width = Qt.binding(function() { return fixture.width })
    surface.contentItem.height = Qt.binding(function() { return fixture.height })
    app.accounts = model.accounts
    app.activeAccount = model.activeAccount
    app.chats = model.chats
    app.chatListWidth = 72
    app.auth = { state: "ready" }
    settle.start()
  }
  Timer {
    id: settle
    interval: 25
    repeat: true
    property int ticks: 0
    onTriggered: {
      if (++ticks > 80) { stop(); fixture.require(false, "Bounded Main sidebar readiness"); return }
      if (!fixture.sidebar) {
        fixture.sidebar = fixture.find(surface.contentItem, function(item) {
          return typeof item.focusSearch === "function" && typeof item.buildRows === "function"
        })
        fixture.handle = fixture.find(surface.contentItem, function(item) {
          return typeof item.forceButtonFocus === "function"
        })
        if (!fixture.sidebar || !fixture.handle) return
        fixture.sidebar.objectName = "main-sidebar"
        fixture.handle.objectName = "main-sidebar-handle"
      }
      fixture.require(!surface.visible && !app.windowFocused, "No surface or read acknowledgement")
      if (fixture.phase === 0) {
        if (!sidebar.compact || sidebar.width !== 72) return
        fixture.handle.toggleRequested()
        fixture.phase = 1
        return
      }
      if (app.chatListWidth !== 300) return
      if (fixture.requireExpanded)
        fixture.require(!sidebar.compact && sidebar.width >= 180, "Expanded chat list remained compact")
      fixture.settled = true
      stop()
    }
  }
}
