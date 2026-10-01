import QtQuick
import QtTest
import qs.Commons

TestCase {
  id: tests
  name: "ComposerGeometryCaptures"
  when: windowShown
  visible: true
  width: 900
  height: 750
  Component { id: scene; QuickConsumer {} }
  function cleanup() { Style.fontBaseSize = 12; Style.cornerRadius = 0 }
  function test_transition_and_settled_states() {
    for (var radius of [0, 8]) {
      var view = createTemporaryObject(scene, tests, { width: 350, height: 680, fontSize: 14, themeRadius: radius })
      tryCompare(view, "ready", true)
      wait(30)
      grabImage(view).save("/tmp/opencode/omagram-geometry-narrow-r" + radius + ".png")
      view.width = 534
      wait(30)
      grabImage(view).save("/tmp/opencode/omagram-geometry-wide-r" + radius + ".png")
      var box = findChild(view, "quick-composer-box")
      var tool = findChild(view, "quick-tool-attach")
      console.log("Measured", radius, "width", view.width, "boxHeight", box.height,
        "toolY", tool.mapToItem(box, 0, 0).y, "toolHeight", tool.height)
      findChild(view, "quick-composer").text = "A multiline draft\nkeeps the actions inside the editor."
      wait(30)
      grabImage(view).save("/tmp/opencode/omagram-geometry-draft-r" + radius + ".png")
      view.destroy()
    }
  }
}
