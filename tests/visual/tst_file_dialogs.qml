import QtQuick
import QtTest
import qs.Commons

TestCase {
  id: tests
  name: "ActualConsumerFilePickers"
  when: windowShown
  visible: true
  width: 1000
  height: 750
  Component { id: scene; FileDialogsConsumer {} }
  function init() { failOnWarning(/DelegateModel::cancel/) }
  function cleanup() { Style.cornerRadius = 0 }
  function chooseFile(view, name) {
    var list = findChild(view.dialog, "file-picker-list")
    tryVerify(function () { return list && list.count > 0 })
    var index = -1
    tryVerify(function () {
      for (var i = 0; i < list.count; i++) {
        list.positionViewAtIndex(i, ListView.Contain)
        wait(5)
        var item = list.itemAtIndex(i)
        if (item && item.fileName === name) { index = i; return true }
      }
      return false
    }, 3000, "Expected fixture file: " + name)
    var entry = list.itemAtIndex(index)
    mouseClick(entry, entry.width / 2, entry.height / 2)
  }
  function test_open_close_and_cancel() {
    for (var kind of ["attachments", "photo"]) {
      var view = createTemporaryObject(scene, tests, { dialogKind: kind })
      verify(view !== null)
      tryCompare(view, "ready", true)
      compare(view.dialog.multiple, kind === "attachments")
      keyClick(Qt.Key_Escape)
      tryCompare(view.dialog, "visible", false)
      compare(view.chat.attachments.length, 0)
      verify(!view.model.lastRequest || view.model.lastRequest.cmd !== "profile.setPhoto")
      for (var i = 0; i < 3; i++) {
        view.dialog.open()
        tryCompare(view.dialog, "visible", true)
        view.dialog.reject()
        tryCompare(view.dialog, "visible", false)
      }
      view.destroy()
    }
  }
  function test_first_open_from_attachment_controls_without_folder() {
    var view = createTemporaryObject(scene, tests, { autoOpen: false })
    tryCompare(view, "ready", true)
    compare(String(view.dialog.currentFolder), "")
    var attach = findChild(view.chat, "composer-action-attach")
    verify(attach !== null)
    mouseClick(attach, attach.width / 2, attach.height / 2)
    tryCompare(view.dialog, "visible", true)
    verify(String(view.dialog.currentFolder).startsWith("file:///"))
    view.dialog.reject()
    view.width = 330
    wait(30)
    var overflow = findChild(view.chat, "composer-action-overflow")
    verify(overflow !== null)
    mouseClick(overflow, overflow.width / 2, overflow.height / 2)
    var menu = findChild(view.chat, "composer-overflow-menu")
    tryCompare(menu, "visible", true)
    keyClick(Qt.Key_Return)
    tryCompare(view.dialog, "visible", true)
  }
  function test_first_profile_photo_open_without_folder() {
    var view = createTemporaryObject(scene, tests, { autoOpen: false, dialogKind: "photo" })
    tryCompare(view, "ready", true)
    compare(String(view.dialog.currentFolder), "")
    view.settings.activate({ kind: "profilePhoto" })
    tryCompare(view.dialog, "visible", true)
    verify(String(view.dialog.currentFolder).startsWith("file:///"))
    keyClick(Qt.Key_Escape)
    tryCompare(view.dialog, "visible", false)
  }
  function test_multiple_attachment_selection_and_accept() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    chooseFile(view, "first.txt")
    chooseFile(view, "second.txt")
    compare(view.dialog.selectedFiles.length, 2)
    var button = findChild(view.dialog, "file-picker-accept")
    mouseClick(button, button.width / 2, button.height / 2)
    tryCompare(view.dialog, "visible", false)
    compare(view.chat.attachments.length, 2)
    compare(view.chat.attachments[0].name, "first.txt")
    compare(view.chat.attachments[1].name, "second.txt")
  }
  function test_profile_photo_selection_and_accept() {
    // Create an inert image locally; grabImage/save returns void on this Qt version.
    grabImage(tests).save("/tmp/opencode/omagram-dialog-profile.png")
    var view = createTemporaryObject(scene, tests, { dialogKind: "photo" })
    tryCompare(view, "ready", true)
    view.dialog.navigate("file:///tmp/opencode")
    chooseFile(view, "omagram-dialog-profile.png")
    view.dialog.accept()
    tryCompare(view.dialog, "visible", false)
    compare(view.model.lastRequest.cmd, "profile.setPhoto")
    compare(view.model.lastRequest.args.path, "/tmp/opencode/omagram-dialog-profile.png")
    wait(50) // Let the inert profile reload finish before this temporary consumer is destroyed.
  }
  function test_path_and_theme_and_directory_navigation() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    var card = findChild(view.dialog, "file-picker-card")
    for (var radius of [0, 8, 0]) { Style.cornerRadius = radius; compare(card.radius, radius) }
    var path = findChild(view.dialog, "file-picker-path")
    path.text = "relative/path"
    view.dialog.goToPath()
    verify(view.dialog.error !== "")
    path.text = decodeURIComponent(String(Qt.resolvedUrl("dialog-files/"))).replace("file://", "")
    view.dialog.goToPath()
    compare(view.dialog.error, "")
    var up = findChild(view.dialog, "file-picker-up")
    mouseClick(up, up.width / 2, up.height / 2)
    tryVerify(function () { return String(view.dialog.currentFolder).endsWith("/visual") })
  }
}
