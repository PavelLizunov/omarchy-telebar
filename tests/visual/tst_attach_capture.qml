import QtQuick
import QtTest
import qs.Commons
import "../../shell" as Shell

TestCase {
  id: tests
  name: "AttachConsumerCaptures"
  when: windowShown
  visible: true
  width: 900
  height: 700
  FixtureApp { id: model }
  Component { id: quickScene; QuickConsumer {} }
  Component { id: fullScene; FileDialogsConsumer {} }
  Component { id: mediaScene; Shell.MediaViewer { width: 900; height: 650; items: [model.photoMessage]; messageId: 50; host: mediaHost } }
  QtObject {
    id: mediaHost
    property var keys: ({})
    property color background: model.background
    property color foreground: model.foreground
    property color selected: model.selected
    property color accent: model.accent
    property color urgent: model.urgent
    property string fontFamily: "Sans Serif"
    property var shownChat: model.chats[0]
    function fileOf(file) { return file }
    function fetchNow() {}
    function fetch() {}
    function urlOf(file) { return file && file.path ? "file://" + file.path : "" }
    function stillThumb() { return null }
    function hints() { return "" }
  }
  function cleanup() { Style.cornerRadius = 0; Style.fontBaseSize = 12 }
  function test_final_states() {
    for (var radius of [0, 8]) {
      var quick = createTemporaryObject(quickScene, tests, { width: 350, height: 500, themeRadius: radius })
      tryCompare(quick, "ready", true)
      wait(30)
      grabImage(quick).save("/tmp/opencode/omagram-quick-attach-controls-" + radius + ".png")
      quick.consumer.attach()
      var picker = findChild(quick, "quick-file-picker")
      picker.navigate(Qt.resolvedUrl("dialog-files/"))
      tryCompare(picker, "ready", true)
      wait(30)
      grabImage(quick).save("/tmp/opencode/omagram-quick-attach-picker-" + radius + ".png")
      quick.destroy()
    }
    var full = createTemporaryObject(fullScene, tests, { autoOpen: false })
    tryCompare(full, "ready", true)
    full.chat.attach(true)
    tryCompare(full.dialog, "visible", true)
    full.dialog.navigate(Qt.resolvedUrl("dialog-files/"))
    tryCompare(full.dialog, "ready", true)
    grabImage(full).save("/tmp/opencode/omagram-full-attach-fixed.png")
    full.destroy()
    var media = createTemporaryObject(mediaScene, tests)
    verify(media !== null)
    wait(100)
    grabImage(media).save("/tmp/opencode/omagram-media-open-fixed.png")
  }
}
