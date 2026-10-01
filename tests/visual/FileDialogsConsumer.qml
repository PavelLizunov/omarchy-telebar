import QtQuick
import qs.Commons
import "../../app" as App

Rectangle {
  id: fixture
  width: 900
  height: 650
  color: model.background
  property string dialogKind: "attachments"
  property int themeRadius: 0
  property bool autoOpen: true
  property bool ready: false
  property alias model: model
  property alias chat: chat
  property alias settings: settings
  property var dialog: null
  FixtureApp { id: model }
  App.ChatView { id: chat; anchors.fill: parent; app: model; client: model; chat: model.chats[0]; visible: fixture.dialogKind === "attachments" }
  App.SettingsView { id: settings; anchors.fill: parent; app: model; visible: fixture.dialogKind === "photo" }
  function findDialog(owner, name) {
    for (var object of owner.data) if (object.objectName === name) return object
    return null
  }
  Component.onCompleted: {
    Style.cornerRadius = fixture.themeRadius
    fixture.dialog = findDialog(dialogKind === "attachments" ? chat : settings,
      dialogKind === "attachments" ? "attachment-file-dialog" : "profile-photo-file-dialog")
    if (!fixture.dialog) return
    if (autoOpen) {
      fixture.dialog.currentFolder = Qt.resolvedUrl("dialog-files/")
      fixture.dialog.open()
    }
    settle.start()
  }
  Timer {
    id: settle
    interval: 20
    repeat: true
    onTriggered: if (fixture.dialog && (!fixture.autoOpen || fixture.dialog.ready)) { fixture.ready = true; stop() }
  }
}
