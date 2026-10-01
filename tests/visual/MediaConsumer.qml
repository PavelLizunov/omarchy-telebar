import QtQuick
import qs.Commons
import "../../app" as App
import "../../shell" as Shell
import "Readiness.js" as Readiness

Rectangle {
  id: preview
  objectName: "review-root"
  color: model.background
  property string state: "photo"
  property int themeRadius: 0
  property int fontSize: 15
  property bool ready: false
  property var videoMessage: ({ id: 60, chatId: 101, date: 1790726000, outgoing: false, senderName: "Alex Demo", content: { kind: "video", text: "Synthetic video preview", media: { duration: 10, width: 640, height: 360, file: { id: 60, path: "", active: false, size: 1024 }, thumb: { format: "jpeg", file: { id: 50, path: model.imagePath } } } } })
  FixtureApp { id: model }
  Component.onCompleted: { Style.cornerRadius = themeRadius; Style.fontBaseSize = fontSize }
  Timer { interval: 20; running: true; repeat: true; property int settled: 0; onTriggered: {
    if (Readiness.pending(preview)) { settled = 0; return }
    if (++settled < 4) return
    preview.ready = true; stop()
  } }
  QtObject {
    id: mediaHost
    property var keys: ({})
    property string fontFamily: model.fontFamily
    property var shownChat: model.chats[0]
    property var service: model
    function fileOf(file) { return file }
    function fetchNow() {}
    function fetch() {}
    function stillThumb(media) { return media.thumb ? media.thumb.file : null }
    function urlOf(file) { return file && file.path ? "file://" + file.path : "" }
    function duration() { return "0:10" }
    function hints() { return "Esc closes · Enter plays" }
  }
  Loader {
    anchors.fill: parent
    onLoaded: if (item) item.objectName = "review-content"
    sourceComponent: preview.state === "shell-photo" || preview.state === "shell-video" ? shellViewer : mediaItem
  }
  Component { id: shellViewer; Shell.MediaViewer { host: mediaHost; items: [model.photoMessage, preview.videoMessage]; messageId: preview.state === "shell-video" ? 60 : 50 } }
  Component {
    id: mediaItem
    Item {
      App.MediaView {
        anchors.centerIn: parent
        app: model
        maxWidth: Math.min(parent.width - 30, 360)
        animationsEnabled: false
        message: preview.state === "photo" ? model.photoMessage
          : preview.state === "video" ? preview.videoMessage
          : ({ id: 70, chatId: 101, content: { kind: preview.state, text: "", media: { duration: 24, width: 320, height: 240, waveform: [3,12,20,31,12,7], fileName: "synthetic-document.txt", emoji: "🙂", format: "webp", file: { id: 70, size: 2345, path: preview.state === "sticker" ? model.imagePath : "", active: false } } } })
      }
    }
  }
}
