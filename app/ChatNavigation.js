.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function focusComposer(context) {
  var blockButton = context.blockButton
  var composer = context.composer
  var root = context.root
  var topicList = context.topicList
    if (root.showTopics) topicList.forceActiveFocus()
    else if (root.composerBlock !== "" && !root.scheduledOpen) blockButton.forceActiveFocus()
    else composer.forceActiveFocus()
  }

function joinChat(context) {
  var client = context.client
  var root = context.root
    if (!root.chat) return
    var chatId = root.chat.id
    var channel = root.chat.kind === "channel"
    root.joiningChatId = chatId
    client.request("chat.join", { chatId: chatId }, function (answer) {
      root.flash(answer.ok ? Model.joinText(answer.result.state, channel) : (answer.error || "Could not join"))
    })
  }

function focusMessages(context) {
  var ListView = context.ListView

  var messageList = context.messageList
  var root = context.root
  var topicList = context.topicList
    if (root.showTopics) { topicList.forceActiveFocus(); return }
    if (!root.messages.length) return
    if (root.cursor < 0 || root.cursor >= root.messages.length) root.cursor = root.messages.length - 1
    messageList.forceActiveFocus()
    messageList.positionViewAtIndex(root.cursor, ListView.Contain)
  }

function focusMessage(context, messageId) {
  var ListView = context.ListView

  var messageList = context.messageList
  var root = context.root
    var found = Model.findMessage(root.history, messageId)
    if (!found) return false
    root.closeScheduled()
    for (var i = 0; i < root.messages.length; i++) {
      if (root.messages[i].id !== messageId) continue
      root.cursor = i
      root.stickToBottom = false
      messageList.forceActiveFocus()
      messageList.positionViewAtIndex(i, ListView.Center)
      return true
    }
    return false
  }

function resetForChat(context) {
  var app = context.app
  var attachDialog = context.attachDialog
  var autoDeleteMenu = context.autoDeleteMenu
  var composerMenu = context.composerMenu
  var draftTimer = context.draftTimer
  var headerMenu = context.headerMenu
  var messageMenu = context.messageMenu
  var muteMenu = context.muteMenu
  var rescheduleMenu = context.rescheduleMenu
  var root = context.root
  var sendMenu = context.sendMenu
    root.navigationSerial++
    root.scheduledSerial++
    attachDialog.visible = false
    root.invalidateSend()
    root.invalidateMention()
    messageMenu.close()
    muteMenu.close()
    sendMenu.close()
    rescheduleMenu.close()
    autoDeleteMenu.close()
    composerMenu.close()
    headerMenu.close()
    draftTimer.stop()
    root.revealed = ({})
    root.pollChoices = ({})
    root.selection = ({})
    root.prompt = null
    root.replyToId = 0
    root.editingId = 0
    root.editingCaption = false
    root.draftBeforeEdit = ""
    root.cursor = -1
    root.confirmDeleteId = 0
    root.notice = ""
    root.stickToBottom = true
    root.stickersOpen = false
    root.emojiOpen = false
    root.forgetReactions()
    root.pinnedMessage = null
    root.lastTypingMs = 0
    root.translations = ({})
    root.botCommands = null
    root.suggestions = []
    root.attachments = []
    root.joiningChatId = 0
    root.locationOpen = false
    root.dateOpen = false
    root.forgetLinkPreview()
    root.scheduledOpen = false
    root.scheduledMessages = []
    // The chat's draft, as Telegram keeps it: you continue where you left off, on any device.
    root.draftChatId = root.chat ? root.chat.id : 0
    root.draftTopicId = root.topicId
    root.draftThread = root.threadOpen
    root.savedDraft = root.topicId ? (app.openTopic.draft || "") : (root.chat && root.chat.draft ? root.chat.draft : "")
    root.setComposerText(root.savedDraft)
    root.loadPinned()
  }

function switchTopic(context) {
  var app = context.app
  var attachDialog = context.attachDialog
  var root = context.root
    root.navigationSerial++
    root.scheduledSerial++
    attachDialog.visible = false
    root.invalidateSend()
    root.invalidateMention()
    root.leaveChat()
    root.closeScheduled()
    root.draftTopicId = root.topicId
    root.draftThread = root.threadOpen
    root.replyToId = 0
    root.editingId = 0
    root.editingCaption = false
    root.draftBeforeEdit = ""
    root.selection = ({})
    root.cursor = -1
    root.confirmDeleteId = 0
    root.stickToBottom = true
    root.savedDraft = root.topicId ? (app.openTopic.draft || "") : (root.chat.draft || "")
    root.setComposerText(root.savedDraft)
  }
