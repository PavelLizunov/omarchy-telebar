import QtQuick
import qs.Commons as Host
import qs.Ui as Native
import "../../app" as App

Rectangle {
  id: preview
  width: 480
  height: 230
  property int themeRadius: 0
  property real canvasSize: 16
  property color ink: "#cacccc"
  property color backgroundInk: "#101315"
  property bool settled: false
  property var loadedMark: null
  property string failure: ""
  readonly property bool ready: settled && failure === "" && desktopImage.status === Image.Ready
  color: backgroundInk

  function check() {
    if (barButton.width !== Host.Style.bar.iconSlot || barButton.height !== Host.Style.bar.sizeHorizontal)
      throw new Error("Native click slot changed")
    if (!loadedMark || loadedMark.size !== canvasSize || loadedMark.color !== ink)
      throw new Error("Optical canvas or theme color not applied")
    if (desktopImage.status !== Image.Ready || desktopImage.implicitWidth !== 128 || desktopImage.implicitHeight !== 128)
      throw new Error("File SVG did not decode at the requested size")
    if (loadedMark.width !== loadedMark.height || Host.Style.cornerRadius !== themeRadius)
      throw new Error("Square mark canvas or native radius changed")
  }

  Component.onCompleted: {
    Host.Style.cornerRadius = themeRadius
    Host.Style.barOverrides = { "icon-canvas": canvasSize }
  }
  Timer {
    interval: 20
    repeat: true
    running: !preview.settled && preview.failure === ""
    onTriggered: if (preview.loadedMark && desktopImage.status === Image.Ready) {
      try { preview.check(); preview.settled = true }
      catch (error) { preview.failure = String(error); console.error(error) }
    }
  }

  Text {
    x: 16; y: 12
    text: "Native bar size"
    font.family: Host.Style.font.family
    font.pixelSize: Host.Style.font.body
    color: preview.ink
  }
  Native.BarIconButton {
    id: barButton
    objectName: "native-icon-button"
    x: 16; y: 34
    foreground: preview.ink
    iconComponent: Component {
      App.RingMark {
        id: nativeMark
        objectName: "native-ring-mark"
        anchors.centerIn: parent
        size: barButton.opticalSize
        color: barButton.foreground
        Component.onCompleted: preview.loadedMark = this
      }
    }
  }
  App.RingMark {
    objectName: "native-quick-mark"
    x: 70; y: 38
    size: 18
    color: preview.ink
  }
  Text {
    x: 16; y: 82
    text: "Vector detail"
    font.family: Host.Style.font.family
    font.pixelSize: Host.Style.font.body
    color: preview.ink
  }
  App.RingMark {
    objectName: "enlarged-ring-mark"
    x: 16; y: 108
    size: 96
    color: preview.ink
  }
  Image {
    id: desktopImage
    objectName: "desktop-svg"
    x: 150; y: 104
    width: 104; height: 104
    source: "../../assets/omagram.svg"
    sourceSize: Qt.size(128, 128)
    fillMode: Image.PreserveAspectFit
  }
}
