import QtQuick
import QtTest
import qs.Commons
import "../../app" as App
import "../../shell" as Shell

// Supplemental actual-consumer captures; MCP acceptance remains a separate gate.
TestCase {
  id: tests
  name: "PhotoActionsCapture"
  when: windowShown
  visible: true
  width: 800
  height: 650
  FixtureApp { id: model }
  QtObject {
    id: mediaHost
    property var service: model
    property var keys: ({})
    property string fontFamily: model.fontFamily
    property color background: model.background
    property color foreground: model.foreground
    property color selected: model.selected
    property color accent: model.accent
    property color urgent: model.urgent
    function fileOf(file) { return file }
    function fetchNow() {}
    function fetch() {}
    function urlOf(file) { return file && file.path ? "file://" + file.path : "" }
    function hints() { return "Esc closes" }
  }
  Component { id: fullScene; App.PhotoViewer { width: 800; height: 650; app: model; messages: [model.photoMessage]; messageId: 50 } }
  Component { id: quickScene; Shell.MediaViewer { width: 800; height: 650; host: mediaHost; items: [model.photoMessage]; messageId: 50 } }
  function test_capture_data() {
    return [{ tag: "window-r0", quick: false, radius: 0 }, { tag: "quick-r0", quick: true, radius: 0 },
            { tag: "window-r8", quick: false, radius: 8 }, { tag: "quick-r8", quick: true, radius: 8 }]
  }
  function test_capture(data) {
    Style.cornerRadius = data.radius
    model.photoCanSave = true
    model.photoError = ""
    var view = createTemporaryObject(data.quick ? quickScene : fullScene, tests)
    verify(view !== null)
    // Readiness is the actual image reaching Ready, then the async permissions menu.
    function imageReady(item) {
      if (item.toString().indexOf("QQuickImage") >= 0 && String(item.source) !== "" && item.status !== Image.Ready) return false
      for (var i = 0; i < item.children.length; i++) if (!imageReady(item.children[i])) return false
      return true
    }
    tryVerify(function () { return imageReady(view) })
    view.forceActiveFocus()
    wait(0)
    view.actions.openMenu(710, 570)
    tryCompare(view.actions.menu, "visible", true)
    waitForRendering(view)
    grabImage(view).save("/tmp/omagram-photo-actions-evidence/" + data.tag + "-menu.png")
    view.actions.menu.dismiss()
    view.actions.file = { id: 50, path: "" }
    view.actions.perform("copy")
    waitForRendering(view)
    grabImage(view).save("/tmp/omagram-photo-actions-evidence/" + data.tag + "-loading.png")
    view.actions.file = model.photoMessage.content.media.file
    model.photoError = "Clipboard unavailable"
    view.actions.perform("copy")
    tryCompare(view.actions, "status", "Clipboard unavailable")
    waitForRendering(view)
    grabImage(view).save("/tmp/omagram-photo-actions-evidence/" + data.tag + "-error.png")
    model.photoCanSave = false
    model.photoError = ""
    view.actions.openMenu(400, 300)
    tryCompare(view.actions, "status", "This photo cannot be saved or copied")
    waitForRendering(view)
    grabImage(view).save("/tmp/omagram-photo-actions-evidence/" + data.tag + "-protected.png")
  }
}
