import QtQuick
import QtQuick.Shapes

// An original stepped paper plane for Telebar, not the official Telegram logo.
// The legacy component name keeps existing native consumers compatible.
Item {
  id: mark

  property real size: 16
  property color color: "white"

  width: mark.size
  height: mark.size
  implicitWidth: mark.size
  implicitHeight: mark.size

  Shape {
    width: 16
    height: 16
    scale: mark.size / 16
    transformOrigin: Item.TopLeft
    preferredRendererType: Shape.CurveRenderer

    ShapePath {
      fillColor: mark.color
      fillRule: ShapePath.OddEvenFill
      strokeColor: "transparent"
      PathSvg {
        path: "M1 6H3V5H6V4H9V3H12V2H15V5H14V9H13V13H12V15H11V14H9V13H7V12H6V14H5V9H3V8H1Z M5 8H7V7H9V6H11V5H12V4H13V5H12V6H11V7H10V8H9V9H8V10H7V11H6V9H5Z"
      }
    }
  }
}
