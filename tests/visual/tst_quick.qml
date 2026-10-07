import QtQuick
import QtTest
import qs.Commons
import "../../app/Model.js" as Model

TestCase {
  id: tests
  name: "QuickReplySendAndReceipts"
  when: windowShown
  visible: true
  width: 650
  height: 700
  Component { id: scene; QuickConsumer {} }
  SignalSpy { id: dismissSpy; signalName: "dismissRequested" }
  function init() { failOnWarning(/DelegateModel::cancel/) }
  function cleanup() { dismissSpy.target = null; dismissSpy.clear(); Style.fontBaseSize = 12; Style.cornerRadius = 0 }
  function test_enter_keeps_open_and_failure_retains_draft() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    var quick = view.consumer
    var model = view.serviceModel
    var input = findChild(view, "quick-composer")
    dismissSpy.target = quick
    input.text = "First reply"
    input.forceActiveFocus()
    keyClick(Qt.Key_Return)
    tryCompare(input, "text", "")
    compare(dismissSpy.count, 0)
    model.recording = { state: "voice", chatId: 101, startedAt: model.nowMs }
    quick.stopRecording(true)
    tryCompare(quick, "sending", false)
    compare(dismissSpy.count, 0)
    model.recording = { state: "idle" }
    compare(quick.replyChatId, 101)
    model.sendError = "Synthetic send error"
    input.text = "Keep my draft"
    keyClick(Qt.Key_Return)
    tryCompare(quick, "status", "Synthetic send error")
    compare(input.text, "Keep my draft")
    compare(dismissSpy.count, 0)
    model.sendError = ""
    quick.addAttachments([model.imagePath], true)
    keyClick(Qt.Key_Return)
    tryVerify(function () { return quick.attachments.length === 0 })
    compare(input.text, "")
    compare(dismissSpy.count, 0)
  }
  function test_retained_recording_pointer_retry_and_unknown_guard() {
    var view = createTemporaryObject(scene, tests, { state: "retained-rejected" })
    tryCompare(view, "ready", true)
    var model = view.serviceModel
    var bar = findChild(view, "recorded-send-bar")
    verify(bar.visible)
    model.holdSend = true
    mouseClick(findChild(bar, "recorded-send-retry"))
    compare(model.lastRequest.cmd, "recording.retry")
    compare(model.lastRequest.args.account, "default")
    verify(bar.busy)
    model.completeSend()
    verify(!bar.busy)
    verify(!findChild(bar, "recorded-send-retry").enabled)
    model.recordedSend = { token: "unknown-token", state: "unknown", chatId: 101, account: "default", kind: "video" }
    verify(!findChild(bar, "recorded-send-retry").enabled)
    mouseClick(findChild(bar, "recorded-send-discard"))
    compare(model.lastRequest.cmd, "recording.discard")
  }
  function test_receipts_update_from_chat_read_cursor() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    tryVerify(function () { return findChild(view, "quick-receipt-20") !== null })
    compare(findChild(view, "quick-receipt-1").text, "✓✓")
    compare(findChild(view, "quick-receipt-20").text, "✓")
    compare(findChild(view, "quick-receipt-21").text, "Sending…")
    compare(findChild(view, "quick-receipt-22").text, "Failed")
    compare(findChild(view, "quick-receipt-23").visible, false)
    var chats = view.serviceModel.chats.slice()
    var chat = Object.assign({}, chats[0], { lastReadOutbox: 20 })
    chats[0] = chat
    view.serviceModel.chats = chats
    tryCompare(findChild(view, "quick-receipt-20"), "text", "✓✓")
  }
  function test_viewed_forum_messages_are_read_data() {
    return [{ tag: "compact", compact: true }, { tag: "wide", compact: false }]
  }
  function test_viewed_forum_messages_are_read(data) {
    var view = createTemporaryObject(scene, tests, { compactMode: data.compact })
    tryCompare(view, "ready", true)
    var quick = view.consumer
    var model = view.serviceModel
    model.readLog = []
    model.history = [
      { id: 90, chatId: -102, topicId: 2, outgoing: false, date: 1790726300, content: { kind: "text", text: "Read this topic", entities: [] } },
      { id: 91, chatId: -102, topicId: 3, outgoing: false, date: 1790726310, content: { kind: "text", text: "And this topic", entities: [] } },
      { id: 92, chatId: -102, topicId: 2, outgoing: true, date: 1790726320, content: { kind: "text", text: "My own message", entities: [] } }
    ]
    quick.reply(-102)
    tryCompare(quick, "historyChatId", -102)
    tryVerify(function () { return model.readLog.filter(function (r) { return r.chatId === -102 }).length === 2 })
    var read = model.readLog.filter(function (r) { return r.chatId === -102 })
    compare(read[0].topicId, 2)
    compare(read[0].messageIds, [90])
    compare(read[1].topicId, 3)
    compare(read[1].messageIds, [91])
    model.messageEvent("message", { message: { id: 93, chatId: -102, topicId: 2, outgoing: false, date: 1790726330,
      content: { kind: "text", text: "A new incoming message", entities: [] } } })
    tryVerify(function () { return model.readLog.some(function (r) { return r.topicId === 2 && r.messageIds.indexOf(93) >= 0 }) })
    quick.opened = false
    quick.leave()
    var count = model.readLog.length
    wait(200)
    compare(model.readLog.length, count, "Closing must cancel pending read work")
  }
  function test_offscreen_topic_is_not_read_and_reads_do_not_repeat() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    var quick = view.consumer
    var model = view.serviceModel
    model.readLog = []
    model.history = Array.from({ length: 20 }, function (_, i) {
      return { id: 100 + i, chatId: -102, topicId: i === 0 ? 2 : 3, outgoing: false, date: 1790726300 + i,
        content: { kind: "text", text: "Synthetic forum message " + i, entities: [] } }
    })
    quick.reply(-102)
    tryVerify(function () { return model.readLog.some(function (r) { return r.topicId === 3 }) })
    verify(!model.readLog.some(function (r) { return r.topicId === 2 }), "A topic outside the viewport must stay unread")
    var count = model.readLog.length
    quick.markViewedRead()
    quick.markRead(model.chats[1])
    wait(200)
    compare(model.readLog.length, count, "Settled messages must not cause repeated requests")
  }
  function test_late_send_response_does_not_clear_another_chat() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    var quick = view.consumer
    var model = view.serviceModel
    var input = findChild(view, "quick-composer")
    model.holdSend = true
    input.text = "Old chat"
    quick.send()
    quick.reply(900)
    input.text = "New chat draft"
    model.completeSend()
    compare(input.text, "New chat draft")
    compare(quick.replyChatId, 900)
  }
  function test_long_draft_and_narrow_input() {
    var view = createTemporaryObject(scene, tests, { width: 350, height: 500 })
    tryCompare(view, "ready", true)
    var input = findChild(view, "quick-composer")
    input.text = Array(20).fill("Long draft line").join("\n")
    input.cursorPosition = input.text.length
    wait(30)
    verify(input.width > 150)
    verify(input.parent.parent.contentY > 0)
    compare(input.text.split("\n").length, 20)
  }
  function test_tools_fit_editor_across_width_and_scale() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    var box = findChild(view, "quick-composer-box")
    var input = findChild(view, "quick-composer")
    for (var size of [12, 14, 15, 18]) {
      Style.fontBaseSize = size
      for (var width of [350, 534, 700, 350, 534]) {
        view.width = width
        for (var draft of ["", "A draft\nwith two lines"]) {
          input.text = draft
          wait(30)
          var viewport = input.parent.parent
          var viewportAt = viewport.mapToItem(box, 0, 0)
          for (var action of ["attach", "stickers", "voice", "video"]) {
            var tool = findChild(view, "quick-tool-" + action)
            var pos = tool.mapToItem(box, 0, 0)
            verify(pos.y >= 2 && pos.y + tool.height <= box.height - 2,
              action + " escapes editor: y=" + pos.y + " height=" + tool.height + " box=" + box.height + " width=" + width + " font=" + size)
            verify(pos.x >= 2 && pos.x + tool.width <= box.width - 2)
            verify(viewportAt.x + viewport.width <= pos.x || viewportAt.y + viewport.height <= pos.y,
              "Tool hit area must not overlap the text viewport")
          }
        }
      }
    }
  }
  function test_receipt_contrast_light_and_dark() {
    for (var light of [false, true]) {
      var view = createTemporaryObject(scene, tests, { lightTheme: light })
      tryCompare(view, "ready", true)
      var quick = view.consumer
      for (var id of [1, 20, 21, 22]) {
        var label = findChild(view, "quick-receipt-" + id)
        verify(label !== null)
        var ground = Model.mixColors(quick.background, quick.text, 0.12)
        verify(Model.colorContrast(label.color, ground) >= 4.5)
      }
    }
  }
  function test_attach_button_selection_cancel_and_send() {
    var view = createTemporaryObject(scene, tests, { width: 350, height: 500 })
    tryCompare(view, "ready", true)
    var quick = view.consumer
    var picker = findChild(view, "quick-file-picker")
    var button = findChild(view, "quick-tool-attach")
    verify(button !== null)
    mouseClick(button, button.width / 2, button.height / 2)
    tryCompare(picker, "visible", true)
    wait(30)
    var card = findChild(picker, "file-picker-card")
    for (var name of ["file-picker-path", "file-picker-list", "file-picker-accept", "file-picker-cancel"]) {
      var item = findChild(picker, name)
      var pos = item.mapToItem(card, 0, 0)
      verify(pos.x >= 0 && pos.x + item.width <= card.width + 1, name + " must fit inside the picker")
    }
    verify(String(picker.currentFolder).startsWith("file:///"))
    keyClick(Qt.Key_Escape)
    tryCompare(picker, "visible", false)
    compare(quick.attachments.length, 0)
    button.forceActiveFocus()
    keyClick(Qt.Key_Return)
    tryCompare(picker, "visible", true)
    picker.navigate(Qt.resolvedUrl("dialog-files/"))
    var list = findChild(picker, "file-picker-list")
    tryCompare(list, "count", 2)
    picker.toggle(0)
    picker.toggle(1)
    picker.accept()
    compare(quick.attachments.length, 2)
    var input = findChild(view, "quick-composer")
    input.text = "Two files"
    input.forceActiveFocus()
    view.serviceModel.holdSend = true
    keyClick(Qt.Key_Return)
    compare(view.serviceModel.lastRequest.cmd, "message.sendFiles")
    compare(view.serviceModel.lastRequest.args.paths.length, 2)
    compare(view.serviceModel.lastRequest.args.caption, "Two files")
    view.serviceModel.completeSend()
    compare(quick.attachments.length, 0)
    compare(quick.replyChatId, 101)
    verify(input.width > 250)
  }
}
