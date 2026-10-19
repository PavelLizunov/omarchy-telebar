import QtQuick
import QtTest

// Run only via file-portal-runner on its private dbus-run-session bus.
TestCase {
  id: tests
  name: "SystemFilePortal"
  when: windowShown
  visible: true
  width: 1000
  height: 750
  Component { id: scene; FileDialogsConsumer { autoOpen: false } }
  Component { id: quickScene; QuickConsumer {} }
  function test_quick_portal_multiple_and_cancel() {
    var view = createTemporaryObject(quickScene, tests)
    tryCompare(view, "ready", true)
    var picker = findChild(view, "quick-file-picker")
    picker.title = "Cancel fixture"
    view.consumer.attach()
    tryCompare(picker, "visible", false)
    compare(view.consumer.attachments.length, 0)
    picker.title = "Attach photos or files"
    view.consumer.attach()
    tryVerify(function () { return view.consumer.attachments.length === 2 })
    compare(view.consumer.attachments[0].name, "first.txt")
    compare(view.consumer.attachments[1].name, "second.txt")
    compare(picker.visible, false)
  }
  function test_actual_consumers_use_portal_data() {
    return [{ tag: "attachments", kind: "attachments" }, { tag: "profile", kind: "photo" }]
  }
  function test_actual_consumers_use_portal(data) {
    var view = createTemporaryObject(scene, tests, { dialogKind: data.kind })
    tryCompare(view, "ready", true)
    var chosenPhoto = ""
    view.model.requestObserved.connect(function (cmd, args) { if (cmd === "profile.setPhoto") chosenPhoto = args.path })
    if (data.kind === "attachments") {
      view.chat.attach(false)
      tryVerify(function () { return view.chat.attachments.length === 2 })
      compare(view.chat.attachments[0].name, "first.txt")
      compare(view.chat.attachments[1].name, "second.txt")
      compare(view.chat.attachAsMedia, false)
      verify(!view.model.lastRequest || view.model.lastRequest.cmd !== "message.sendFiles")
    } else {
      view.settings.activate({ kind: "profilePhoto" })
      tryVerify(function () { return chosenPhoto.endsWith("/profile.png") })
    }
    tryCompare(view.dialog, "visible", false)
    wait(50)
  }
}
