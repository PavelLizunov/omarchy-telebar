import QtQuick
import QtTest
import qs.Commons

// Supplemental QtTest captures while MCP wrapper/renderer versions disagree.
TestCase {
  id: tests
  name: "FilePickerCapture"
  when: windowShown
  visible: true
  width: 900
  height: 650
  Component { id: pickerScene; FileDialogsConsumer {} }
  Component { id: settingsScene; SettingsConsumer { state: "stories"; storiesShown: false } }
  function cleanup() { Style.cornerRadius = 0; Style.fontBaseSize = 12 }
  function test_capture_final_consumers() {
    for (var radius of [0, 8]) {
      var view = createTemporaryObject(pickerScene, tests, { themeRadius: radius })
      tryCompare(view, "ready", true)
      wait(50)
      grabImage(view).save("/tmp/opencode/omagram-filepicker-radius-" + radius + ".png")
      view.destroy()
    }
    var photo = createTemporaryObject(pickerScene, tests, { dialogKind: "photo", themeRadius: 8, width: 444, height: 600 })
    tryCompare(photo, "ready", true)
    grabImage(photo).save("/tmp/opencode/omagram-filepicker-photo.png")
    photo.destroy()
    var settings = createTemporaryObject(settingsScene, tests, { width: 552, height: 650 })
    tryCompare(settings, "ready", true)
    grabImage(settings).save("/tmp/opencode/omagram-stories-hidden.png")
  }
}
