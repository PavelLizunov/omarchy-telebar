import QtQuick
import qs.Commons
import "../../app" as App
import "Readiness.js" as Readiness

Rectangle {
  id: preview
  objectName: "review-root"
  color: model.background
  property int themeRadius: 0
  property string state: "photo"
  property bool ready: false
  FixtureApp { id: model }
  Component.onCompleted: { Style.cornerRadius = themeRadius; Style.fontBaseSize = 15 }
  App.StoryViewer {
    objectName: "review-content"
    anchors.fill: parent
    app: model; client: model
    chats: model.storyChats
    chatId: 101; storyId: 1
    paused: true
    story: preview.state === "loading" ? null : ({ id: 1, chatId: 101, date: model.nowMs / 1000, kind: preview.state === "live" ? "live" : "photo", caption: { text: "Synthetic story caption with several words for wrapping", entities: [] }, media: { file: { id: 50, path: preview.state === "download" ? "" : model.imagePath, active: true, size: 1000, downloaded: 250 } } })
  }
  Timer {
    interval: 20; running: true; repeat: true
    property int settled: 0
    onTriggered: {
      if (Readiness.pending(preview)) { settled = 0; return }
      if (++settled < 4) return
      preview.ready = true; stop()
    }
  }
}
