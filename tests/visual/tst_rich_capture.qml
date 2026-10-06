import QtQuick
import QtTest
import qs.Commons
import "Readiness.js" as Readiness

TestCase {
  id: tests
  name: "RichConsumerSupplementalCaptures"
  when: windowShown
  visible: true
  width: 800
  height: 700
  Component { id: scene; RichConsumer {} }
  function init() { failOnWarning(/.*/) }
  function cleanup() { Style.cornerRadius = 0; Style.fontBaseSize = 12 }
  function test_reading_order() {
    for (var compact of [false, true]) {
      for (var radius of [0, 8]) {
        var view = createTemporaryObject(scene, tests, { compact: compact, themeRadius: radius, width: compact ? 380 : 760 })
        verify(view !== null)
        tryCompare(view, "ready", true)
        tryVerify(function () { return !Readiness.pending(view) })
        waitForRendering(view)
        var path = "/tmp/omagram-rich-evidence/" + (compact ? "quick" : "window") + "-radius" + radius + ".png"
        grabImage(view).save(path)
        view.destroy()
      }
    }
  }
}
