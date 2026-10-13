import QtQuick
import QtQuick.Shapes

// Telebar's ring and speech tail in the host wordmark's stepped vocabulary.
// One filled vector keeps the same silhouette from a native bar canvas up.
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
        path: "M4 1H12V2H13V3H14V11H13V12H12V13H6V14H4V15H2V3H3V2H4Z M5 3H11V4H12V10H11V11H5V10H4V4H5Z"
      }
    }
  }
}
