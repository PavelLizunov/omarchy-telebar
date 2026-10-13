.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function translate(context, message) {
  var client = context.client
  var root = context.root
    if (!message || !message.content || !message.content.text) return
    var id = message.id
    var chatId = message.chatId
    var next = root.copyOf(root.translations)
    next[id] = null
    root.translations = next
    client.request("message.translate", { chatId: chatId, messageId: id }, function (answer) {
      if (!root.chat || root.chat.id !== chatId || root.translations[id] !== null) return
      var done = root.copyOf(root.translations)
      if (answer.ok && answer.result.text) {
        done[id] = answer.result
      } else {
        delete done[id]
        root.flash(answer.error || "Could not translate the message")
      }
      root.translations = done
    })
  }

function untranslate(context, message) {
  var root = context.root
    if (!message) return
    var next = root.copyOf(root.translations)
    delete next[message.id]
    root.translations = next
  }

function forward(context, ids) {
  var root = context.root
    if (!root.chat || !ids || !ids.length) return
    root.forwardRequested(root.chat.id, ids.slice(0, 100))
  }

function forwardSelection(context) {
  var root = context.root
    root.forward(Model.selectedIds(root.messages, root.selection))
  }

function deleteMessages(context, ids, revoke) {
  var client = context.client
  var root = context.root
    if (!root.chat || !ids.length) return
    client.request("message.delete", { chatId: root.chat.id, messageIds: ids.slice(0, 100), revoke: revoke }, function (answer) {
      if (!answer.ok) root.flash("Could not delete: " + (answer.error || "unknown error"))
    })
  }

function confirmDelete(context, ids, revoke) {
  var root = context.root
    if (!ids.length) return
    root.prompt = { text: (ids.length === 1 ? "Delete the message" : "Delete " + ids.length + " messages") + (revoke ? " for everyone?" : " for you?"),
                    action: "Delete", run: function () { root.deleteMessages(ids, revoke); root.clearSelection() } }
  }

function deleteSelection(context) {
  var root = context.root
    var ids = Model.selectedIds(root.messages, root.selection)
    if (!ids.length) return
    root.prompt = { text: "Delete " + (ids.length === 1 ? "the message" : ids.length + " messages") + ", for everyone where allowed?",
                    action: "Delete", run: function () { root.deleteMessages(ids, true); root.clearSelection() } }
  }

function runSelection(context, action) {
  var root = context.root
    if (action === "forward") root.forwardSelection()
    else if (action === "react") root.reactToSelection()
    else if (action === "copy") root.copy(null)
    else if (action === "delete") root.deleteSelection()
    else root.clearSelection()
  }

function fileOf(context, message) {
  var app = context.app
    var media = message && message.content ? message.content.media : null
    return media && media.file ? app.fileState(media.file) : null
  }

function openFile(context, message) {
  var app = context.app
  var client = context.client
  var root = context.root
    var file = root.fileOf(message)
    if (!file) return
    if (!file.path) { app.download(file.id, 32); root.flash("Downloading… open it again when it is done"); return }
    var name = Model.saveName(message)
    var run = function () {
      client.request("file.open", { fileId: file.id }, function (answer) { if (!answer.ok) root.flash(answer.error || "Could not open the file") })
    }
    if (Model.riskyFile(name) || Model.riskyFile(file.path.split("/").pop()))
      root.prompt = { text: "“" + name + "” could run a program on this computer. Open it anyway?", action: "Open", run: run }
    else run()
  }

function saveFile(context, message) {
  var app = context.app
  var client = context.client
  var root = context.root
    var file = root.fileOf(message)
    if (!file) return
    if (!file.path) { app.download(file.id, 32); root.flash("Downloading… save it again when it is done"); return }
    client.request("file.save", { fileId: file.id, chatId: message.chatId, messageId: message.id,
                                  fileName: Model.saveName(message) }, function (answer) {
      if (answer.ok) root.flash("Saved to Downloads as " + String(answer.result.path).split("/").pop())
      else root.flash(answer.error || "Could not save the file")
    })
  }

function openMuteMenu(context, item) {
  var Style = context.Style

  var muteMenu = context.muteMenu
  var root = context.root
    if (!root.chat) return
    var at = item.mapToItem(root, item.width, item.height)
    muteMenu.open(at.x - Style.space(260), at.y)
  }

function invalidateMention(context) {
  var root = context.root
    root.reactionSerial++
    root.mentionSerial++
    root.mentionBusy = false
    root.pendingMention = 0
  }

