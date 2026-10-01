import QtQuick
import QtQuick.Shapes

// Original matched contours. Only two semantic state pairs morph, on a short finite animation.
Item {
  id: morph
  property string name: "play"
  property color color: "white"
  property bool animated: true
  readonly property bool panel: name === "panelClose" || name === "panelOpen"
  property real progress: name === "pause" || name === "panelOpen" ? 1 : 0
  Behavior on progress { NumberAnimation { duration: morph.animated && morph.visible ? 160 : 0; easing.type: Easing.InOutQuad } }
  Item {
    width: 24
    height: 24
    anchors.centerIn: parent
    scale: Math.min(morph.width, morph.height) / 24
    Shape {
      anchors.fill: parent
      visible: !morph.panel
      ShapePath {
        strokeColor: morph.color
        strokeWidth: 2.4
        fillColor: "transparent"
        joinStyle: ShapePath.RoundJoin
        PathPolyline {
          path: [Qt.point(7, 4), Qt.point(20 - 10 * morph.progress, 12 - 8 * morph.progress),
                 Qt.point(20 - 10 * morph.progress, 12 + 8 * morph.progress), Qt.point(7, 20), Qt.point(7, 4)]
        }
      }
      ShapePath {
        strokeColor: Qt.rgba(morph.color.r, morph.color.g, morph.color.b, morph.color.a * morph.progress)
        strokeWidth: 2.4
        fillColor: "transparent"
        joinStyle: ShapePath.RoundJoin
        PathPolyline {
          path: [Qt.point(15, 4), Qt.point(19, 4), Qt.point(19, 20), Qt.point(15, 20), Qt.point(15, 4)]
        }
      }
    }
    Shape {
      anchors.fill: parent
      visible: morph.panel
      ShapePath {
        strokeColor: morph.color
        strokeWidth: 2.4
        fillColor: "transparent"
        joinStyle: ShapePath.RoundJoin
        PathSvg { path: "M3 4h18v16H3V4ZM9 4v16" }
      }
      ShapePath {
        strokeColor: morph.color
        strokeWidth: 2.4
        fillColor: "transparent"
        capStyle: ShapePath.RoundCap
        joinStyle: ShapePath.RoundJoin
        PathPolyline { path: [Qt.point(16 - 3 * morph.progress, 8), Qt.point(12 + 5 * morph.progress, 12), Qt.point(16 - 3 * morph.progress, 16)] }
      }
    }
  }
}
