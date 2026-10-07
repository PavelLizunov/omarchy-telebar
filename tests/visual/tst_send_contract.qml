import QtQuick
import QtTest
import qs.Commons
import "../../app" as App

TestCase {
  id: tests
  name: "MainComposerContract"
  when: windowShown
  visible: true
  width: 700
  height: 650
  FixtureApp { id: model }
  Component { id: chatComponent; App.ChatView { app: model; client: model; chat: model.chats[0]; width: 700; height: 650 } }

  function init() {
    model.sendError = ""
    model.holdSend = false
    model.synchronousSend = false
    model.pendingSend = null
    model.pendingMarkdown = null
    model.activeAccount = "default"
    model.openTopic = null
    model.recordedSend = null
  }

  function prepare(files) {
    var view = createTemporaryObject(chatComponent, tests)
    verify(view !== null)
    view.setComposerText("Synthetic unsent composition")
    view.replyToId = 77
    if (files) view.addAttachments([model.imagePath], false)
    return view
  }

  function test_rejected_send_data() {
    return [{ tag: "text-delayed", files: false, synchronous: false },
            { tag: "files-delayed", files: true, synchronous: false },
            { tag: "text-immediate", files: false, synchronous: true },
            { tag: "files-immediate", files: true, synchronous: true }]
  }
  function test_rejected_send(data) {
    var view = prepare(data.files)
    var input = findChild(view, "composer-text")
    model.sendError = "Synthetic rejection"
    model.synchronousSend = data.synchronous
    view.send()
    tryVerify(function () { return view.notice.indexOf("Synthetic rejection") >= 0 })
    compare(input.text, "Synthetic unsent composition")
    compare(view.replyToId, 77)
    compare(view.attachments.length, data.files ? 1 : 0)
    compare(view.attachAsMedia, data.files ? false : true)
  }

  function test_pending_no_duplicate_and_success_data() { return [{ tag: "text", files: false }, { tag: "files", files: true }] }
  function test_pending_no_duplicate_and_success(data) {
    var view = prepare(data.files)
    var input = findChild(view, "composer-text")
    model.holdSend = true
    view.send()
    var callback = model.pendingSend
    compare(typeof callback, "function")
    compare(input.text, "Synthetic unsent composition")
    view.send()
    compare(model.pendingSend, callback)
    model.completeSend()
    compare(input.text, "")
    compare(view.replyToId, 0)
    compare(view.attachments.length, 0)
    view.setComposerText("A new composition")
    view.send()
    verify(model.pendingSend !== null)
  }

  function test_new_edits_survive_success_data() { return test_pending_no_duplicate_and_success_data() }
  function test_new_edits_survive_success(data) {
    var view = prepare(data.files)
    var input = findChild(view, "composer-text")
    model.holdSend = true
    view.send()
    view.setComposerText("New typing while pending")
    view.replyToId = 88
    if (data.files) view.removeAttachment(0)
    model.completeSend()
    compare(input.text, "New typing while pending")
    compare(view.replyToId, 88)
    compare(view.attachments.length, 0)
  }

  function test_context_changes_ignore_completion_data() {
    return [{ tag: "chat", kind: "chat" }, { tag: "account", kind: "account" }, { tag: "topic", kind: "topic" }]
  }
  function test_context_changes_ignore_completion(data) {
    var view = prepare(false)
    model.holdSend = true
    view.send()
    if (data.kind === "account") { model.activeAccount = "work"; view.resetForChat() }
    else if (data.kind === "chat") view.chat = model.chats[2]
    else {
      view.chat = model.chats[1]
      model.openTopic = model.topicData[0]
    }
    view.setComposerText("New context draft")
    model.completeSend()
    compare(findChild(view, "composer-text").text, "New context draft")
  }

  function test_reply_change_and_edit_back_do_not_clear() {
    var view = prepare(false)
    model.holdSend = true
    view.send()
    view.setComposerText("Temporary edit")
    view.setComposerText("Synthetic unsent composition")
    model.completeSend()
    compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
    view.send()
    view.replyToId = 88
    model.completeSend()
    compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
    compare(view.replyToId, 88)
  }

  function test_failure_releases_admission_and_duplicate_callback_is_stale() {
    var view = prepare(false)
    model.holdSend = true
    view.send()
    var first = model.pendingSend
    model.sendError = "Synthetic rejection"
    model.completeSend()
    verify(!view.sendPending)
    model.sendError = ""
    view.send()
    var second = model.pendingSend
    first({ ok: true, result: {} })
    verify(view.sendPending)
    compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
    compare(model.pendingSend, second)
    model.completeSend()
    compare(findChild(view, "composer-text").text, "")
  }

  function prepareEdit() {
    var view = prepare(false)
    view.startEdit({ id: 88, chatId: model.chats[0].id, outgoing: true,
                     content: { kind: "text", text: "Original message", entities: [] } }, true)
    view.setComposerText("Synthetic unsent edit")
    return view
  }

  function test_rejected_edit_retains_retry_state_data() {
    return [{ tag: "immediate", synchronous: true }, { tag: "delayed", synchronous: false }]
  }
  function test_rejected_edit_retains_retry_state(data) {
    var view = prepareEdit()
    model.sendError = "Synthetic rejection"
    model.synchronousSend = data.synchronous
    view.send()
    tryVerify(function () { return view.notice.indexOf("Synthetic rejection") >= 0 })
    compare(findChild(view, "composer-text").text, "Synthetic unsent edit")
    compare(view.editingId, 88)
    compare(view.draftBeforeEdit, "Synthetic unsent composition")
    verify(!view.sendPending)
    model.sendError = ""
    view.send()
    compare(model.lastRequest.cmd, "message.edit")
    compare(model.lastRequest.args.messageId, 88)
    compare(model.lastRequest.args.text, "Synthetic unsent edit")
    tryCompare(view, "editingId", 0)
    compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
  }

  function test_pending_edit_success_and_duplicate_admission() {
    var view = prepareEdit()
    model.holdSend = true
    view.send()
    var first = model.pendingSend
    compare(typeof first, "function")
    view.send()
    compare(model.pendingSend, first)
    model.completeSend()
    compare(view.editingId, 0)
    compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
    view.setComposerText("New draft")
    first({ ok: true, result: {} })
    compare(findChild(view, "composer-text").text, "New draft")
  }

  function test_new_edit_typing_survives_old_success() {
    var view = prepareEdit()
    model.holdSend = true
    view.send()
    view.setComposerText("New unsent edit")
    model.completeSend()
    compare(view.editingId, 88)
    compare(findChild(view, "composer-text").text, "New unsent edit")
    verify(!view.sendPending)
  }

  function test_cancelled_edit_completion_preserves_draft() {
    var view = prepareEdit()
    model.holdSend = true
    view.send()
    view.finishEdit()
    view.setComposerText("New draft after cancellation")
    model.completeSend()
    compare(view.editingId, 0)
    compare(findChild(view, "composer-text").text, "New draft after cancellation")
  }

  function test_caption_edit_retry_and_empty_success() {
    var view = prepare(false)
    view.startEdit(model.photoMessage, true)
    verify(view.editingCaption)
    model.holdSend = true
    model.sendError = "Synthetic rejection"
    view.setComposerText("Unsent caption")
    view.send()
    compare(model.lastRequest.cmd, "message.edit")
    compare(model.lastRequest.args.caption, true)
    model.completeSend()
    compare(view.editingId, model.photoMessage.id)
    compare(findChild(view, "composer-text").text, "Unsent caption")
    model.sendError = ""
    view.setComposerText("")
    view.send()
    compare(model.lastRequest.args.text, "")
    model.completeSend()
    compare(view.editingId, 0)
    compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
  }

  function test_edit_context_changes_ignore_completion_data() {
    return [{ tag: "chat", kind: "chat" }, { tag: "account", kind: "account" },
            { tag: "topic", kind: "topic" }, { tag: "thread", kind: "thread" }]
  }
  function test_edit_context_changes_ignore_completion(data) {
    var view = prepareEdit()
    model.holdSend = true
    view.send()
    if (data.kind === "account") { model.activeAccount = "work"; view.resetForChat() }
    else if (data.kind === "chat") view.chat = model.chats[2]
    else if (data.kind === "thread") model.openTopic = { id: 55, chatId: view.chat.id, thread: true, name: "Synthetic thread" }
    else { view.chat = model.chats[1]; model.openTopic = model.topicData[0] }
    view.setComposerText("New context draft")
    var currentId = view.editingId
    model.completeSend()
    compare(findChild(view, "composer-text").text, "New context draft")
    compare(view.editingId, currentId)
  }

  function test_scheduled_edit_success_restores_draft() {
    var view = prepareEdit()
    view.scheduledOpen = true
    model.holdSend = true
    view.send()
    model.completeSend()
    compare(view.editingId, 0)
    compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
    compare(model.lastRequest.cmd, "chat.scheduled")
  }

  function test_scheduled_edit_failure_and_typing_drift() {
    var view = prepareEdit()
    view.scheduledOpen = true
    model.holdSend = true
    model.sendError = "Synthetic schedule rejection"
    view.send()
    model.completeSend()
    compare(view.editingId, 88)
    compare(view.draftBeforeEdit, "Synthetic unsent composition")
    compare(findChild(view, "composer-text").text, "Synthetic unsent edit")
    verify(view.notice.indexOf("Synthetic schedule rejection") >= 0)
    compare(model.lastRequest.cmd, "message.edit")
    model.sendError = ""
    view.send()
    view.setComposerText("New scheduled edit typing")
    model.completeSend()
    compare(view.editingId, 88)
    compare(findChild(view, "composer-text").text, "New scheduled edit typing")
    compare(model.lastRequest.cmd, "chat.scheduled")
  }

  function test_edit_markdown_only_updates_unchanged_context_data() {
    return [{ tag: "current", kind: "current" }, { tag: "edit-revert", kind: "edit-revert" },
            { tag: "chat", kind: "chat" }, { tag: "account", kind: "account" },
            { tag: "topic", kind: "topic" }, { tag: "thread", kind: "thread" }]
  }
  function test_edit_markdown_only_updates_unchanged_context(data) {
    var view = prepare(false)
    var message = { id: 88, chatId: view.chat.id, outgoing: true,
      content: { kind: "text", text: "Same plain text", entities: [{ type: "bold", offset: 0, length: 4 }] } }
    view.startEdit(message, true)
    var old = model.pendingMarkdown
    compare(typeof old, "function")
    if (data.kind === "edit-revert") {
      view.setComposerText("New typing")
      view.setComposerText("Same plain text")
    } else if (data.kind !== "current") {
      if (data.kind === "chat") view.chat = model.chats[2]
      else if (data.kind === "account") { model.activeAccount = "work"; view.resetForChat() }
      else if (data.kind === "topic") { view.chat = model.chats[1]; model.openTopic = model.topicData[0] }
      else model.openTopic = { id: 55, chatId: view.chat.id, thread: true, name: "Synthetic thread" }
      message.chatId = view.chat.id
      view.startEdit(message, true)
    }
    old({ ok: true, result: { text: "**Same plain text**" } })
    compare(findChild(view, "composer-text").text,
            data.kind === "current" ? "**Same plain text**" : "Same plain text")
    compare(view.editingId, 88)
  }

  function test_extra_rejection_keeps_reply_data() {
    var cases = [{ command: "message.sendDice", args: { emoji: "🎲" } },
                 { command: "message.sendPoll", args: { question: "Synthetic?", options: ["Yes", "No"] } },
                 { command: "message.sendContact", args: { userId: 42 } },
                 { command: "message.sendLocation", args: { latitude: 10, longitude: 20 } }]
    var result = []
    for (var entry of cases) for (var immediate of [true, false])
      result.push({ tag: entry.command + (immediate ? "-immediate" : "-delayed"),
                    command: entry.command, args: entry.args, immediate: immediate })
    return result
  }
  function test_extra_rejection_keeps_reply(data) {
    var view = prepare(false)
    model.synchronousSend = data.immediate
    model.sendError = "Synthetic rejection"
    view.sendExtra(data.command, data.args, "synthetic content")
    tryCompare(view, "sendPending", false)
    compare(view.notice, "Could not send synthetic content: Synthetic rejection")
    compare(view.replyToId, 77)
    compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
    compare(model.lastRequest.args.replyToMessageId, 77)
    verify(!view.sendPending)
  }
  function test_extra_preserves_staged_attachments_data() {
    return [{ tag: "rejected", error: "Synthetic rejection" }, { tag: "accepted", error: "" }]
  }
  function test_extra_preserves_staged_attachments(data) {
    var view = prepare(true)
    model.synchronousSend = true
    model.sendError = data.error
    var path = view.attachments[0].path
    view.sendExtra("message.sendDice", { emoji: "🎲" }, "the dice")
    compare(view.attachments.length, 1)
    compare(view.attachments[0].path, path)
    compare(view.attachAsMedia, false)
    compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
    compare(view.replyToId, data.error ? 77 : 0)
  }
  function test_extra_rejected_during_edit_preserves_edit_and_operation_label() {
    var view = prepareEdit()
    model.synchronousSend = true
    model.sendError = "Synthetic rejection"
    view.sendExtra("message.sendDice", { emoji: "🎲" }, "the dice")
    compare(view.notice, "Could not send the dice: Synthetic rejection")
    compare(view.editingId, 88)
    compare(view.draftBeforeEdit, "Synthetic unsent composition")
    compare(findChild(view, "composer-text").text, "Synthetic unsent edit")
  }
  function test_extra_stale_failure_and_duplicate_callback_cannot_replace_notice() {
    var view = prepare(false)
    model.holdSend = true
    view.sendExtra("message.sendDice", { emoji: "🎲" }, "the dice")
    var callback = model.pendingSend
    view.chat = model.chats[2]
    view.notice = "Current chat notice"
    callback({ ok: false, error: "Old rejection", result: {} })
    compare(view.notice, "Current chat notice")
    verify(!view.sendPending)
    view.replyToId = 99
    view.sendExtra("message.sendDice", { emoji: "🎲" }, "the dice")
    callback({ ok: true, result: {} })
    verify(view.sendPending)
    compare(view.replyToId, 99)
    model.completeSend()
    compare(view.replyToId, 0)
  }
  function test_extra_success_owns_reply_only_data() {
    return [{ tag: "current", kind: "current" }, { tag: "typing", kind: "typing" },
            { tag: "reply", kind: "reply" }, { tag: "chat", kind: "chat" },
            { tag: "account", kind: "account" }, { tag: "topic", kind: "topic" },
            { tag: "thread", kind: "thread" }, { tag: "editing", kind: "editing" }]
  }
  function test_extra_success_owns_reply_only(data) {
    var view = data.kind === "editing" ? prepareEdit() : prepare(false)
    var reply = view.replyToId
    var text = findChild(view, "composer-text").text
    model.holdSend = true
    view.sendExtra("message.sendDice", { emoji: "🎲" }, "the dice")
    var callback = model.pendingSend
    compare(view.replyToId, reply)
    view.sendExtra("message.sendDice", { emoji: "🎲" }, "the dice")
    compare(model.pendingSend, callback)
    if (data.kind === "typing") view.setComposerText("New typing")
    else if (data.kind === "reply") view.replyToId = 99
    else if (data.kind === "chat") { view.chat = model.chats[2]; view.replyToId = 99 }
    else if (data.kind === "account") { view.leaveAccount(); model.activeAccount = "work"; view.replyToId = 99 }
    else if (data.kind === "topic") { view.chat = model.chats[1]; model.openTopic = model.topicData[0]; view.replyToId = 99 }
    else if (data.kind === "thread") { model.openTopic = { id: 55, chatId: view.chat.id, thread: true, name: "Synthetic thread" }; view.replyToId = 99 }
    model.completeSend()
    compare(view.replyToId, data.kind === "current" || data.kind === "editing" ? 0 : data.kind === "typing" ? reply : 99)
    if (data.kind === "current" || data.kind === "editing") compare(findChild(view, "composer-text").text, text)
    if (data.kind === "editing") compare(view.editingId, 88)
    verify(!view.sendPending)
    view.replyToId = 101
    callback({ ok: true, result: {} })
    compare(view.replyToId, 101)
  }
  function test_voice_discard_does_not_consume_pending_composer_send() {
    var view = prepare(false)
    model.holdSend = true
    view.send()
    var original = model.pendingSend
    var serial = view.sendSerial
    view.stopVoice(false)
    compare(model.lastRequest.cmd, "voice.stop")
    compare(model.lastRequest.args.send, false)
    compare(view.sendSerial, serial)
    verify(view.sendPending)
    var discard = model.pendingSend
    discard({ ok: true, result: {} })
    verify(view.sendPending)
    compare(view.replyToId, 77)
    original({ ok: true, result: {} })
    verify(!view.sendPending)
    compare(findChild(view, "composer-text").text, "")
  }
  function test_voice_stop_rejection_during_edit_names_send_and_preserves_edit() {
    var view = prepareEdit()
    model.synchronousSend = true
    model.sendError = "Synthetic capacity rejection"
    view.stopVoice(true)
    compare(view.notice, "Could not send: Synthetic capacity rejection")
    compare(view.editingId, 88)
    compare(findChild(view, "composer-text").text, "Synthetic unsent edit")
    compare(view.draftBeforeEdit, "Synthetic unsent composition")
    verify(!view.sendPending)
  }
  function test_voice_stop_rejection_keeps_reply() {
    var view = prepare(false)
    model.synchronousSend = true
    model.sendError = "Synthetic capacity rejection"
    view.stopVoice(true)
    compare(view.replyToId, 77)
    compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
    verify(view.notice.indexOf("Synthetic capacity rejection") >= 0)
  }
  function test_voice_stop_success_clears_only_unchanged_reply_data() {
    return [{ tag: "current", kind: "current" }, { tag: "typing", kind: "typing" },
            { tag: "reply", kind: "reply" }, { tag: "chat", kind: "chat" },
            { tag: "account", kind: "account" }, { tag: "topic", kind: "topic" },
            { tag: "thread", kind: "thread" }]
  }
  function test_voice_stop_success_clears_only_unchanged_reply(data) {
    var view = prepare(false)
    model.holdSend = true
    view.stopVoice(true)
    var callback = model.pendingSend
    compare(view.replyToId, 77)
    view.stopVoice(true)
    compare(model.pendingSend, callback)
    if (data.kind === "typing") view.setComposerText("New typing during voice send")
    else if (data.kind === "reply") view.replyToId = 88
    else if (data.kind === "chat") { view.chat = model.chats[2]; view.replyToId = 88 }
    else if (data.kind === "account") { view.leaveAccount(); model.activeAccount = "work"; view.replyToId = 88 }
    else if (data.kind === "topic") { view.chat = model.chats[1]; model.openTopic = model.topicData[0]; view.replyToId = 88 }
    else if (data.kind === "thread") { model.openTopic = { id: 55, chatId: view.chat.id, thread: true, name: "Synthetic thread" }; view.replyToId = 88 }
    model.completeSend()
    compare(view.replyToId, data.kind === "current" ? 0 : (data.kind === "typing" ? 77 : 88))
    if (data.kind === "current") compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
    verify(!view.sendPending)
    view.replyToId = 99
    callback({ ok: true, result: {} })
    compare(view.replyToId, 99)
  }
  function pickerSend(view, kind) {
    if (kind === "sticker") findChild(view, "chat-sticker-picker").picked({ file: { id: 456 }, width: 128, height: 128, emoji: "" })
    else if (kind === "saved-gif") view.sendGif({ gif: { file: { id: 789 }, width: 320, height: 240, duration: 2 } })
    else view.sendGif({ gif: {}, queryId: "12345", resultId: "synthetic-result" })
  }
  function test_picker_rejection_retains_composition_data() {
    var result = []
    for (var kind of ["sticker", "saved-gif", "inline-gif"]) for (var immediate of [true, false])
      result.push({ tag: kind + (immediate ? "-immediate" : "-delayed"), kind: kind, immediate: immediate })
    return result
  }
  function test_picker_rejection_retains_composition(data) {
    var view = prepare(true)
    view.stickersOpen = true
    model.sendError = "Synthetic rejection"
    model.synchronousSend = data.immediate
    pickerSend(view, data.kind)
    tryVerify(function () { return view.notice.indexOf("Synthetic rejection") >= 0 })
    compare(view.replyToId, 77)
    compare(view.attachments.length, 1)
    compare(view.attachAsMedia, false)
    compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
    verify(view.stickersOpen)
    verify(!view.sendPending)
    compare(model.lastRequest.args.replyToMessageId, 77)
  }
  function test_picker_success_owns_reply_only_data() {
    var result = []
    for (var kind of ["sticker", "saved-gif", "inline-gif"])
      for (var change of ["current", "typing", "reply", "chat", "account", "topic", "thread", "editing"])
        result.push({ tag: kind + "-" + change, kind: kind, change: change })
    return result
  }
  function test_picker_success_owns_reply_only(data) {
    var view = data.change === "editing" ? prepareEdit() : prepare(true)
    model.holdSend = true
    var text = findChild(view, "composer-text").text
    var reply = view.replyToId
    pickerSend(view, data.kind)
    var callback = model.pendingSend
    compare(typeof callback, "function")
    compare(view.replyToId, reply)
    verify(view.sendPending)
    pickerSend(view, data.kind)
    compare(model.pendingSend, callback)
    if (data.change === "typing") view.setComposerText("New typing")
    else if (data.change === "reply") view.replyToId = 99
    else if (data.change === "chat") { view.chat = model.chats[2]; view.replyToId = 99 }
    else if (data.change === "account") { view.leaveAccount(); model.activeAccount = "work"; view.replyToId = 99 }
    else if (data.change === "topic") { view.chat = model.chats[1]; model.openTopic = model.topicData[0]; view.replyToId = 99 }
    else if (data.change === "thread") { model.openTopic = { id: 55, chatId: view.chat.id, thread: true, name: "Synthetic thread" }; view.replyToId = 99 }
    model.completeSend()
    compare(view.replyToId, data.change === "current" || data.change === "editing" ? 0 : data.change === "typing" ? reply : 99)
    if (data.change === "current" || data.change === "editing") compare(findChild(view, "composer-text").text, text)
    if (data.change === "current") { compare(view.attachments.length, 1); compare(view.attachAsMedia, false) }
    if (data.change === "editing") compare(view.editingId, 88)
    verify(!view.sendPending)
    view.replyToId = 101
    callback({ ok: true, result: {} })
    compare(view.replyToId, 101)
  }
  function test_picker_stale_rejection_ignores_notice_data() {
    return [{ tag: "sticker", kind: "sticker" }, { tag: "saved-gif", kind: "saved-gif" }, { tag: "inline-gif", kind: "inline-gif" }]
  }
  function test_picker_stale_rejection_ignores_notice(data) {
    var view = prepare(false)
    model.holdSend = true
    pickerSend(view, data.kind)
    var callback = model.pendingSend
    view.chat = model.chats[2]
    view.notice = "Current chat notice"
    callback({ ok: false, error: "Old rejection", result: {} })
    compare(view.notice, "Current chat notice")
    view.replyToId = 99
    pickerSend(view, data.kind)
    callback({ ok: false, error: "Duplicate rejection", result: {} })
    verify(view.sendPending)
    compare(view.notice, "Current chat notice")
  }
  function test_picker_topic_and_thread_target_data() {
    var result = []
    for (var kind of ["sticker", "saved-gif", "inline-gif"]) for (var thread of [false, true])
      result.push({ tag: kind + (thread ? "-thread" : "-topic"), kind: kind, thread: thread })
    return result
  }
  function test_picker_topic_and_thread_target(data) {
    var view = prepare(false)
    view.chat = model.chats[1]
    model.openTopic = { id: 55, chatId: view.chat.id, thread: data.thread, name: "Synthetic topic" }
    view.replyToId = 77
    model.holdSend = true
    pickerSend(view, data.kind)
    compare(model.lastRequest.args[data.thread ? "threadId" : "topicId"], 55)
    compare(model.lastRequest.args[data.thread ? "topicId" : "threadId"], undefined)
    compare(model.lastRequest.args.chatId, view.chat.id)
  }
  function inertVideo(view) {
    var recorder = findChild(view, "chat-video-recorder")
    verify(recorder !== null)
    // Keep the real recorder's CaptureSession Loader inactive even while testing open/stop state.
    recorder.visible = false
    return recorder
  }
  function openInertVideo(view, recorder) {
    var context = { chatId: view.chat.id, topicId: view.topicId, thread: view.threadOpen,
                    account: model.activeAccount, replyToId: view.replyToId }
    if (typeof view.openVideo === "function") view.openVideo()
    else recorder.open(view.chat.id)
    return recorder.sendContext || context
  }
  function emitVideo(recorder, context, path) {
    if (recorder.sendContext === undefined) recorder.recorded(context.chatId, path)
    else recorder.recorded(context.chatId, path, context)
  }
  function test_video_original_context_and_reply_data() {
    var result = []
    for (var change of ["current", "typing", "reply", "chat", "topic", "thread"])
      for (var failure of [false, true]) result.push({ tag: change + (failure ? "-rejected" : "-accepted"), change: change, failure: failure })
    return result
  }
  function test_video_original_context_and_reply(data) {
    var view = prepare(false)
    view.chat = model.chats[1]
    model.openTopic = model.topicData[1]
    view.replyToId = 77
    view.setComposerText("Original caption draft")
    var recorder = inertVideo(view)
    var context = openInertVideo(view, recorder)
    compare(recorder.phase, "preview")
    compare(recorder.mediaRecorder, null)
    compare(context.topicId, 2)
    compare(context.replyToId, 77)
    compare(context.account, "default")
    if (data.change === "typing") view.setComposerText("New typing")
    else if (data.change === "reply") view.replyToId = 99
    else if (data.change === "chat") { view.chat = model.chats[2]; view.replyToId = 99 }
    else if (data.change === "topic") { model.openTopic = model.topicData[2]; view.replyToId = 99 }
    else if (data.change === "thread") { model.openTopic = { id: 2, chatId: view.chat.id, thread: true, name: "Synthetic thread" }; view.replyToId = 99 }
    model.holdSend = true
    model.sendError = data.failure ? "Synthetic rejection" : ""
    recorder.phase = "closed"
    emitVideo(recorder, context, "/synthetic/recordings/video.mp4")
    compare(model.lastRequest.cmd, "videonote.send")
    compare(model.lastRequest.args.chatId, model.chats[1].id)
    compare(model.lastRequest.args.topicId, 2)
    compare(model.lastRequest.args.threadId, undefined)
    compare(model.lastRequest.args.replyToMessageId, 77)
    var callback = model.pendingSend
    model.completeSend()
    compare(view.replyToId, data.change === "current" ? (data.failure ? 77 : 0) : data.change === "typing" ? 77 : 99)
    if (data.change === "current") compare(findChild(view, "composer-text").text, "Original caption draft")
    if (data.change === "typing") compare(findChild(view, "composer-text").text, "New typing")
    if (data.failure && (data.change === "current" || data.change === "typing" || data.change === "reply"))
      compare(view.notice, "Could not send the video message: Synthetic rejection")
    verify(!view.sendPending)
    view.replyToId = 101
    callback({ ok: true, result: {} })
    compare(view.replyToId, 101)
  }
  function test_video_old_completion_cannot_retire_new_send_data() {
    return [{ tag: "accepted", failure: false }, { tag: "rejected", failure: true }]
  }
  function test_video_old_completion_cannot_retire_new_send(data) {
    var view = prepare(false)
    var recorder = inertVideo(view)
    var context = openInertVideo(view, recorder)
    recorder.phase = "closed"
    view.chat = model.chats[2]
    view.replyToId = 99
    model.holdSend = true
    view.setComposerText("New chat draft")
    view.send()
    var current = model.pendingSend
    var serial = view.sendSerial
    emitVideo(recorder, context, "/synthetic/recordings/old.mp4")
    var old = model.pendingSend
    old({ ok: !data.failure, error: "Old rejection", result: {} })
    compare(view.sendSerial, serial)
    verify(view.sendPending)
    compare(view.replyToId, 99)
    compare(findChild(view, "composer-text").text, "New chat draft")
    current({ ok: true, result: {} })
    verify(!view.sendPending)
    compare(view.replyToId, 0)
    compare(findChild(view, "composer-text").text, "")
  }
  function test_video_account_change_discards_instead_of_rerouting() {
    var view = prepare(false)
    var recorder = inertVideo(view)
    var context = openInertVideo(view, recorder)
    recorder.phase = "closed"
    model.activeAccount = "work"
    emitVideo(recorder, context, "/synthetic/recordings/old.mp4")
    compare(model.lastRequest.cmd, "videonote.discard")
    compare(model.pendingSend, null)
    verify(!view.sendPending)
  }
  function test_video_cancel_while_stopping_revokes_send() {
    var view = prepare(false)
    var recorder = inertVideo(view)
    openInertVideo(view, recorder)
    recorder.phase = "stopping"
    recorder.sendWhenStopped = true
    view.leaveAccount()
    compare(recorder.sendWhenStopped, false)
    compare(recorder.mediaRecorder, null)
    recorder.phase = "closed"
  }
  function test_video_visible_overlay_blocks_other_composer_sends() {
    var view = prepare(true)
    var recorder = findChild(view, "chat-video-recorder")
    var capture = findChild(recorder, "video-capture-session")
    verify(capture !== null)
    capture.active = false
    view.openVideo()
    compare(recorder.phase, "preview")
    verify(recorder.visible)
    compare(recorder.mediaRecorder, null)
    model.holdSend = true
    view.send()
    view.sendAttachments()
    pickerSend(view, "sticker")
    pickerSend(view, "saved-gif")
    view.stopVoice(true)
    compare(model.pendingSend, null)
    compare(view.replyToId, 77)
    compare(view.attachments.length, 1)
    recorder.finish(false)
  }
  function test_video_no_second_open_during_pending_send() {
    var view = prepare(false)
    var recorder = inertVideo(view)
    model.holdSend = true
    view.send()
    openInertVideo(view, recorder)
    compare(recorder.phase, "closed")
    compare(recorder.sendContext, null)
  }
  function test_retained_recording_explicit_retry_and_discard_data() {
    return [{ tag: "preparation-retry", state: "prepareFailed", retry: true },
            { tag: "rejected-retry", state: "rejected", retry: true },
            { tag: "rejected-discard", state: "rejected", retry: false },
            { tag: "unknown-dismiss", state: "unknown", retry: false }]
  }
  function test_retained_recording_explicit_retry_and_discard(data) {
    var view = prepare(false)
    model.recordedSend = { token: "synthetic-token", state: data.state, kind: "voice", chatId: view.chat.id, account: "default" }
    var bar = findChild(view, "recorded-send-bar")
    verify(bar.visible)
    model.holdSend = true
    mouseClick(findChild(bar, data.retry ? "recorded-send-retry" : "recorded-send-discard"))
    compare(model.lastRequest.cmd, data.retry ? "recording.retry" : "recording.discard")
    compare(model.lastRequest.args.token, "synthetic-token")
    compare(model.lastRequest.args.account, "default")
    verify(bar.busy)
    var callback = model.pendingSend
    bar.act(data.retry)
    compare(model.pendingSend, callback)
    model.sendError = "Synthetic retry rejected"
    model.completeSend()
    verify(!bar.busy)
    compare(bar.error, "Synthetic retry rejected")
    compare(view.replyToId, 77)
    compare(findChild(view, "composer-text").text, "Synthetic unsent composition")
  }
  function test_retained_recording_keyboard_retry_and_discard() {
    var view = prepare(false)
    model.recordedSend = { token: "keyboard-token", state: "rejected", kind: "voice", chatId: view.chat.id, account: "default" }
    var bar = findChild(view, "recorded-send-bar")
    var retry = findChild(bar, "recorded-send-retry")
    model.holdSend = true
    retry.forceActiveFocus()
    verify(retry.activeFocus)
    keyClick(Qt.Key_Return)
    compare(model.lastRequest.cmd, "recording.retry")
    model.sendError = "Synthetic retry rejected"
    model.completeSend()
    var discard = findChild(bar, "recorded-send-discard")
    discard.forceActiveFocus()
    verify(discard.activeFocus)
    keyClick(Qt.Key_Space)
    compare(model.lastRequest.cmd, "recording.discard")
    model.sendError = ""
    model.completeSend()
    verify(!retry.enabled)
    verify(!discard.enabled)
  }
  function test_retained_recording_unknown_and_stale_callbacks_fail_closed() {
    var view = prepare(false)
    model.recordedSend = { token: "old-token", state: "unknown", kind: "video", chatId: view.chat.id, account: "default" }
    var bar = findChild(view, "recorded-send-bar")
    verify(!findChild(bar, "recorded-send-retry").enabled)
    bar.act(true)
    compare(model.pendingSend, null)
    model.recordedSend = { token: "old-token", state: "rejected", kind: "video", chatId: view.chat.id, account: "default" }
    model.holdSend = true
    bar.act(true)
    var old = model.pendingSend
    model.recordedSend = { token: "new-token", state: "rejected", kind: "voice", chatId: view.chat.id, account: "work" }
    bar.act(true)
    verify(bar.busy)
    old({ ok: false, error: "Old error" })
    verify(bar.busy)
    compare(bar.error, "")
    model.completeSend()
    verify(!bar.busy)
  }
  function test_save_request_binds_original_message() {
    var view = prepare(false)
    view.saveFile(model.photoMessage)
    compare(model.lastRequest.cmd, "file.save")
    compare(model.lastRequest.args.chatId, model.photoMessage.chatId)
    compare(model.lastRequest.args.messageId, model.photoMessage.id)
    compare(model.lastRequest.args.fileId, model.photoMessage.content.media.file.id)
  }
}
