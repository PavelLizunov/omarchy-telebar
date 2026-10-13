import QtQuick
import QtTest
import qs.Commons as Host
import "../../app" as App

TestCase {
  id: tests
  name: "NativeTelebarRingMark"
  when: windowShown
  width: 480
  height: 260
  visible: true
  Component { id: scene; RingMarkConsumer {} }
  Component { id: markScene; App.RingMark {} }

  function init() { failOnWarning(/.*/) }
  function cleanup() {
    Host.Style.cornerRadius = 0
    Host.Style.barOverrides = ({})
  }
  function test_native_canvas_file_decode_and_reactivity() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    var mark = findChild(view, "native-ring-mark")
    var button = findChild(view, "native-icon-button")
    verify(mark !== null && button !== null)
    compare(mark.size, Host.Style.bar.iconCanvas)
    compare(button.width, Host.Style.bar.iconSlot)
    compare(button.height, Host.Style.bar.sizeHorizontal)
    var slotWidth = button.width
    view.ink = "#30465b"
    compare(mark.color, view.ink)
    Host.Style.barOverrides = { "icon-canvas": 20 }
    compare(mark.size, 20)
    compare(button.width, slotWidth)
    Host.Style.cornerRadius = 8
    compare(mark.size, 20)
    Host.Style.cornerRadius = 0
    compare(mark.size, 20)
  }
  function test_vector_scale_data() {
    return [ { tag: "bar", size: 16 }, { tag: "quick", size: 18 },
      { tag: "small", size: 12 }, { tag: "detail", size: 96 } ]
  }
  function test_vector_scale(data) {
    var mark = createTemporaryObject(markScene, tests, { size: data.size, color: "#5d7590" })
    compare(mark.width, data.size)
    compare(mark.height, data.size)
    compare(mark.implicitWidth, data.size)
    compare(mark.implicitHeight, data.size)
    compare(mark.color, "#5d7590")
  }
}
