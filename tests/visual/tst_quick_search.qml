import QtQuick
import QtTest
import qs.Commons

TestCase {
  id: tests
  name: "CompactChatSearch"
  when: windowShown
  visible: true
  width: 650
  height: 700
  Component { id: scene; QuickConsumer {} }
  SignalSpy { id: dismissSpy; signalName: "dismissRequested" }
  function cleanup() { dismissSpy.target = null; dismissSpy.clear(); Style.fontBaseSize = 12; Style.cornerRadius = 0 }

  function test_search_pointer_escape_and_composition() {
    var view = createTemporaryObject(scene, tests, { width: 350, height: 500 })
    tryCompare(view, "ready", true)
    var quick = view.consumer
    var input = findChild(view, "quick-composer")
    var button = findChild(view, "quick-search-button")
    var search = findChild(view, "quick-search-input")
    var list = findChild(view, "quick-chat-list")
    dismissSpy.target = quick
    input.text = "Unsent reply"
    compare(quick.draftPresent, true)
    quick.addAttachments([view.serviceModel.imagePath], true)
    var serial = quick.sendSerial
    mouseClick(button, button.width / 2, button.height / 2)
    tryCompare(search, "activeFocus", true)
    verify(search.visible)
    compare(quick.focusItem, search)
    compare(quick.draftPresent, true)
    compare(quick.replyChatId, 101)
    compare(quick.sendSerial, serial)
    search.text = "Community"
    tryCompare(list, "count", 1)
    compare(quick.highlighted.id, -102)
    keyClick(Qt.Key_Escape)
    compare(search.text, "")
    compare(quick.searchOpen, true)
    keyClick(Qt.Key_Escape)
    tryCompare(input, "activeFocus", true)
    compare(quick.searchOpen, false)
    compare(input.text, "Unsent reply")
    compare(quick.attachments.length, 1)
    compare(dismissSpy.count, 0)
  }

  function test_search_enter_keeps_pending_reply_in_original_chat_data() {
    return [{ tag: "draft", pending: "draft" }, { tag: "files", pending: "files" },
            { tag: "send", pending: "send" }, { tag: "recording", pending: "recording" }]
  }
  function test_search_enter_keeps_pending_reply_in_original_chat(data) {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    var quick = view.consumer
    var input = findChild(view, "quick-composer")
    var search = findChild(view, "quick-search-input")
    if (data.pending === "draft") input.text = "Only for Alex"
    else if (data.pending === "files") quick.addAttachments([view.serviceModel.imagePath], true)
    else if (data.pending === "send") quick.sending = true
    else view.serviceModel.recording = { state: "voice", chatId: 101, startedAt: Date.now() }
    var serial = quick.sendSerial
    quick.focusSearch()
    tryCompare(search, "activeFocus", true)
    search.text = "Community"
    keyClick(Qt.Key_Return)
    compare(quick.replyChatId, 101)
    compare(quick.searchOpen, false)
    compare(quick.sendSerial, serial)
    verify(quick.status.indexOf("current reply") >= 0)
    if (data.pending === "draft") compare(input.text, "Only for Alex")
    else if (data.pending === "files") compare(quick.attachments.length, 1)
    else if (data.pending === "send") compare(quick.sending, true)
    else compare(quick.recordingHere, true)
  }

  function test_send_completion_keeps_search_focus() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    var quick = view.consumer
    var input = findChild(view, "quick-composer")
    var search = findChild(view, "quick-search-input")
    input.text = "Sending to Alex"
    view.serviceModel.holdSend = true
    quick.send()
    quick.focusSearch()
    tryCompare(search, "activeFocus", true)
    search.text = "Community"
    view.serviceModel.completeSend()
    compare(quick.sending, false)
    compare(input.text, "")
    compare(search.activeFocus, true)
    compare(quick.replyChatId, 101)
  }

  function test_search_keyboard_switch_and_back() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    var quick = view.consumer
    var input = findChild(view, "quick-composer")
    var button = findChild(view, "quick-search-button")
    var search = findChild(view, "quick-search-input")
    button.forceActiveFocus()
    keyClick(Qt.Key_Space)
    tryCompare(search, "activeFocus", true)
    search.text = "Community"
    keyClick(Qt.Key_Return)
    tryCompare(quick, "replyChatId", -102)
    compare(quick.searchOpen, false)
    tryCompare(input, "activeFocus", true)
    button.forceActiveFocus()
    keyClick(Qt.Key_Return)
    tryCompare(search, "activeFocus", true)
    var back = findChild(view, "quick-search-back")
    mouseClick(back, back.width / 2, back.height / 2)
    tryCompare(input, "activeFocus", true)
    compare(quick.replyChatId, -102)
  }

  function test_search_geometry_data() {
    return [{ tag: "narrow", width: 350, font: 15, radius: 0 },
            { tag: "scaled", width: 350, font: 18, radius: 8 },
            { tag: "normal", width: 500, font: 15, radius: 0 }]
  }
  function test_search_geometry(data) {
    var view = createTemporaryObject(scene, tests, { width: data.width, height: 500, fontSize: data.font, themeRadius: data.radius })
    tryCompare(view, "ready", true)
    var button = findChild(view, "quick-search-button")
    var opener = findChild(view, "quick-open-chat-window")
    var position = button.mapToItem(view, 0, 0)
    var openPosition = opener.mapToItem(view, 0, 0)
    verify(position.x >= 0 && position.x + button.width <= openPosition.x)
    mouseClick(button, button.width / 2, button.height / 2)
    var search = findChild(view, "quick-search-input")
    tryCompare(search, "activeFocus", true)
    verify(search.width > 120)
    var top = search.mapToItem(view, 0, 0)
    verify(top.x >= 0 && top.x + search.width <= view.width)
    search.text = "no-synthetic-chat-matches"
    compare(view.consumer.results.length, 0)
    search.text = "alex"
    compare(view.consumer.results.length, 1)
  }
}
