import QtQuick
import QtTest
import qs.Commons
import "../../app" as App

TestCase {
  id: tests
  name: "SidebarInteractionAndTheme"
  when: windowShown
  width: 900
  height: 650
  visible: true
  FixtureApp { id: model; selected: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.08) }
  Component { id: scene; Sidebar {} }
  Component { id: quickScene; QuickReply {} }
  Component {
    id: rowComponent
    App.MessageRow {
      width: 460
      app: model
      view: ({ chat: model.chats[0], nowMs: model.nowMs, cursor: -1, messagesFocused: false, revealed: {}, selection: {}, translations: {}, pollChoices: {}, confirmDeleteId: 0 })
      mid: 1
      index: 0
      messages: [{ id: 1, chatId: 101, date: 1790726000, outgoing: false, content: { kind: "text", text: "Theme corners", entities: [] }, reactions: [] }]
    }
  }
  function cleanup() { Style.fontBaseSize = 12; Style.cornerRadius = 0 }
  function test_click_drag_no_accidental_toggle_and_bounds() {
    var view = createTemporaryObject(scene, tests)
    verify(view !== null)
    tryCompare(view, "ready", true)
    var grip = findChild(view, "sidebar-grip-hit")
    mouseClick(grip, grip.width / 2, grip.height / 2)
    tryCompare(view, "sidebarWidth", 72)
    wait(30)
    mouseClick(grip, grip.width / 2, grip.height / 2)
    tryCompare(view, "sidebarWidth", 300)
    wait(30)
    var at = grip.mapToItem(tests, grip.width / 2, grip.height / 2)
    mousePress(tests, at.x, at.y)
    mouseMove(tests, at.x + 55, at.y, 20)
    mouseRelease(tests, at.x + 55, at.y)
    tryCompare(view, "sidebarWidth", 355)
    // After a drag a subsequent click still toggles normally.
    mouseClick(grip, grip.width / 2, grip.height / 2)
    tryCompare(view, "sidebarWidth", 72)
    wait(30)
    at = grip.mapToItem(tests, grip.width / 2, grip.height / 2)
    mousePress(tests, at.x, at.y)
    mouseMove(tests, at.x + 500, at.y, 20)
    mouseRelease(tests, at.x + 500, at.y)
    tryCompare(view, "sidebarWidth", 405)
    wait(30)
    var button = findChild(view, "sidebar-button")
    button.forceActiveFocus()
    keyClick(Qt.Key_Space)
    tryCompare(view, "sidebarWidth", 72)
    wait(30)
    button.forceActiveFocus()
    verify(button.activeFocus, "Handle must own keyboard focus")
    keyClick(Qt.Key_Right)
    tryCompare(view, "sidebarWidth", 180)
    wait(30)
    var rail = findChild(view, "sidebar-rail-hit")
    at = rail.mapToItem(tests, rail.width / 2, 200)
    mousePress(tests, at.x, at.y)
    mouseMove(tests, at.x + 35, at.y, 20)
    mouseRelease(tests, at.x + 35, at.y)
    tryCompare(view, "sidebarWidth", 215)
  }
  function test_theme_radius_updates_without_recreating_surfaces() {
    var view = createTemporaryObject(scene, tests)
    var row = createTemporaryObject(rowComponent, tests)
    verify(view !== null && row !== null)
    tryCompare(view, "ready", true)
    var button = findChild(view, "sidebar-button")
    for (var radius of [0, 8, 0]) {
      Style.cornerRadius = radius
      compare(button.radius, radius)
      compare(row.bubbleItem.radius, radius)
      compare(button.color.a, 1)
    }
    for (var size of [12, 15]) {
      Style.fontBaseSize = size
      compare(button.width, Style.space(28))
      compare(findChild(view, "sidebar-icon").width, Style.space(16))
      compare(findChild(view, "sidebar-grip-hit").width, Style.space(32))
    }
  }
  function test_quick_reply_theme_radius_and_hover_backing() {
    var view = createTemporaryObject(scene, tests)
    var quick = createTemporaryObject(quickScene, tests)
    verify(view !== null && quick !== null)
    tryCompare(quick, "ready", true, 3000)
    var background = findChild(quick, "quick-message-1")
    verify(background !== null)
    for (var radius of [0, 8, 0]) {
      Style.cornerRadius = radius
      compare(background.radius, radius)
    }
    var button = findChild(view, "sidebar-button")
    mouseMove(button, button.width / 2, button.height / 2)
    wait(30)
    compare(button.color.a, 1)
    var image = grabImage(button)
    compare(image.pixel(14, 4), image.pixel(12, 4), "Divider must not show through hover backing")
  }
}
