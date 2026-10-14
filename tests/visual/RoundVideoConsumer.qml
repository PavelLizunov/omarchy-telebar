import QtQuick
import QtMultimedia
import qs.Commons
import "../../app" as App

Rectangle {
  id: scene
  objectName: "round-review-root"
  width: 900
  height: 760
  color: "#101315"
  property int themeRadius: 0
  property real initialZoom: 1
  property bool loadingState: false
  property string videoPath: decodeURIComponent(String(Qt.resolvedUrl("round-480.mp4")).replace("file://", ""))
  property var files: loadingState ? ({ "71": { id: 71, path: "", active: true, size: 200000, downloaded: 50000 } }) : ({})
  property int downloads: 0
  property bool downloadRefused: false
  property alias viewer: viewer
  property bool settled: false
  property int decodedFrames: 0
  property bool pausedFrame: false
  property string failure: ""
  readonly property bool ready: settled && failure === ""
  readonly property var sample: ({ id: 701, chatId: 101, outgoing: true, content: { kind: "videoNote", media: { duration: 4, width: 480, height: 480, file: { id: 71, path: videoPath, size: 200000, active: false } } } })
  QtObject {
    id: model
    property string fontFamily: "Sans Serif"
    property real playbackRate: 1
    function fileState(file) { return scene.files[file.id] || file }
    function download(id, priority, callback) { scene.downloads++; if (scene.downloadRefused && callback) callback({ ok: false }) }
  }
  App.RoundVideoViewer {
    id: viewer
    anchors.fill: parent
    app: model
    message: scene.sample
    playbackMuted: true
  }
  Connections {
    target: viewer.player ? viewer.player.videoSink : null
    function onVideoFrameChanged() { scene.decodedFrames++ }
  }
  Component.onCompleted: { Style.cornerRadius = themeRadius; viewer.setZoom(initialZoom) }
  Timer {
    interval: 25
    repeat: true
    property int ticks: 0
    running: !scene.ready && !scene.failure
    onTriggered: {
      if (++ticks > 120) { scene.failure = "Bounded native video readiness"; stop(); return }
      if (scene.loadingState) { scene.settled = true; stop(); return }
    if (!viewer.player) return
      if (viewer.player.error !== MediaPlayer.NoError) { scene.failure = "Native decoder error"; stop(); return }
      var size = viewer.player.videoSink.videoSize
      if (size.width === 480 && size.height === 480 && scene.decodedFrames >= 4) {
        if (!scene.pausedFrame) { viewer.player.pause(); scene.pausedFrame = true; return }
        scene.settled = true
        stop()
      }
    }
  }
}
