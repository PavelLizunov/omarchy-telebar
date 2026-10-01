import QtQuick
import qs.Commons
import "../../app" as App
import "Readiness.js" as Readiness

Rectangle {
  id: preview
  objectName: "review-root"
  color: model.background
  property int themeRadius: 0
  property string state: "members"
  property string chatKind: "group"
  property bool ready: false
  FixtureApp { id: model }
  Component.onCompleted: { Style.cornerRadius = themeRadius; Style.fontBaseSize = 15 }
  App.ChatInfo {
    id: info
    objectName: "review-content"
    anchors.fill: parent
    app: model
    client: model
    chat: ({ id: -102, kind: preview.chatKind, title: "Synthetic community with a long title", memberCount: 284, myStatus: "member" })
    Component.onCompleted: Qt.callLater(function () { Qt.callLater(function () {
      info.details = { canGetMembers: true, memberCount: 284, description: "Synthetic description for native layout review" }
      info.counts = { photos: 1, files: 1, links: 1, voice: 1, music: 1, gifs: 1 }
      info.selectTab(preview.state === "loading" ? "members" : preview.state)
      Qt.callLater(function () {
        info.exhausted = true
        info.loading = preview.state === "loading"
        info.items = preview.state === "loading" ? [] : preview.state === "members" ? [{ type: "user", id: 101, name: "Very long synthetic member name for elision", status: "admin" }]
          : [{ id: 50, chatId: -102, date: model.nowMs / 1000, senderName: "Alex Demo", content: { kind: ({ gifs: "gif", files: "file", links: "text", voice: "voice", music: "audio" })[preview.state] || "photo", text: "Synthetic shared message", entities: [], linkPreview: { url: "https://example.invalid/" }, media: { duration: 24, fileName: "synthetic-document.txt", title: "Synthetic audio title", performer: "Preview performer", file: { id: 50, path: model.imagePath, size: 1500 }, thumb: { file: { id: 50, path: model.imagePath } } } } }]
        settling.start()
      })
    }) })
  }
  Timer {
    id: settling
    interval: 20; repeat: true
    property int settled: 0
    onTriggered: {
      if (Readiness.pending(preview)) { settled = 0; return }
      if (++settled < 4) return
      preview.ready = true; stop()
    }
  }
}
