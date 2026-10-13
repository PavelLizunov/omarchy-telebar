import QtQuick
import qs.Commons as Host
import "../../shell" as Telebar

Rectangle {
  id: preview
  width: 420
  height: 116
  property int themeRadius: 0
  property bool verticalBar: false
  property bool serviceReady: true
  property int unreadCount: 0
  property color ink: "#cacccc"
  property color backgroundInk: "#101315"
  property bool settled: false
  property string failure: ""
  property alias fixtureService: service
  property alias fixtureBar: host
  property alias widget: widget
  readonly property bool ready: settled && failure === ""
  color: preview.backgroundInk

  function childNamed(parent, name) {
    if (parent.objectName === name) return parent
    var children = parent.children || []
    for (var i = 0; i < children.length; i++) {
      var found = childNamed(children[i], name)
      if (found) return found
    }
    return null
  }
  function validate() {
    var button = childNamed(widget, "telebar-icon-button")
    var mark = childNamed(widget, "telebar-ring-mark")
    var dot = childNamed(widget, "telebar-unread-dot")
    if (!button || !mark || !dot) throw new Error("Actual Telebar icon items missing")
    if (mark.size !== Host.Style.bar.iconCanvas || mark.color !== preview.ink)
      throw new Error("Actual bar mark does not follow native optical/theme owner")
    if (dot.visible !== (preview.serviceReady && preview.unreadCount > 0))
      throw new Error("Actual unread state changed")
    if (button.width !== (preview.verticalBar ? Host.Style.bar.sizeVertical : Host.Style.bar.iconSlot)
        || button.height !== (preview.verticalBar ? Host.Style.bar.iconSlot : Host.Style.bar.sizeHorizontal))
      throw new Error("Actual bar click slot changed")
    if (Host.Style.cornerRadius !== preview.themeRadius) throw new Error("Native radius changed")
  }
  Component.onCompleted: Host.Style.cornerRadius = themeRadius
  Timer {
    interval: 20
    repeat: true
    running: !preview.settled && preview.failure === ""
    onTriggered: if (preview.childNamed(widget, "telebar-ring-mark")) {
      try { preview.validate(); preview.settled = true }
      catch (error) { preview.failure = String(error); console.error(error) }
    }
  }
  QtObject {
    id: service
    property bool ready: preview.serviceReady
    property int unread: preview.unreadCount
    property bool quiet: false
    property bool stopped: false
    property var accounts: []
    property string activeAccount: "default"
    property int openedWindows: 0
    property int quitCount: 0
    function openWindow() { openedWindows++ }
    function setQuiet(value) { quiet = value }
    function quit() { quitCount++ }
  }
  QtObject {
    id: host
    property bool vertical: preview.verticalBar
    property int barSize: vertical ? Host.Style.bar.sizeVertical : Host.Style.bar.sizeHorizontal
    property color barForeground: preview.ink
    property color urgent: Host.Color.urgent
    property bool foregroundAnimationEnabled: false
    property string fontFamily: Host.Style.font.family
    property string position: vertical ? "left" : "top"
    property var shell: null
    property var clickTarget: null
    function registerClickTarget(target) { clickTarget = target }
    function unregisterClickTarget(target) { if (clickTarget === target) clickTarget = null }
    function hideTooltip(target) {}
    function showTooltip(target, text) {}
  }
  Text {
    x: 16; y: 12
    text: "Actual Telebar bar widget"
    color: preview.ink
    font.family: Host.Style.font.family
    font.pixelSize: Host.Style.font.body
  }
  Telebar.BarWidget {
    id: widget
    objectName: "telebar-widget"
    x: 24; y: 48
    bar: host
    omagram: service
  }
  Text {
    x: 88; y: 48
    text: !preview.serviceReady ? "Unavailable" : (preview.unreadCount > 0 ? "Unread" : "Ready")
    color: preview.ink
    font.family: Host.Style.font.family
    font.pixelSize: Host.Style.font.body
  }
}
