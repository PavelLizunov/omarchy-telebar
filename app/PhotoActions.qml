import QtQuick
import "Model.js" as Model

// Shared photo menu; operations use the original message/file, never the preview.
Item {
  id: actions
  property var app
  property var client
  property var message: null
  property var file: null
  property string status: ""
  property int serial: 0
  property bool allowed: false
  property alias menu: menu
  signal downloadRequested(real fileId)

  onMessageChanged: {
    actions.serial++
    actions.allowed = false
    actions.status = ""
    menu.close()
  }
  onVisibleChanged: if (!visible) { actions.serial++; actions.allowed = false; menu.close() }

  function openMenu(x, y) {
    if (!actions.message || !actions.file || !actions.client) return
    var token = ++actions.serial
    actions.allowed = false
    actions.status = ""
    menu.close()
    actions.client.request("message.properties", { chatId: actions.message.chatId, messageId: actions.message.id }, function (answer) {
      if (token !== actions.serial || !actions.visible) return
      if (!answer.ok) { actions.status = answer.error || "Could not check photo permissions"; return }
      if (!answer.result || answer.result.canSave !== true) { actions.status = "This photo cannot be saved or copied"; return }
      actions.allowed = true
      menu.open(x, y)
    })
  }

  function perform(id) {
    if (!actions.allowed || !actions.message || !actions.file || (id !== "save" && id !== "copy")) return
    if (!actions.file.path) {
      actions.downloadRequested(actions.file.id)
      actions.status = "Downloading… try again when it is done"
      return
    }
    var token = actions.serial
    var args = { fileId: actions.file.id, chatId: actions.message.chatId, messageId: actions.message.id }
    if (id === "save") args.fileName = Model.saveName(actions.message)
    actions.status = id === "save" ? "Saving photo…" : "Copying image…"
    actions.client.request(id === "save" ? "file.save" : "file.copyImage", args, function (answer) {
      if (token !== actions.serial || !actions.visible) return
      actions.status = answer.ok ? (id === "save" ? "Saved to Downloads" : "Image copied")
                                : (answer.error || (id === "save" ? "Could not save the photo" : "Could not copy the image"))
    })
  }

  MouseArea {
    anchors.fill: parent
    acceptedButtons: Qt.RightButton
    onClicked: function (mouse) { actions.openMenu(mouse.x, mouse.y) }
  }
  ContextMenu {
    id: menu
    objectName: "photoActionsMenu"
    anchors.fill: parent
    app: actions.app
    onVisibleChanged: if (!visible) {
      // A hidden FocusScope can still keep key focus in Qt Quick.
      var restore = menu.activeFocus
      menu.focus = false
      if (restore) actions.parent.forceActiveFocus()
    }
    items: [{ id: "save", label: "Save to Downloads" }, { id: "copy", label: "Copy image" }]
    onPicked: function (id) { actions.parent.forceActiveFocus(); actions.perform(id) }
    onDismissed: actions.parent.forceActiveFocus()
  }
}
