.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function selectList(context, key) {
  var omagram = context.omagram
  var service = context.service
  if (!key) return
  omagram.listKey = key
  if (omagram.auth.state !== "ready") return
  var account = omagram.activeAccount
  var revision = omagram.accountRevision
  var loads = omagram.loadedLists
  function current() {
    return account === omagram.activeAccount && revision === omagram.accountRevision
      && loads === omagram.loadedLists
  }
  function refresh() {
    var before = {}
    omagram.chats.forEach(function (chat) { before[chat.id] = chat })
    service.request("chats.list", { list: key, limit: 500 }, function (answer) {
      if (!current() || !answer.ok) return
      var latest = {}
      omagram.chats.forEach(function (chat) { latest[chat.id] = chat })
      // Do not roll back stream updates flushed while the snapshot was pending.
      var incoming = (answer.result.chats || []).filter(function (chat) {
        return latest[chat.id] === before[chat.id]
      })
      omagram.chats = Model.mergeChatUpdates(omagram.chats, incoming, "")
    })
  }
  refresh()
  if (loads[key]) return
  loads[key] = true
  service.request("chats.load", { list: key, limit: 100 }, function (answer) {
    if (!current()) return
    // TDLib uses 404 when the list is fully loaded; still hydrate known chats.
    if (!answer.ok && answer.code !== 404) delete loads[key]
    refresh()
  })
}

function openChatAt(context, chatId, messageId) {
  var omagram = context.omagram
  var screen = context.screen
  var service = context.service
    omagram.openChatById(chatId, false)
    var topic = omagram.openTopic && omagram.openTopic.chatId === chatId ? omagram.openTopic : null
    var args = { chatId: chatId, fromMessageId: messageId, offset: -20, limit: 40 }
    service.request(omagram.historyCommand(topic, args), args, function (answer) {
      if (!answer.ok || chatId !== omagram.openChatId) return
      var found = (answer.result.messages || []).filter(function (m) { return m.id === messageId })[0]
      var chat = Model.findChat(omagram.chats, chatId)
      // A message of a forum opens in its topic.
      if (!topic && found && found.topicId && chat && chat.forum) {
        omagram.selectTopic({ id: found.topicId, chatId: chatId, name: "Topic", draft: "" })
        omagram.openChatAt(chatId, messageId)
        return
      }
      var key = Model.historyKey(chatId, topic ? topic.id : 0, !!topic && topic.thread === true)
      if (key !== omagram.openKey) return
      omagram.setMessages(key, Model.mergeMessages(omagram.messages[key] || [], answer.result.messages || []))
      var view = screen.item
      var serial = view ? view.navigationSerial : 0
      Qt.callLater(function () {
        if (key !== omagram.openKey || screen.item !== view || !view || view.navigationSerial !== serial) return
        if (view.focusMessage) view.focusMessage(messageId)
      })
    })
  }

function selectTopic(context, topic) {
  var omagram = context.omagram
    if (!topic || topic.chatId !== omagram.openChatId) return
    omagram.openTopic = topic
    omagram.keepMessagesOf(omagram.openKey)
    omagram.loadHistory(topic.chatId, 0)
  }

function openThread(context, chatId, messageId) {
  var omagram = context.omagram
  var screen = context.screen
  var service = context.service
    var key = omagram.openKey
    var account = omagram.activeAccount
    service.request("thread.open", { chatId: chatId, messageId: messageId }, function (answer) {
      if (key !== omagram.openKey || account !== omagram.activeAccount) return
      if (!answer.ok) {
        if (screen.item && screen.item.notify) screen.item.notify(answer.error || "Could not open the comments")
        return
      }
      var t = answer.result
      if (t.chat) omagram.knowChat(t.chat)
      var from = Model.findChat(omagram.chats, chatId)
      var channel = !!from && from.kind === "channel"
      omagram.openChatById(t.chatId, false, {
        thread: true, id: t.threadId, chatId: t.chatId, name: channel ? "Comments" : "Replies",
        subtitle: from ? Model.chatTitle(from, omagram.meId) : "", draft: t.draft || "", root: t.messages || [],
        from: { chatId: chatId, messageId: messageId }
      })
    })
  }

function historyCommand(context, topic, args) {
    if (!topic) return "chat.history"
    if (topic.thread) {
      args.threadId = topic.id
      return "thread.history"
    }
    args.topicId = topic.id
    return "topic.history"
  }

