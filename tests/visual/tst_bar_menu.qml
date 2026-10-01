import QtQuick
import QtTest
import qs.Commons

TestCase {
  id: tests
  name: "ActualBarMenuContent"
  when: windowShown
  width: 300
  height: 300
  visible: true
  Component { id: scene; BarMenuConsumer {} }
  SignalSpy { id: spy; signalName: "picked" }
  function cleanup() { spy.target = null; spy.clear(); Style.cornerRadius = 0; Style.fontBaseSize = 12 }
  function test_click_keyboard_and_theme() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    spy.target = findChild(view, "bar-menu-content")
    for (var action of ["open", "switch:default", "switch:work", "quiet", "quit"]) {
      var entry = findChild(view, "bar-menu-" + action)
      verify(entry !== null)
      mouseClick(entry, entry.width / 2, entry.height / 2)
      compare(spy.signalArguments[spy.count - 1][0], action)
      entry.forceActiveFocus()
      keyClick(Qt.Key_Return)
      compare(spy.signalArguments[spy.count - 1][0], action)
      Style.cornerRadius = 8
      compare(entry.radius, 8)
      Style.cornerRadius = 0
      compare(entry.radius, 0)
    }
    compare(spy.count, 10)
  }
}
