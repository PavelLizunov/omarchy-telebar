import QtQuick
import qs.Commons as Host
import "../../shell" as Shell

// Actual Panel and QuickView. Only the host keyboard window boundary is inert.
Rectangle {
  id: fixture
  width: 500
  height: 600
  color: "#101315"
  property int themeRadius: 0
  property bool focusOpen: false
  property bool settled: false
  property string failure: ""
  readonly property bool ready: settled && failure === ""
  property var quick: null
  property var mark: null
  property var label: null
  property alias panel: panel
  property alias model: service
  function find(item,name) {
    if (item.objectName === name) return item
    for (var child of item.children || []) { var found=find(child,name);if(found)return found }
    return null
  }
  FixtureApp {
    id: service
    property int openedWindows: 0
    property var openedChats: []
    function openWindow() { openedWindows++ }
    function openChat(chatId) { openedChats=openedChats.concat([chatId]) }
    property var playing: ({fileId:0})
    property var files: ({})
    history: []
    ready: true
    connected: true
  }
  QtObject { id: bar; property color barForeground: "#cacccc" }
  Shell.Panel {
    id: panel
    bar: bar
    omagram: service
    width: 460
    height: 560
  }
  Component.onCompleted: {
    Host.Style.cornerRadius=themeRadius
    panel.open()
  }
  Timer {
    interval: 25
    repeat: true
    property int ticks: 0
    running: !fixture.ready && fixture.failure === ""
    onTriggered: {
      if (++ticks>60) {fixture.failure="Bounded actual panel readiness";stop();return}
      fixture.mark=fixture.find(panel,"openOmagram")
      fixture.label=fixture.find(panel,"quick-open-window-label")
      for (var item of panel.children) {
        if (typeof item.reset === "function") fixture.quick=item
        for (var nested of item.children || []) if (typeof nested.reset === "function") fixture.quick=nested
      }
      if (!mark || !label || !quick || mark.width<=0) return
      if (fixture.focusOpen) mark.forceActiveFocus()
      fixture.settled=true
      stop()
    }
  }
}
