import QtQuick
import qs.Commons
import "../../shell" as Shell

Rectangle {
  id: preview
  width: 300
  height: 250
  color: Color.popups.background
  property int themeRadius: 0
  property int fontSize: 15
  property bool ready: false
  Component.onCompleted: {
    Style.cornerRadius = themeRadius
    Style.fontBaseSize = fontSize
    Qt.callLater(function () { preview.ready = true })
  }
  Shell.BarMenu {
    objectName: "bar-menu-content"
    width: parent.width - 20
    x: 10
    y: 10
    entries: [ { action: "open", label: "Open Omagram" }, { action: "switch:default", label: "✓ Alex Demo (4)" },
      { action: "switch:work", label: "Work Demo (17)" }, { action: "quiet", label: "Mute notifications" }, { action: "quit", label: "Quit" } ]
  }
}
