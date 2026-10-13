.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function resetAccountView(context) {
  var chatFlush = context.chatFlush
  var omagram = context.omagram
  var screen = context.screen
  var service = context.service
  var storyViewer = context.storyViewer
    chatFlush.stop()
    omagram.pendingChats = ({})
    if (screen.item && screen.item.leaveAccount) screen.item.leaveAccount()
    service.request("ui.focus", { chatId: 0 })
    omagram.accountTransition = true
    screen.active = false
    omagram.settingsOpen = false
    omagram.viewerMessageId = 0
    storyViewer.finish()
    omagram.openChatId = 0
    omagram.openTopic = null
    omagram.messages = ({})
    omagram.messagesRevision++
    omagram.files = ({})
    omagram.filesRevision++
    omagram.lottieCache = ({})
    omagram.customEmoji = ({})
    omagram.customEmojiRevision++
    omagram.noOlder = ({})
    omagram.historyOrder = []
    omagram.loadedLists = ({})
    omagram.loadingOlder = false
    omagram.listKey = "main"
    omagram.chatActions = ({})
    omagram.userStatuses = ({})
    omagram.recording = { state: "idle" }
  }

function applyAccountSnapshot(context, result) {
  var chatFlush = context.chatFlush
  var omagram = context.omagram
  var screen = context.screen
    if (result.accountRevision !== undefined && result.accountRevision < omagram.accountRevision) return
    omagram.accountRevision = result.accountRevision === undefined ? omagram.accountRevision : result.accountRevision
    var changed = result.activeAccount && result.activeAccount !== omagram.activeAccount
    if (changed) omagram.resetAccountView()
    chatFlush.stop()
    omagram.pendingChats = ({})
    omagram.activeAccount = result.activeAccount || omagram.activeAccount
    omagram.auth = result.auth || { state: "starting" }
    omagram.meId = result.meId || 0
    omagram.connection = result.connection || ""
    omagram.recordedSend = result.recordedSend || null
    omagram.chats = result.allChats || result.chats || []
    omagram.folders = result.folders || []
    omagram.mainPosition = result.mainPosition || 0
    if (result.accounts) omagram.accounts = result.accounts
    var calls = {}, stories = {}
    for (var c = 0; c < (result.calls || []).length; c++) calls[result.calls[c].id] = result.calls[c]
    for (var s = 0; s < (result.stories || []).length; s++) stories[result.stories[s].chatId] = result.stories[s]
    omagram.calls = calls
    omagram.activeStories = stories
    omagram.accountTransition = false
    screen.active = true
  }