function finishMention(context) {
  var client = context.client
  var root = context.root
    if (!root.pendingMention || !root.chat || !root.focusMessage(root.pendingMention)) return
    var id = root.pendingMention
    root.pendingMention = 0
    client.request("chat.read", root.target({ chatId: root.chat.id, messageIds: [id] }), function (answer) {
      if (!answer.ok) root.flash(answer.error || "Could not mark the mention read")
    })
  }

function nextMention(context) {
  var app = context.app
  var client = context.client
  var root = context.root
    if (!root.chat || root.mentionBusy) return
    var chatId = root.chat.id
    var topic = root.topicId
    var thread = root.threadOpen
    var account = app.activeAccount
    var token = ++root.mentionSerial
    root.mentionBusy = true
    client.request("chat.nextMention", root.target({ chatId: chatId }), function (answer) {
      if (token !== root.mentionSerial) return
      root.mentionSerial++
      root.mentionBusy = false
      if (!root.chat || root.chat.id !== chatId || root.topicId !== topic || root.threadOpen !== thread || app.activeAccount !== account) return
      if (!answer.ok) { root.flash(answer.error || "Could not find the next mention"); return }
      if (answer.result.messageId) {
        root.pendingMention = answer.result.messageId
        if (root.focusMessage(root.pendingMention)) root.finishMention()
        else app.openChatAt(chatId, root.pendingMention)
      } else {
        client.request("chat.readMentions", root.target({ chatId: chatId }), function (cleared) {
          if (!cleared.ok) root.flash(cleared.error || "Could not clear unread mentions")
        })
      }
    })
  }

function nextReaction(context) {
  var app = context.app
  var client = context.client
  var root = context.root
    if (!root.chat) return
    var chatId = root.chat.id
    var topic = root.topicId
    var thread = root.threadOpen
    var account = app.activeAccount
    var token = ++root.reactionSerial
    client.request("chat.nextReaction", root.target({ chatId: chatId }), function (answer) {
      if (token !== root.reactionSerial) return
      root.reactionSerial++
      if (!answer.ok || !root.chat || root.chat.id !== chatId || root.topicId !== topic || root.threadOpen !== thread || app.activeAccount !== account) return
      if (answer.result.messageId) {
        root.jumpTo(answer.result.messageId)
        app.markRead(chatId, [answer.result.messageId], topic, thread)
      } else {
        client.request("chat.readReactions", root.target({ chatId: chatId }))
      }
    })
  }

function showReactions(context, message) {
  var client = context.client
  var peopleList = context.peopleList
  var root = context.root
    if (!message) return
    peopleList.open("Who reacted")
    var serial = peopleList.serial
    client.request("message.reactions", { chatId: message.chatId, messageId: message.id }, function (answer) {
      if (!peopleList.visible || serial !== peopleList.serial) return
      if (!answer.ok) { peopleList.show([]); root.flash(answer.error || "Telegram did not say who reacted"); return }
      peopleList.show((answer.result.reactions || []).map(function (r) {
        return { type: r.type, id: r.id, name: r.name, detail: (r.emoji || "a reaction") + "   " + Model.listTime(r.date, root.nowMs) }
      }))
    })
  }

function showViewers(context, message) {
  var client = context.client
  var peopleList = context.peopleList
  var root = context.root
    if (!message) return
    peopleList.open("Who has seen it")
    var serial = peopleList.serial
    client.request("message.viewers", { chatId: message.chatId, messageId: message.id }, function (answer) {
      if (!peopleList.visible || serial !== peopleList.serial) return
      if (!answer.ok) { peopleList.show([]); root.flash(answer.error || "Telegram did not say who has seen it"); return }
      peopleList.show((answer.result.viewers || []).map(function (v) {
        return { type: "user", id: v.userId, name: v.name, detail: v.date ? "seen " + Model.listTime(v.date, root.nowMs) : "seen" }
      }))
    })
  }

function openPerson(context, row) {
  var app = context.app
  var root = context.root
    root.focusMessages()
    if (row.type === "user") root.openUser(row.id)
    else app.openChatById(row.id, false)
  }

function toBottom(context) {
  var app = context.app
  var messageList = context.messageList
  var root = context.root
    if (!root.chat) return
    root.stickToBottom = true
    messageList.positionViewAtEnd()
    app.loadHistory(root.chat.id, 0)
  }

function openInfo(context) {
  var infoPanel = context.infoPanel
  var root = context.root
    if (!root.chat) return
    root.infoOpen = true
    Qt.callLater(function () { infoPanel.forceActiveFocus() })
  }

