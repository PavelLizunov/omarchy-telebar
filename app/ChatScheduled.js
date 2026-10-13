.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function setPinned(context, message, pinned) {
  var client = context.client
  var root = context.root
    if (!message) return
    client.request("message.pin", { chatId: message.chatId, messageId: message.id, pinned: pinned }, function (answer) {
      if (!answer.ok) root.flash(answer.error || (pinned ? "Could not pin the message" : "Could not unpin the message"))
    })
  }

function loadPinned(context) {
  var client = context.client
  var root = context.root
    if (!root.chat) return
    var chatId = root.chat.id
    client.request("chat.pinned", { chatId: chatId }, function (answer) {
      if (root.chat && root.chat.id === chatId) root.pinnedMessage = answer.ok ? answer.result.message : null
    })
  }

function openSendMenu(context, item) {
  var Style = context.Style

  var app = context.app
  var composer = context.composer
  var root = context.root
  var sendMenu = context.sendMenu
    if (!root.chat || root.editingId || root.secretBlocked) return
    root.sendMenuItems = Model.sendMenu(root.chat, app.meId, root.nowMs, composer.text.trim() !== "")
    if (!root.sendMenuItems.length) { root.flash("Type a message first"); return }
    // Above what opened it: the clock button, or the text cursor.
    var at = item ? item.mapToItem(root, 0, 0) : composer.mapToItem(root, composer.cursorRectangle.x, composer.cursorRectangle.y)
    sendMenu.open(at.x, at.y - Style.space(4))
  }

function openScheduled(context) {
  var messageList = context.messageList
  var root = context.root
    if (!root.chat) return
    root.navigationSerial++
    if (root.editingId) root.finishEdit()
    root.selection = ({})
    root.scheduledMessages = []
    root.scheduledOpen = true
    root.cursor = -1
    root.stickToBottom = true
    root.loadScheduled()
    messageList.forceActiveFocus()
  }

function closeScheduled(context) {
  var root = context.root
    root.scheduledSerial++
    root.scheduledLoading = false
    if (!root.scheduledOpen) return
    if (root.editingId) root.finishEdit()
    root.scheduledOpen = false
    root.scheduledMessages = []
    root.cursor = -1
    root.stickToBottom = true
  }

function loadScheduled(context) {
  var client = context.client
  var root = context.root
    if (!root.chat || !root.scheduledOpen) return
    var chatId = root.chat.id
    var serial = ++root.scheduledSerial
    root.scheduledLoading = true
    client.request("chat.scheduled", { chatId: chatId }, function (answer) {
      if (serial !== root.scheduledSerial || !root.chat || root.chat.id !== chatId || !root.scheduledOpen) return
      root.scheduledLoading = false
      if (!answer.ok) { root.flash(answer.error || "Could not load the scheduled messages"); return }
      root.scheduledMessages = Model.scheduledOrder(answer.result.messages)
      if (root.cursor >= root.scheduledMessages.length) root.cursor = root.scheduledMessages.length - 1
    })
  }

function reschedule(context, message, sendAt) {
  var client = context.client
  var root = context.root
  var scheduledReload = context.scheduledReload
    if (!message) return
    client.request("message.reschedule", { chatId: message.chatId, messageId: message.id, sendAt: sendAt }, function (answer) {
      if (!answer.ok) { root.flash(answer.error || "Could not change when the message is sent"); return }
      root.flash(sendAt ? "It will be sent " + Model.scheduleText(sendAt, Date.now()) : "Sent")
      scheduledReload.restart()
    })
  }

function openRescheduleMenu(context, message) {
  var app = context.app
  var messageMenu = context.messageMenu
  var rescheduleMenu = context.rescheduleMenu
  var root = context.root
    if (!message || !root.chat) return
    root.rescheduleMessage = message
    root.rescheduleItems = Model.rescheduleMenu(root.chat, app.meId, root.nowMs)
    rescheduleMenu.open(messageMenu.menuX, messageMenu.menuY)
  }

function sendGif(context, item) {
  var root = context.root
    if (!root.chat || !item || !item.gif || (!item.queryId && !(item.gif.file && item.gif.file.id))) return
    var args = {}
    if (item.queryId) {
      args.queryId = item.queryId
      args.resultId = item.resultId
    } else {
      args.fileId = item.gif.file.id
      args.width = item.gif.width || 0
      args.height = item.gif.height || 0
      args.duration = item.gif.duration || 0
    }
    root.sendExtra("message.sendGif", args, "the GIF")
    // As with stickers, the picker stays open for another one.
  }
