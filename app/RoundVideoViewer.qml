import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import QtMultimedia
import qs.Commons
import "Model.js" as Model

// Original video at screen size; zoom/pan the same decoder without replaying it.
FocusScope {
  id: viewer
  objectName: "round-video-viewer"
  property var app: null
  property var message: null
  property bool playbackMuted: false
  property bool surfaceReady: false
  property int decodedFrames: 0
  property int presentationSwaps: 0
  // Bounded incident observation: technical counters only, no media identity.
  Timer {
    id: presentationSample
    interval: 1000
    repeat: true
    running: viewer.visible && samples < 12
    property int samples: 0
    property int lastFrames: 0
    property int lastSwaps: 0
    property real sampledAt: Date.now()
    onTriggered: {
      var now = Date.now(), p = viewer.player
      console.info("Telebar round presentation", JSON.stringify({ sample: ++samples,
        elapsedMs: now - sampledAt, decoded: viewer.decodedFrames - lastFrames,
        swaps: viewer.presentationSwaps - lastSwaps, surfaceReady: viewer.surfaceReady,
        position: p ? p.position : 0, error: p ? p.error : 0, api: viewer.GraphicsInfo.api }))
      sampledAt = now
      lastFrames = viewer.decodedFrames
      lastSwaps = viewer.presentationSwaps
    }
  }
  readonly property bool pictureReady: decodedFrames > 0
  // Bind the decoder only after this window has presented its first frame.
  // A newly mapped overlay does not yet own the render resources of a warm one.
  Connections {
    target: viewer.Window.window
    function onFrameSwapped() { viewer.surfaceReady = true; viewer.presentationSwaps++ }
  }
  Connections {
    target: viewer.player ? viewer.player.videoSink : null
    function onVideoFrameChanged() {
      if (viewer.player.videoSink.videoSize.width > 0) viewer.decodedFrames++
    }
  }
  property real zoom: 1
  property real panX: 0
  property real panY: 0
  property string downloadError: ""
  property int downloadSerial: 0
  property real requestedFileId: 0
  readonly property var media: message && message.content ? message.content.media : null
  readonly property var file: media && app ? app.fileState(media.file) : null
  readonly property string url: file ? Model.fileUrl(file.path) : ""
  readonly property real diameter: Math.max(0, Math.min(width - Style.space(32), height - Style.space(132)))
  readonly property var player: videoLoader.item
  readonly property real panLimit: diameter * (zoom - 1) / 2
  readonly property string fontFamily: app && app.fontFamily ? app.fontFamily : Style.font.family
  signal closed()

  function resetZoom() { zoom = 1; panX = 0; panY = 0 }
  function setZoom(value) {
    zoom = Math.max(1, Math.min(4, value))
    panX = Math.max(-panLimit, Math.min(panLimit, panX))
    panY = Math.max(-panLimit, Math.min(panLimit, panY))
  }
  function togglePlay() {
    if (!player) return
    if (player.playbackState === MediaPlayer.PlayingState) player.pause()
    else player.play()
  }
  function fetch() {
    if (file && !url && !file.active && app && downloadError === "" && requestedFileId !== file.id) {
      requestedFileId = file.id
      var serial = ++downloadSerial
      app.download(file.id, 32, function(answer) {
        if (serial !== viewer.downloadSerial) return
        if (!answer.ok) viewer.downloadError = "Could not download this round video"
      })
    }
  }
  onMessageChanged: { downloadSerial++; requestedFileId = 0; downloadError = ""; resetZoom() }
  onFileChanged: fetch()
  onDiameterChanged: setZoom(zoom)
  Component.onCompleted: { fetch(); forceActiveFocus() }
  onVisibleChanged: if (!visible) {
    surfaceReady = false
    decodedFrames = 0
    if (player) player.stop()
  }
  onUrlChanged: { decodedFrames = 0; if (url !== "") downloadError = "" }

  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Escape || event.key === Qt.Key_Q) viewer.closed()
    else if (event.key === Qt.Key_Plus || event.key === Qt.Key_Equal) viewer.setZoom(viewer.zoom * 1.25)
    else if (event.key === Qt.Key_Minus) viewer.setZoom(viewer.zoom / 1.25)
    else if (event.key === Qt.Key_0) viewer.resetZoom()
    else if (event.key === Qt.Key_Space || event.key === Qt.Key_Return) viewer.togglePlay()
    else if (event.key === Qt.Key_Left) viewer.panX = Math.min(viewer.panLimit, viewer.panX + Style.space(32))
    else if (event.key === Qt.Key_Right) viewer.panX = Math.max(-viewer.panLimit, viewer.panX - Style.space(32))
    else if (event.key === Qt.Key_Up) viewer.panY = Math.min(viewer.panLimit, viewer.panY + Style.space(32))
    else if (event.key === Qt.Key_Down) viewer.panY = Math.max(-viewer.panLimit, viewer.panY - Style.space(32))
    else return
    event.accepted = true
  }

  Rectangle { anchors.fill: parent; color: "black" }
  MouseArea { anchors.fill: parent; onClicked: viewer.closed() }

  Text {
    anchors.top: parent.top
    anchors.horizontalCenter: parent.horizontalCenter
    anchors.topMargin: Style.space(18)
    width: Math.max(0, parent.width - Style.space(100))
    text: "Round video"
    horizontalAlignment: Text.AlignHCenter
    color: "white"
    font.family: viewer.fontFamily
    font.pixelSize: Style.font.body
  }

  Item {
    id: frame
    objectName: "round-video-frame"
    anchors.horizontalCenter: parent.horizontalCenter
    y: Style.space(54)
    width: viewer.diameter
    height: width
    Item {
      id: videoSurface
      anchors.fill: parent
      clip: true
      Loader {
        id: videoLoader
        objectName: "round-video-player-loader"
        anchors.centerIn: parent
        anchors.horizontalCenterOffset: viewer.panX
        anchors.verticalCenterOffset: viewer.panY
        width: parent.width * viewer.zoom
        height: width
        active: viewer.visible && viewer.surfaceReady && viewer.diameter > 0 && viewer.url !== ""
        sourceComponent: FollowingVideo {
          objectName: "round-video-player"
          source: viewer.url
          autoPlay: true
          muted: viewer.playbackMuted || !viewer.pictureReady
          playbackRate: Model.playbackRate(viewer.app ? viewer.app.playbackRate : 1)
          fillMode: VideoOutput.PreserveAspectCrop
          endOfStreamPolicy: VideoOutput.KeepLastFrame
        }
      }
    }
    // Render video directly into its window, not an invisible shader-source
    // texture. Static outer corners preserve the circle without an extra target.
    Canvas {
      anchors.fill: parent
      onWidthChanged: requestPaint()
      onHeightChanged: requestPaint()
      onPaint: {
        var context = getContext("2d")
        context.reset()
        context.fillStyle = "black"
        context.fillRect(0, 0, width, height)
        context.globalCompositeOperation = "destination-out"
        context.beginPath()
        context.arc(width / 2, height / 2, width / 2, 0, 2 * Math.PI)
        context.fill()
        context.globalCompositeOperation = "source-over"
      }
    }
    MouseArea {
      id: pan
      objectName: "round-video-pan"
      anchors.fill: parent
      property real pressX: 0
      property real pressY: 0
      property real originalX: 0
      property real originalY: 0
      property bool moved: false
      cursorShape: viewer.zoom > 1 ? (pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor) : Qt.PointingHandCursor
      onPressed: function(mouse) { pressX = mouse.x; pressY = mouse.y; originalX = viewer.panX; originalY = viewer.panY; moved = false }
      onPositionChanged: function(mouse) {
        if (!pressed || viewer.zoom <= 1) return
        var dx = mouse.x - pressX, dy = mouse.y - pressY
        if (Math.abs(dx) + Math.abs(dy) > Style.space(4)) moved = true
        viewer.panX = Math.max(-viewer.panLimit, Math.min(viewer.panLimit, originalX + dx))
        viewer.panY = Math.max(-viewer.panLimit, Math.min(viewer.panLimit, originalY + dy))
      }
      onClicked: if (!moved) viewer.togglePlay()
      onWheel: function(wheel) {
        if (wheel.angleDelta.y !== 0) viewer.setZoom(viewer.zoom * (wheel.angleDelta.y > 0 ? 1.25 : 0.8))
        wheel.accepted = true
      }
    }
    Text {
      objectName: "round-video-status"
      anchors.centerIn: parent
      width: parent.width * 0.8
      horizontalAlignment: Text.AlignHCenter
      wrapMode: Text.Wrap
      visible: !viewer.url || !viewer.pictureReady || (viewer.player && viewer.player.error !== MediaPlayer.NoError)
      text: viewer.downloadError !== "" ? viewer.downloadError
          : viewer.player && viewer.player.error !== MediaPlayer.NoError ? "Could not play this round video"
          : "Loading round video… " + Math.round(Model.progress(viewer.file) * 100) + "%"
      color: "white"
      font.family: viewer.fontFamily
      font.pixelSize: Style.font.body
    }
  }

  component Control: Rectangle {
    id: control
    property string text: ""
    property string label: text
    signal clicked()
    implicitWidth: Math.max(Style.space(38), controlText.implicitWidth + Style.space(20))
    implicitHeight: Style.space(36)
    radius: Style.cornerRadius
    color: Qt.rgba(1, 1, 1, control.activeFocus || pointer.containsMouse ? 0.22 : 0.12)
    border.width: control.activeFocus ? 2 : 1
    border.color: Qt.rgba(1, 1, 1, 0.8)
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: label
    Accessible.onPressAction: clicked()
    Keys.onReturnPressed: clicked()
    Keys.onEnterPressed: clicked()
    Keys.onSpacePressed: clicked()
    Text { id: controlText; anchors.centerIn: parent; text: control.text; color: "white"; font.family: viewer.fontFamily; font.pixelSize: Style.font.body }
    MouseArea { id: pointer; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: control.clicked() }
  }
  ColumnLayout {
    anchors.horizontalCenter: parent.horizontalCenter
    anchors.bottom: parent.bottom
    anchors.bottomMargin: Style.space(14)
    spacing: Style.space(8)
    RowLayout {
      Layout.alignment: Qt.AlignHCenter
      spacing: Style.space(8)
      Control { objectName: "round-video-zoom-out"; text: "−"; label: "Zoom out"; onClicked: viewer.setZoom(viewer.zoom / 1.25) }
      Control { objectName: "round-video-zoom-reset"; text: Math.round(viewer.zoom * 100) + "%"; label: "Reset zoom"; onClicked: viewer.resetZoom() }
      Control { objectName: "round-video-zoom-in"; text: "+"; label: "Zoom in"; onClicked: viewer.setZoom(viewer.zoom * 1.25) }
      Control { objectName: "round-video-play-pause"; text: viewer.player && viewer.player.playbackState === MediaPlayer.PlayingState ? "Pause" : "Play"; onClicked: viewer.togglePlay() }
      Control { objectName: "round-video-close"; text: "Close"; onClicked: viewer.closed() }
    }
    Text {
      Layout.alignment: Qt.AlignHCenter
      text: "Wheel or +/− zoom · Drag to move · 0 resets · Esc closes"
      color: Qt.rgba(1, 1, 1, 0.8)
      font.family: viewer.fontFamily
      font.pixelSize: Style.font.caption
    }
  }
}
