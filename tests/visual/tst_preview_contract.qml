import QtQuick
import QtTest
import "Readiness.js" as Readiness

TestCase {
  id: tests
  name: "PreviewReadinessContract"
  when: windowShown
  visible: true
  width: 700
  height: 650
  Component { id: sendScene; SendContractConsumer {} }
  Component { id: mentionScene; MentionContextConsumer {} }
  function test_consumers_match_actual_radius_data() {
    return [{tag: "send-square", scene: sendScene, radius: 0},
            {tag: "send-rounded", scene: sendScene, radius: 8},
            {tag: "mention-square", scene: mentionScene, radius: 0},
            {tag: "mention-rounded", scene: mentionScene, radius: 8}]
  }
  function test_consumers_match_actual_radius(data) {
    var scene = createTemporaryObject(data.scene, tests, {themeRadius: data.radius})
    verify(scene !== null)
    var box = findChild(scene, "composer-box")
    verify(box !== null)
    compare(box.radius, data.radius)
    verify(Readiness.matchesRadius(scene, "composer-box", data.radius))
    tryCompare(scene, "ready", true, 1500)
  }
}