function closeTopic(context) {
  var omagram = context.omagram
  var screen = context.screen
    var topic = omagram.openTopic
    omagram.openTopic = null
    // From comments back to the post they are under, from replies back to the message.
    if (topic && topic.thread && topic.from) {
      if (topic.from.chatId === omagram.openChatId) omagram.keepMessagesOf(omagram.openKey)
      omagram.openChatAt(topic.from.chatId, topic.from.messageId)
      return
    }
    // The forum view is now visible again; refresh it even when the chat itself did not change.
    if (omagram.openChat && omagram.openChat.forum) omagram.topicsChanged(omagram.openChatId)
    Qt.callLater(function () { if (screen.item && screen.item.focusMessages) screen.item.focusMessages() })
  }

function openPhoto(context, message) {
  var omagram = context.omagram
    if (message && message.id) {
      omagram.viewerBlock = message
      omagram.viewerFileId = message.content && message.content.media && message.content.media.file ? message.content.media.file.id : 0
      omagram.viewerMessageId = message.id
    }
  }

function openChatById(context, chatId, force, topic) {
  var omagram = context.omagram
  var service = context.service
    if (!chatId || (chatId === omagram.openChatId && !force && !topic)) return
    omagram.viewerMessageId = 0
    if (chatId !== omagram.openChatId || topic) omagram.openTopic = topic || null
    if (omagram.openChatId && omagram.openChatId !== chatId) service.request("chat.close", { chatId: omagram.openChatId })
    omagram.openChatId = chatId
    service.request("chat.open", { chatId: chatId }, function (answer) {
      if (answer.ok && answer.result.chat) omagram.knowChat(answer.result.chat)
    })
    var chat = Model.findChat(omagram.chats, chatId)
    if (chat && chat.forum && !omagram.openTopic) return   // a forum opens on its topics
    omagram.keepMessagesOf(omagram.openKey)
    omagram.loadHistory(chatId, 0)
  }

function loadHistory(context, chatId, fromMessageId) {
  var omagram = context.omagram
  var screen = context.screen
  var service = context.service
    var topic = chatId === omagram.openChatId && omagram.openTopic && omagram.openTopic.chatId === chatId ? omagram.openTopic : null
    var thread = !!topic && topic.thread === true
    var key = Model.historyKey(chatId, topic ? topic.id : 0, thread)
    if (fromMessageId && (omagram.loadingOlder || omagram.noOlder[key])) return
    if (fromMessageId) omagram.loadingOlder = true
    var args = { chatId: chatId, fromMessageId: fromMessageId, limit: 50 }
    service.request(omagram.historyCommand(topic, args), args, function (answer) {
      if (fromMessageId) omagram.loadingOlder = false
      if (!answer.ok) {
        if (key === omagram.openKey && screen.item && screen.item.notify)
          screen.item.notify(answer.error || "Could not load messages")
        return
      }
      var incoming = answer.result.messages || []
      var before = (omagram.messages[key] || []).length
      var merged = Model.mergeMessages(omagram.messages[key] || [], incoming)
      // Nothing older came: the history is whole. A thread starts from its post or message, on top.
      var whole = fromMessageId ? merged.length === before : thread && incoming.length === 0
      if (whole && thread) merged = Model.mergeMessages(merged, topic.root || [])
      omagram.setMessages(key, merged)
      if (whole) omagram.noOlder[key] = true
      if (key !== omagram.openKey) return
      // TDLib answers the first page from its local cache, which may be short; a thread's first page
      // is asked to go on to its start, so the post it is about shows above a few comments.
      if (!fromMessageId && merged.length > 0 && merged.length < (thread ? 50 : 20)) omagram.loadHistory(chatId, Model.oldestId(merged))
      omagram.markOpenChatRead()
    })
  }

function markOpenChatRead(context) {
  var omagram = context.omagram
    var chat = omagram.openChat
    var topic = chat && omagram.openTopic && omagram.openTopic.chatId === chat.id ? omagram.openTopic : null
    var thread = !!topic && topic.thread === true
    if (!chat || !omagram.windowFocused || !(chat.unread > 0 || topic)) return
    omagram.markRead(chat.id, Model.incomingIds(omagram.messages[omagram.openKey] || [], 100), topic ? topic.id : 0, thread)
  }

function markRead(context, chatId, ids, topicId, thread) {
  var service = context.service
    if (!ids.length) return
    var args = { chatId: chatId, messageIds: ids }
    if (topicId) args[thread ? "threadId" : "topicId"] = topicId
    service.request("chat.read", args)
  }
