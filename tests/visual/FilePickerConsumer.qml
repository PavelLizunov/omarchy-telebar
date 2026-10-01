import QtQuick
import qs.Commons
import "../../app" as App

Rectangle {
  id: preview
  objectName: "review-root"
  color: model.background
  property string dialogKind: "attachments"
  property int themeRadius: 0
  property bool ready: false
  FixtureApp { id: model }
  Component.onCompleted: { Style.cornerRadius = themeRadius; Style.fontBaseSize = 15 }
  Loader {
    id: consumer
    anchors.fill: parent
    sourceComponent: preview.dialogKind === "photo" ? settingsPage : chatPage
    onLoaded: Qt.callLater(function () {
      item.objectName = "review-content"
      for (var object of item.data) if (object.objectName === (preview.dialogKind === "photo" ? "profile-photo-file-dialog" : "attachment-file-dialog")) {
        object.currentFolder = Qt.resolvedUrl("dialog-files/")
        object.open()
        settling.picker = object
        settling.start()
      }
    })
  }
  Timer {
    id: settling
    property var picker: null
    interval: 20
    repeat: true
    onTriggered: if (picker && picker.ready) { preview.ready = true; stop() }
  }
  Component { id: settingsPage; App.SettingsView { app: model } }
  Component { id: chatPage; App.ChatView { app: model; client: model; chat: model.chats[0] } }
}
