import QtQuick
import "../../shell" as Shell
Rectangle {
  width: 480
  height: 240
  color: "#101315"
  QtObject {
    id: barFixture
    property string fontFamily: "Sans Serif"
    property color barForeground: "#cacccc"
    property bool vertical: false
    property int barSize: 30
    property string position: "top"
    property var shell: null
  }
  Shell.BarWidget { anchors.centerIn: parent; bar: barFixture }
}
