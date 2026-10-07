import QtQuick
import QtTest
import qs.Commons
import "../../app" as App
import "../../shell" as Shell

TestCase {
  id: tests
  name: "MessageFragmentSelection"
  when: windowShown
  visible: true
  width: 650
  height: 700
  Component { id: serviceScene; Shell.Service {} }
  Component { id: quickScene; QuickConsumer {} }
  Component { id: richScene; RichConsumer {} }
  Component { id: chatScene; App.ChatView { width: 600; height: 650; app: model; client: model; chat: model.chats[0]; history: model.history } }
  FixtureApp { id: model }
  TextEdit { id: clipboardProbe; visible: false }
  function cleanup() { Style.fontBaseSize = 12; Style.cornerRadius = 0; model.history = []; model.userStatuses = ({}) }

  function dragFragment(text, start, end) {
    var a = text.positionToRectangle(start)
    var b = text.positionToRectangle(end)
    mousePress(text, a.x + 1, a.y + a.height / 2)
    for (var i = 1; i <= 8; i++) mouseMove(text, a.x + (b.x - a.x) * i / 8 + 1, a.y + a.height / 2, 15)
    mouseRelease(text, b.x + 1, b.y + b.height / 2)
    compare(text.selectedText, "fragment")
    verify(text.activeFocus)
    keyClick(Qt.Key_C, Qt.ControlModifier)
    clipboardProbe.text = ""
    clipboardProbe.paste()
    compare(clipboardProbe.text, "fragment", "Ctrl+C must copy only the dragged fragment")
    var original = text.text
    keyClick(Qt.Key_X)
    keyClick(Qt.Key_Delete)
    compare(text.text, original, "Message text must remain read-only")
  }
  function test_main_fragment_and_message_menu() {
    model.history = [{ id: 70, chatId: 101, date: 1790726000, outgoing: false,
      content: { kind: "text", text: "Select fragment, not the entire message. Safe link", entities: [{ type: "textUrl", offset: 40, length: 9, url: "https://example.org" }] }, reactions: [] }]
    var view = createTemporaryObject(chatScene, tests)
    var text = null
    tryVerify(function () { text = findChild(view, "message-text-70"); return text && text.visible })
    wait(30)
    dragFragment(text, 7, 15)
    var peer = findChild(view, "chat-peer-status")
    model.userStatuses = { 101: { state: "online" } }
    tryCompare(peer, "text", "online")
    verify(findChild(view, "avatar-online-101").visible)
    model.userStatuses = { 101: { state: "recently" } }
    tryCompare(peer, "text", "last seen recently")
    verify(!findChild(view, "avatar-online-101").visible)
    model.userStatuses = ({})
    mouseClick(text, 2, 2, Qt.RightButton)
    var menu = findChild(view, "chat-context-menu")
    verify(menu.visible)
    var copyIndex = menu.items.findIndex(function (item) { return item.id === "copy" })
    verify(copyIndex >= 0)
    compare(menu.items[copyIndex].label, "Copy selected text")
    menu.pick(copyIndex)
    clipboardProbe.text = ""
    clipboardProbe.paste()
    compare(clipboardProbe.text, "fragment")
    var link = text.positionToRectangle(42)
    mouseClick(text, link.x + 1, link.y + link.height / 2)
    compare(model.lastRequest.cmd, "link.open")
    mouseClick(text, 2, 2, Qt.RightButton)
    verify(findChild(view, "chat-context-menu").visible)
    keyClick(Qt.Key_Escape)
    mouseClick(text, 2, 2, Qt.LeftButton, Qt.ControlModifier)
    verify(view.selection[70])
  }
  function test_compact_long_message_scroll_and_fragment() {
    var view = createTemporaryObject(quickScene, tests, { width: 430, height: 530 })
    tryCompare(view, "ready", true)
    var longText = "Select fragment, not everything.\n" + Array(24).fill("Another long message line.").join("\n") + "\nEND OF THE MESSAGE"
    view.serviceModel.history = [{ id: 80, chatId: 101, date: 1790726000, outgoing: false,
      content: { kind: "text", text: longText, entities: [] } }]
    view.consumer.reset(101, 0)
    var text = null
    tryVerify(function () { text = findChild(view, "quick-message-text-80"); return text !== null })
    var list = findChild(view, "quick-message-list")
    wait(50)
    verify(text.parent.height >= text.implicitHeight, "Compact mode must not clip after six lines")
    verify(list.contentHeight > list.height)
    list.positionViewAtBeginning()
    wait(50)
    dragFragment(text, 7, 15)
    var before = list.contentY
    mouseWheel(list, list.width / 2, list.height / 2, 0, -480)
    tryVerify(function () { return list.contentY > before })
    list.positionViewAtEnd()
    wait(50)
    var end = text.positionToRectangle(longText.length - 5)
    var at = text.mapToItem(list, end.x, end.y)
    verify(at.y >= 0 && at.y + end.height <= list.height + 1, "The last line must be reachable inside the viewport")
  }
  function test_actual_service_status_stream_account_and_batch_guards() {
    var service = createTemporaryObject(serviceScene, tests)
    verify(service !== null)
    var transport = null
    for (var child of service.children) if (typeof child.handleLine === "function") transport = child
    verify(transport !== null)
    service.activeAccount = "default"
    service.chats = [{ id: 101, kind: "private", userId: 101, lists: ["main"], order: "1", status: { state: "offline" } }]
    function event(value) { transport.handleLine(JSON.stringify(value)) }
    event({ event: "userStatus", account: "other", userId: 101, status: { state: "online" } })
    compare(service.chats[0].status.state, "offline")
    event({ event: "userStatus", account: "default", userId: 101, status: { state: "online" } })
    compare(service.chats[0].status.state, "online")
    event({ event: "chat", account: "default", chat: Object.assign({}, service.chats[0], { title: "Pending title" }) })
    event({ event: "user", account: "default", user: { id: 101, status: { state: "recently" } } })
    tryCompare(service, "pendingChats", ({}))
    compare(service.chats[0].status.state, "recently", "Pending chat batch must not overwrite the newer peer status")
    compare(service.chats[0].title, "Pending title")
    service.applyAccountSnapshot({ activeAccount: "other", chats: [], auth: { state: "ready" } })
    compare(service.chats.length, 0)
    event({ event: "userStatus", account: "default", userId: 101, status: { state: "online" } })
    compare(service.chats.length, 0)
  }
  function test_peer_status_live_updates_and_privacy() {
    var view = createTemporaryObject(quickScene, tests)
    tryCompare(view, "ready", true)
    var quick = view.consumer
    var service = view.serviceModel
    var label = findChild(view, "quick-chat-peer-status")
    function status(value) {
      service.chats = service.chats.map(function (chat) { return chat.id === 101 ? Object.assign({}, chat, { status: value }) : chat })
    }
    status({ state: "online" })
    tryCompare(label, "text", "online")
    quick.back()
    tryCompare(findChild(view, "quick-peer-status-101"), "text", "online")
    quick.reply(101)
    status({ state: "offline", wasOnline: Math.floor(quick.nowMs / 1000) - 300 })
    tryCompare(label, "text", "last seen 5 minutes ago")
    for (var pair of [["recently", "last seen recently"], ["lastWeek", "last seen within a week"], ["lastMonth", "last seen within a month"], ["", ""]]) {
      status({ state: pair[0] })
      tryCompare(label, "text", pair[1])
    }
    status({ state: "online" })
    quick.reply(900)
    tryCompare(label, "text", "")
    quick.reply(-102)
    tryCompare(label, "text", "")
    service.activeAccount = "other-account"
    tryCompare(quick, "replyChatId", 0)
    compare(label.text, "")
  }
  function test_rich_fragment_data() { return [{ tag: "main", compact: false }, { tag: "compact", compact: true }] }
  function test_rich_fragment(data) {
    var view = createTemporaryObject(richScene, tests, { compact: data.compact,
      richContent: { kind: "rich", full: true, blocks: [{ kind: "text", text: "Select fragment from a rich post", entities: [] }] } })
    tryCompare(view, "ready", true)
    var text = null
    tryVerify(function () { text = findChild(view, "rich-text-0"); return text && text.visible })
    wait(50)
    dragFragment(text, 7, 15)
  }
}