function closeInfo(context) {
  var root = context.root
    root.infoOpen = false
    root.focusComposer()
  }

function toggleInfo(context) {
  var root = context.root
    if (root.infoOpen) root.closeInfo()
    else root.openInfo()
  }

function infoAction(context, id) {
  var app = context.app
  var client = context.client
  var root = context.root
    if (!root.chat) return
    if (id === "mute") app.toggleMute(root.chat.id)
    else if (id === "search") root.searchInChatRequested()
    else if (id === "leave") root.askLeaveChat(root.chat)
    else if (id === "clear" || id === "delete") root.askClearChat(root.chat, id === "delete")
    else if (id === "secret") root.startSecretChat(root.chat)
    else if (id === "endSecret") root.askEndSecret(root.chat)
    else if (id === "autoDelete") root.openAutoDeleteMenu()
    else if (id === "hearSound") client.request("sounds.play", { userId: root.chat.userId })
    else if (id === "otherSound") client.request("sounds.another", { userId: root.chat.userId }, function (answer) {
      root.flash(answer.ok ? "A new sound for them: this is it" : (answer.error || "Could not change their sound"))
    })
  }

function toggleSilent(context) {
  var client = context.client
  var root = context.root
    if (!root.chat || !root.canWrite) return
    var chatId = root.chat.id
    var silent = !root.chat.silent
    client.request("chat.setSilent", { chatId: chatId, silent: silent }, function (answer) {
      if (!answer.ok) root.flash(answer.error || "Telegram did not take the change")
      else root.flash(silent ? "Silent sending is on: messages here go without sound" : "Silent sending is off")
    })
  }

function openAutoDeleteMenu(context) {
  var Style = context.Style

  var autoDeleteMenu = context.autoDeleteMenu
  var root = context.root
    if (!root.chat) return
    if (!root.chat.canSetAutoDelete) { root.flash("Messages cannot be set to disappear in this chat"); return }
    autoDeleteMenu.open(Math.max(Style.space(12), root.width - Style.space(300)), Style.space(64))
  }

function setAutoDelete(context, chat, seconds) {
  var client = context.client
  var root = context.root
    client.request("chat.setAutoDelete", { chatId: chat.id, seconds: seconds }, function (answer) {
      if (!answer.ok) root.flash(answer.error || "Telegram did not take the change")
      else root.flash(seconds ? "Messages now disappear after " + Model.autoDeleteText(seconds) : "Messages stay now")
    })
  }

function askLeaveChat(context, chat) {
  var app = context.app
  var client = context.client
  var root = context.root
    if (!chat) return
    var chatId = chat.id
    root.prompt = { text: "Leave “" + Model.chatTitle(chat, app.meId) + "”?", action: "Leave",
                    run: function () {
                      client.request("chat.leave", { chatId: chatId }, function (answer) { if (!answer.ok) root.flash(answer.error || "Could not leave") })
                      root.infoOpen = false
                    } }
  }

function askClearChat(context, chat, removeFromList) {
  var app = context.app
  var client = context.client
  var root = context.root
    if (!chat) return
    var chatId = chat.id
    root.prompt = { text: (removeFromList ? "Delete the chat with “" : "Clear the history of “") + Model.chatTitle(chat, app.meId) + "” for you?",
                    action: removeFromList ? "Delete" : "Clear",
                    run: function () {
                      client.request("chat.clearHistory", { chatId: chatId, removeFromList: removeFromList, revoke: false }, function (answer) {
                        if (!answer.ok) root.flash(answer.error || "Could not clear the history")
                      })
                      if (removeFromList) root.infoOpen = false
                    } }
  }

function startSecretChat(context, chat) {
  var app = context.app
  var client = context.client
  var root = context.root
    if (!chat || !chat.userId) return
    client.request("secret.create", { userId: chat.userId }, function (answer) {
      if (!answer.ok || !answer.result.chatId) { root.flash(answer.error || "Could not start a secret chat"); return }
      root.infoOpen = false
      app.openChatById(answer.result.chatId, false)
    })
  }

function askEndSecret(context, chat) {
  var app = context.app
  var client = context.client
  var root = context.root
    if (!chat) return
    var chatId = chat.id
    root.prompt = { text: "End the secret chat with “" + Model.chatTitle(chat, app.meId) + "”? Nothing more can be sent in it.", action: "End",
                    run: function () {
                      client.request("secret.close", { chatId: chatId }, function (answer) { if (!answer.ok) root.flash(answer.error || "Could not end the secret chat") })
                    } }
  }
