import QtQuick
import QtQuick.Dialogs
import QtTest

// Non-native offscreen route checks are separate from the real portal handoff.
TestCase {
  id: tests
  name: "ActualConsumerFilePickers"
  when: windowShown
  visible: true
  width: 1000
  height: 750
  Component { id: scene; FileDialogsConsumer { autoOpen: false } }
  function consumer(kind) {
    var view = createTemporaryObject(scene, tests, { dialogKind: kind || "attachments" })
    verify(view !== null)
    tryCompare(view, "ready", true)
    view.dialog.options = FileDialog.DontUseNativeDialog
    return view
  }
  function test_native_default_modes_and_cancel() {
    for (var kind of ["attachments", "photo"]) {
      var view = consumer(kind)
      compare(view.dialog.fileMode, kind === "attachments" ? FileDialog.OpenFiles : FileDialog.OpenFile)
      verify(view.dialog.nameFilters[0].indexOf("(") >= 0)
      for (var i = 0; i < 3; i++) {
        view.dialog.open()
        tryCompare(view.dialog, "visible", true)
        view.dialog.reject()
        tryCompare(view.dialog, "visible", false)
      }
      compare(view.chat.attachments.length, 0)
      verify(!view.model.lastRequest || view.model.lastRequest.cmd !== "profile.setPhoto")
    }
  }
  function test_chat_replacement_closes_chooser() {
    var view = consumer()
    view.chat.attach(true)
    tryCompare(view.dialog, "visible", true)
    view.chat.chat = view.model.chats[1]
    tryCompare(view.dialog, "visible", false)
    compare(view.chat.attachments.length, 0)
  }
  function test_attachment_control_and_selection() {
    var view = consumer()
    var attach = findChild(view.chat, "composer-action-attach")
    verify(attach !== null)
    mouseClick(attach, attach.width / 2, attach.height / 2)
    tryCompare(view.dialog, "visible", true)
    view.dialog.reject()
    // Route acceptance explicitly; native transport acceptance has its own private-bus test.
    view.dialog.selectedFile = Qt.resolvedUrl("dialog-files/first.txt")
    view.dialog.accepted()
    tryCompare(view.dialog, "visible", false)
    compare(view.chat.attachments.length, 1)
    compare(view.chat.attachments[0].name, "first.txt")
    verify(!view.model.lastRequest || view.model.lastRequest.cmd !== "message.sendFiles", "Choosing must not send")
  }
  function test_profile_photo_selection() {
    var view = consumer("photo")
    view.settings.activate({ kind: "profilePhoto" })
    tryCompare(view.dialog, "visible", true)
    view.dialog.reject()
    view.dialog.selectedFile = Qt.resolvedUrl("sample-photo.svg")
    view.dialog.accepted()
    tryCompare(view.dialog, "visible", false)
    compare(view.model.lastRequest.cmd, "profile.setPhoto")
    compare(view.model.lastRequest.args.path, decodeURIComponent(String(Qt.resolvedUrl("sample-photo.svg")).replace("file://", "")))
    wait(50)
  }
}
