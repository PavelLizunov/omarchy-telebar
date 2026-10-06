import QtQuick
import QtTest
import qs.Commons
import "Readiness.js" as Readiness

TestCase {
  id: tests
  name: "TelebarRenameConsumers"
  when: windowShown
  visible: true
  width: 900
  height: 760
  Component { id: screenScene; Preview {} }
  Component { id: menuScene; BarMenuConsumer {} }
  Component { id: quickScene; QuickConsumer {} }
  function init() { failOnWarning(/.*/) }
  function cleanup() { Style.cornerRadius = 0; Style.fontBaseSize = 12 }
  function hasText(item, text) {
    if (item.text !== undefined && item.text === text) return true
    for (var child of item.children || []) if (hasText(child, text)) return true
    return false
  }
  function test_branded_consumers() {
    for (var radius of [0, 8]) {
      for (var state of ["setup", "login"]) {
        var view = createTemporaryObject(screenScene, tests, { scene: state, themeRadius: radius, width: 700, height: 650 })
        verify(view !== null)
        tryCompare(view, "ready", true)
        tryVerify(function () { return hasText(view, state === "setup" ? "Telebar" : "Telebar — sign in") })
        tryVerify(function () { return !Readiness.pending(view) })
        waitForRendering(view)
        var capture = grabImage(view)
        compare(capture.width, 700)
        capture.save("/tmp/telebar-rename-evidence/" + state + "-radius" + radius + ".png")
        view.destroy()
      }
      var menu = createTemporaryObject(menuScene, tests, { themeRadius: radius })
      tryCompare(menu, "ready", true)
      verify(hasText(menu, "Open Telebar"))
      waitForRendering(menu)
      grabImage(menu).save("/tmp/telebar-rename-evidence/menu-radius" + radius + ".png")
      menu.destroy()
      var quick = createTemporaryObject(quickScene, tests, { themeRadius: radius })
      tryCompare(quick, "ready", true)
      quick.consumer.leave()
      quick.serviceModel.ready = false
      quick.serviceModel.connected = false
      tryVerify(function () { return hasText(quick, "Connecting to Telebar…") })
      waitForRendering(quick)
      grabImage(quick).save("/tmp/telebar-rename-evidence/quick-connecting-radius" + radius + ".png")
      quick.destroy()
    }
  }
}
