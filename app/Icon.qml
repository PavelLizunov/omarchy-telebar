import QtQuick
import "Icons.js" as Icons
import "Model.js" as Model

// Small original SVGs use Qt's shared Image cache at a stable decode size.
Item {
  id: icon
  property string name: ""
  property var glyph: ""
  property color color: "#808080"
  property real size: 20
  property bool spinning: false
  property bool animated: true
  readonly property string resolvedName: name || Icons.nameFor(glyph)
  readonly property bool morphing: resolvedName === "play" || resolvedName === "pause" || resolvedName === "panelOpen" || resolvedName === "panelClose"
  implicitWidth: size
  implicitHeight: size
  Image {
    id: picture
    visible: !icon.morphing
    anchors.centerIn: parent
    width: Math.min(parent.width, icon.size)
    height: Math.min(parent.height, icon.size)
    sourceSize: Qt.size(Math.ceil(icon.size * 2), Math.ceil(icon.size * 2))
    source: icon.morphing ? "" : Icons.source(icon.resolvedName, Model.hexOf(icon.color))
    opacity: icon.color.a
    fillMode: Image.PreserveAspectFit
    // Reuse the decoded texture; animate item scale only on a discrete icon change.
    onSourceChanged: if (icon.animated && icon.visible && !icon.spinning) statePulse.restart()
    SequentialAnimation {
      id: statePulse
      NumberAnimation { target: picture; property: "scale"; from: 0.92; to: 1; duration: 100 }
    }
    RotationAnimator on rotation {
      running: icon.spinning && icon.visible
      from: 0
      to: 360
      duration: 1000
      loops: Animation.Infinite
    }
  }
  Loader {
    anchors.centerIn: parent
    width: Math.min(parent.width, icon.size)
    height: Math.min(parent.height, icon.size)
    active: icon.morphing
    sourceComponent: MorphIcon { name: icon.resolvedName; color: icon.color; animated: icon.animated }
  }
}
