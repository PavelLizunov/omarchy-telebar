import QtQuick
import QtTest
import "../../app" as App

TestCase {
  id: tests
  name: "MentionContextContract"
  when: windowShown
  visible: true
  width: 700
  height: 650
  FixtureApp { id: model }
  property var callbacks: []
  property var reads: []
  QtObject {
    id: inertClient
    function request(cmd, args, callback) {
      if (cmd === "chat.nextMention" || cmd === "chat.nextReaction") tests.callbacks = tests.callbacks.concat([callback])
      else if (cmd === "chat.read" || cmd === "chat.readMentions" || cmd === "chat.readReactions") tests.reads = tests.reads.concat([{ cmd: cmd, args: args }])
      else model.request(cmd, args, callback)
    }
  }
  Component { id: component; App.ChatView { app: model; client: inertClient; width: 700; height: 650 } }
  function init() { model.activeAccount = "default"; model.openTopic = null; callbacks = []; reads = []; model.readLog = [] }
  function prepare() {
    model.openTopic = model.topicData[0]
    var view = createTemporaryObject(component, tests, { chat: model.chats[1] })
    verify(view !== null)
    return view
  }
  function message(id) {
    return { id: id, chatId: -102, date: 1790726000, outgoing: false, senderName: "Synthetic sender",
      sender: { type: "user", id: 101 }, content: { kind: "text", text: "Synthetic mention", entities: [] }, reactions: [] }
  }
  function test_loaded_result_invalidated_before_new_history() {
    var view = prepare()
    view.nextMention()
    callbacks[0]({ ok: true, result: { messageId: 77 } })
    compare(view.pendingMention, 77)
    var serial = view.mentionSerial
    model.openTopic = model.topicData[1]
    compare(view.pendingMention, 0)
    verify(view.mentionSerial > serial)
    view.history = [message(77)]
    wait(20)
    compare(reads.length, 0)
  }
  function test_away_and_back_drops_inflight_result() {
    var view = prepare()
    view.nextMention()
    model.openTopic = model.topicData[1]
    model.openTopic = model.topicData[0]
    callbacks[0]({ ok: true, result: { messageId: 77 } })
    compare(view.pendingMention, 0)
    verify(!view.mentionBusy)
    compare(reads.length, 0)
  }
  function test_topic_to_thread_with_same_id_invalidates() {
    var view = prepare()
    view.pendingMention = 77
    view.mentionBusy = true
    var serial = view.mentionSerial
    model.openTopic = { id: 1, chatId: -102, thread: true, draft: "", name: "Synthetic thread" }
    compare(view.pendingMention, 0)
    verify(!view.mentionBusy)
    verify(view.mentionSerial > serial)
    compare(view.draftThread, true)
  }
  function test_account_leave_invalidates_result() {
    var view = prepare()
    view.nextMention()
    callbacks[0]({ ok: true, result: { messageId: 77 } })
    view.leaveAccount()
    compare(view.pendingMention, 0)
    model.activeAccount = "work"
    view.history = [message(77)]
    wait(20)
    compare(reads.length, 0)
  }
  function test_reaction_away_and_back_drops_inflight_clear() {
    var view = prepare()
    view.nextReaction()
    model.openTopic = model.topicData[1]
    model.openTopic = model.topicData[0]
    callbacks[0]({ ok: true, result: { messageId: 0 } })
    compare(reads.length, 0)
  }
  function test_reaction_same_id_thread_drops_clear() {
    var view = prepare()
    view.nextReaction()
    model.openTopic = { id: 1, chatId: -102, thread: true, draft: "", name: "Synthetic thread" }
    callbacks[0]({ ok: true, result: { messageId: 0 } })
    compare(reads.length, 0)
  }
  function test_reaction_account_leave_drops_clear() {
    var view = prepare()
    view.nextReaction()
    view.leaveAccount()
    callbacks[0]({ ok: true, result: { messageId: 0 } })
    compare(reads.length, 0)
  }
  function test_reaction_loaded_message_keeps_topic_context() {
    var view = prepare()
    view.history = [message(77)]
    view.nextReaction()
    callbacks[0]({ ok: true, result: { messageId: 77 } })
    compare(model.readLog.length, 1)
    compare(model.readLog[0].topicId, 1)
    compare(model.readLog[0].messageIds[0], 77)
  }
  function test_reaction_current_context_clears_once() {
    var view = prepare()
    view.nextReaction()
    callbacks[0]({ ok: true, result: { messageId: 0 } })
    compare(reads.length, 1)
    compare(reads[0].cmd, "chat.readReactions")
    compare(reads[0].args.topicId, 1)
    callbacks[0]({ ok: true, result: { messageId: 0 } })
    compare(reads.length, 1)
  }
  function test_duplicate_mention_completion_clears_once() {
    var view = prepare()
    view.nextMention()
    var callback = callbacks[0]
    callback({ ok: true, result: { messageId: 0 } })
    callback({ ok: true, result: { messageId: 0 } })
    compare(reads.length, 1)
    compare(reads[0].cmd, "chat.readMentions")
    compare(reads[0].args.topicId, 1)
  }
  function test_duplicate_mention_error_cannot_become_navigation() {
    var view = prepare()
    view.nextMention()
    var callback = callbacks[0]
    callback({ ok: false, error: "Synthetic mention rejection" })
    var notice = view.notice
    callback({ ok: true, result: { messageId: 77 } })
    compare(view.notice, notice)
    compare(view.pendingMention, 0)
    compare(reads.length, 0)
    verify(!view.mentionBusy)
    view.nextMention()
    compare(callbacks.length, 2)
    callback({ ok: true, result: { messageId: 0 } })
    verify(view.mentionBusy)
    callbacks[1]({ ok: true, result: { messageId: 0 } })
    compare(reads.length, 1)
  }
  function test_duplicate_mention_completion_cannot_restore_finished_navigation() {
    var view = prepare()
    view.nextMention()
    var callback = callbacks[0]
    callback({ ok: true, result: { messageId: 77 } })
    view.history = [message(77)]
    tryCompare(view, "pendingMention", 0)
    compare(reads.length, 1)
    callback({ ok: true, result: { messageId: 77 } })
    compare(view.pendingMention, 0)
    compare(reads.length, 1)
  }
  function test_current_context_reads_once() {
    var view = prepare()
    view.nextMention()
    callbacks[0]({ ok: true, result: { messageId: 77 } })
    view.history = [message(77)]
    tryCompare(view, "pendingMention", 0)
    compare(reads.length, 1)
    compare(reads[0].args.messageIds[0], 77)
    compare(reads[0].args.topicId, 1)
    view.finishMention()
    compare(reads.length, 1)
  }
}
