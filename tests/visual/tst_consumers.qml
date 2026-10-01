import QtQuick
import QtTest
import qs.Commons
import "../../app" as App

TestCase {
  id: tests
  name: "InertConsumers"
  when: windowShown
  visible: true
  width: 900
  height: 650
  FixtureApp { id: model }
  Component { id: settingsComponent; App.SettingsView { app: model; width: 900; height: 650; visible: true } }
  Component { id: menuComponent; App.ContextMenu { app: model; width: 900; height: 650; items: [{ id: "reply", label: "Reply" }, { id: "delete", label: "Delete", danger: true }] } }
  Component { id: topicComponent; App.TopicList { app: model; client: model; chat: model.chats[1]; width: 900; height: 650 } }
  Component { id: loginComponent; App.LoginView { app: model; client: model; width: 900; height: 650 } }
  Component { id: chatComponent; App.ChatView { app: model; client: model; chat: model.chats[0]; width: 462; height: 650 } }
  Component { id: accountsComponent; SearchConsumer { accountMenu: true; width: 350; height: 500 } }
  SignalSpy { id: spy }

  function cleanup() { spy.target = null; spy.signalName = ""; spy.clear(); Style.fontBaseSize = 12; model.recording = { state: "idle" } }

  function test_narrow_resize_draft_send_and_overflow() {
    Style.fontBaseSize = 15
    var view = createTemporaryObject(chatComponent, tests)
    verify(view !== null)
    var input = findChild(view, "composer-text")
    var box = findChild(view, "composer-box")
    tryVerify(function () { return box.width > view.width / 2 }, 1000)
    verify(view.compactControls)
    view.setComposerText("Unsaved draft survives resizing")
    for (var w of [330, 462, 820, 462]) {
      view.width = w
      wait(20)
      compare(input.text, "Unsaved draft survives resizing")
      verify(box.width > 140)
      verify(input.width >= 140)
      verify(findChild(view, "composer-actions").x >= box.x + box.width)
      verify(findChild(view, "composer-action-send") !== null)
    }
    view.composerAction("overflow", null)
    var menu = findChild(view, "composer-overflow-menu")
    verify(menu.visible)
    verify(view.modalOpen)
    compare(menu.items.length, 8)
    menu.cursor = 2
    keyClick(Qt.Key_Return)
    compare(menu.visible, false)
    compare(view.stickersOpen, true)
    view.stickersOpen = false
    view.composerAction("overflow", null)
    keyClick(Qt.Key_Escape)
    verify(input.activeFocus)
    var sendButton = findChild(view, "composer-action-send")
    mouseClick(sendButton, sendButton.width / 2, sendButton.height / 2)
    compare(model.lastRequest.cmd, "message.send")
    compare(model.lastRequest.args.text, "Unsaved draft survives resizing")
    tryCompare(input, "text", "")
    var headerButton = findChild(view, "header-action-overflow")
    headerButton.forceActiveFocus()
    keyClick(Qt.Key_Space)
    var headerMenu = findChild(view, "chat-actions-menu")
    verify(headerMenu.visible)
    keyClick(Qt.Key_Return)
    compare(view.infoOpen, true)
    view.closeInfo()
    view.openInfo()
    wait(20)
    var info = findChild(view, "chat-info-panel")
    compare(info.width, view.width)
    verify(box.width > 140)
    view.closeInfo()
    verify(input.activeFocus)
  }

  function test_settings_initial_load_and_escape() {
    var view = createTemporaryObject(settingsComponent, tests)
    verify(view !== null)
    tryVerify(function () { return view.profile !== null }, 1000)
    compare(view.profile.firstName, "Alex")
    spy.signalName = "closed"
    spy.target = view
    view.forceActiveFocus()
    keyClick(Qt.Key_Escape)
    compare(spy.count, 1)
  }
  function test_story_visibility_setting() {
    var view = createTemporaryObject(settingsComponent, tests)
    tryVerify(function () { return view.profile !== null })
    model.showStories = true
    var row = view.rows.find(function (r) { return r.kind === "showStories" })
    verify(row !== undefined)
    view.activate(row)
    tryCompare(model, "showStories", false)
    compare(model.lastRequest.cmd, "settings.set")
    view.activate(row)
    tryCompare(model, "showStories", true)
  }

  function test_narrow_emoji_search_and_settings_cancel() {
    Style.fontBaseSize = 15
    var view = createTemporaryObject(chatComponent, tests, { width: 462, height: 500 })
    view.openEmoji()
    var search = findChild(view, "emoji-search")
    tryVerify(function () { return search.width > 300 && search.activeFocus })
    keyClick(Qt.Key_Escape)
    compare(view.emojiOpen, false)
    var settings = createTemporaryObject(settingsComponent, tests, { width: 420, height: 500 })
    tryVerify(function () { return settings.profile !== null })
    settings.startEditing({ field: "bio", label: "Bio" })
    verify(settings.editing !== null)
    keyClick(Qt.Key_Escape)
    compare(settings.editing, null)
    wait(30)
    settings.ask("A long confirmation about removing cached files on this computer, without affecting the originals on Telegram.", function () { fail("Cancelled action must not run") })
    settings.forceActiveFocus()
    verify(settings.activeFocus)
    keyClick(Qt.Key_Escape)
    compare(settings.confirm, null)
  }
  function test_accounts_scroll_and_escape() {
    var view = createTemporaryObject(accountsComponent, tests)
    tryCompare(view, "ready", true)
    var menu = findChild(view, "account-menu")
    verify(menu.visible)
    verify(menu.y + menu.height <= view.height)
    var scroll = menu.children[0]
    verify(scroll.contentHeight > scroll.height)
    scroll.contentY = scroll.contentHeight - scroll.height
    verify(scroll.contentY > 0)
    menu.forceActiveFocus()
    keyClick(Qt.Key_Escape)
    compare(menu.visible, false)
  }

  function test_context_menu_keyboard_pick_and_dismiss() {
    var view = createTemporaryObject(menuComponent, tests)
    verify(view !== null)
    spy.signalName = "picked"
    spy.target = view
    view.open(20,20)
    verify(view.visible)
    keyClick(Qt.Key_Down)
    compare(view.cursor, 1)
    keyClick(Qt.Key_Return)
    compare(spy.count, 1)
    compare(spy.signalArguments[0][0], "delete")
    compare(view.visible, false)
    spy.clear()
    spy.signalName = "dismissed"
    view.open(20,20)
    keyClick(Qt.Key_Escape)
    compare(spy.count, 1)
  }

  function test_topics_load_and_open_selected() {
    var view = createTemporaryObject(topicComponent, tests)
    verify(view !== null)
    tryVerify(function () { return view.list.length === 3 }, 1000)
    spy.signalName = "opened"
    spy.target = view
    view.forceActiveFocus()
    keyClick(Qt.Key_Down)
    keyClick(Qt.Key_Return)
    compare(spy.count, 1)
    compare(spy.signalArguments[0][0].chatId, -102)
    verify(spy.signalArguments[0][0].id > 0)
  }

  function test_login_method_and_password_state() {
    model.auth = { state: "qr" }
    var view = createTemporaryObject(loginComponent, tests)
    verify(view !== null)
    tryCompare(view, "step", "qr")
    compare(view.showingQr, true)
    view.switchMethod()
    compare(view.askingPhone, true)
    model.auth = { state: "password", hint: "Example" }
    tryCompare(view, "step", "password")
    compare(view.phoneInstead, false)
    compare(view.showingQr, false)
    model.auth = { state: "phone" }
  }
}
