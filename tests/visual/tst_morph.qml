import QtQuick
import QtTest
import "../../app" as App

TestCase {
  name: "OriginalIconMorph"
  when: windowShown
  width: 200
  height: 200
  visible: true
  App.MorphIcon { id: morph; width: 48; height: 48; name: "play" }
  function test_play_pause_settles_and_returns() {
    compare(morph.progress, 0)
    morph.name = "pause"
    wait(70)
    verify(morph.progress > 0 && morph.progress < 1, "Matched paths must pass through intermediate geometry")
    tryCompare(morph, "progress", 1, 500)
    morph.name = "play"
    tryCompare(morph, "progress", 0, 500)
  }
  function test_panel_motion_can_be_disabled() {
    morph.animated = false
    morph.name = "panelOpen"
    compare(morph.progress, 1)
    morph.name = "panelClose"
    compare(morph.progress, 0)
    morph.name = "play"
    morph.animated = true
  }
}
