import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.Commons
import "Model.js" as Model
import "Keymap.js" as Keymap
import "ChatNavigation.js" as ChatNavigation
import "ChatAttachments.js" as ChatAttachments
import "ChatActions.js" as ChatActions
import "ChatScheduled.js" as ChatScheduled
import "ChatInfoActions.js" as ChatInfoActions
import "ChatReactions.js" as ChatReactions
import "ChatComposition.js" as ChatComposition
import "ChatSuggestions.js" as ChatSuggestions

// The open chat: its header, the pinned message, messages and the composer.
//
// Every key is in Keymap.js and can be changed in settings. By default -- composer: Enter sends,
// Ctrl+Shift+Enter sends without sound, Ctrl+Alt+Enter sends later, Shift+Enter adds a line, Esc
// cancels a reply or edit, ↑ in an empty composer edits your last message, pasting a copied image
// offers to send it. Anywhere in the chat: Ctrl+O attaches,
// Ctrl+S opens stickers, Ctrl+; emoji, Ctrl+M jumps to a mention, Ctrl+Shift+M mutes; files
// dropped on the chat are sent. Messages: ↑/↓ or j/k select, Enter opens media, r replies, e
// edits, y copies, f forwards, x selects, p pins, s saves a file, m opens the message's menu,
// d or Delete asks to delete (press again to confirm), Esc returns to the composer. A right
// click opens a message's menu; Ctrl+click selects.
FocusScope {
  id: root

  property var app
  readonly property var composer: composerView.input
  readonly property var composerButtons: composerView.buttons
  readonly property var headerButtons: headerView.buttons
  property var client
  property var chat: null
  property var history: []            // the chat's messages, or its open topic's
  property real nowMs: Date.now()

  property real replyToId: 0
  property real editingId: 0
  property int cursor: -1
  property real confirmDeleteId: 0
  property string notice: ""
  property bool stickToBottom: true
  property bool stickersOpen: false
  property bool emojiOpen: false             // the emoji panel, in the stickers' place
  property var reactionTargets: []           // the messages the emoji panel finds a reaction for
  property int reactionToken: 0              // which "which reactions?" answer the panel is still waiting on
  onStickersOpenChanged: if (stickersOpen) root.emojiOpen = false
  onEmojiOpenChanged: if (emojiOpen) root.stickersOpen = false

  readonly property var replyTo: root.replyToId ? Model.findMessage(root.messages, root.replyToId) : null
  readonly property var editing: root.editingId ? Model.findMessage(root.messages, root.editingId) : null
  readonly property var selectedMessage: root.cursor >= 0 && root.cursor < root.messages.length ? root.messages[root.cursor] : null

  signal loadOlder()
  signal toList()
  signal searchRequested(string text)

  // What the messages need from here: which spoilers you opened, poll answers being chosen,
  // messages selected, and a question waiting in the bar (joining a group, starting a bot).
  property var revealed: ({})
  property var pollChoices: ({})
  property var selection: ({})
  readonly property bool selecting: Object.keys(root.selection).length > 0
  readonly property bool messagesFocused: messageList.activeFocus
  property var prompt: null
  readonly property var keyboard: Model.latestKeyboard(root.messages)
  property real keyboardHiddenFor: 0

  // The message menu and what Telegram allows for its message; the mute menu in the header.
  property var menuMessage: null
  property string menuSelectedText: ""
  property var menuProperties: null
  property var menuReactions: []
  property bool menuToComposer: false
  readonly property bool modalOpen: messageMenu.visible || muteMenu.visible || sendMenu.visible || rescheduleMenu.visible
                                    || moreMenu.visible || composerMenu.visible || headerMenu.visible || diceMenu.visible || peoplePicker.visible || pollComposer.visible || peopleList.visible || autoDeleteMenu.visible || attachDialog.visible
  readonly property var people: peopleList    // who reacted or has seen a message, for checks from outside
  readonly property var emoji: emojiPanel
  readonly property var polls: pollComposer   // the poll being made, for checks from outside
  property bool blocked: false        // a dialog over the whole window, such as choosing where to forward
  property bool confirmDeleteRevoke: true
  property var pinnedMessage: null
  property bool infoOpen: false
  // Fit controls to the chat viewport and the host's spacing/font scale.
  readonly property bool compactControls: chatColumn.width < Style.space(600)
  readonly property bool minimalControls: chatColumn.width < Style.space(400)
  readonly property bool infoOverlay: root.width < Style.space(720)

  // What the list shows: the history, or the chat's scheduled messages while scheduledOpen.
  property bool scheduledOpen: false
  onScheduledOpenChanged: root.navigationSerial++
  property var scheduledMessages: []
  property bool scheduledLoading: false
  readonly property var messages: root.scheduledOpen ? root.scheduledMessages : root.history
  property var sendMenuItems: []
  property var rescheduleItems: []
  property var rescheduleMessage: null

  // Translations under their messages: message id -> null while on its way, then { text, entities }.
  property var translations: ({})

  // A secret chat takes messages once both devices have set it up, and none after it has ended.
  readonly property bool secretBlocked: !!root.chat && root.chat.kind === "secret" && (!root.chat.secret || root.chat.secret.state !== "ready")

  // A forum group opens on its topics; a topic opened shows its messages, and what is sent goes
  // there. Comments under a channel post, or the replies to a message, open the same way: as a
  // thread of the chat they live in. The composer's text belongs to draftChatId (and draftTopicId,
  // a thread's when draftThread) until it is saved.
  readonly property bool forum: !!root.chat && root.chat.forum === true
  readonly property bool threadOpen: !!root.chat && !!app.openTopic && app.openTopic.thread === true && app.openTopic.chatId === root.chat.id
  readonly property real topicId: (root.forum || root.threadOpen) && app.openTopic && app.openTopic.chatId === root.chat.id ? app.openTopic.id : 0
  readonly property bool showTopics: root.forum && !root.topicId
  onShowTopicsChanged: if (root.showTopics && root.chat) topicList.refresh()
  property real draftChatId: 0
  property real draftTopicId: 0
  property bool draftThread: false

  // Telegram keeps drafts, so they follow you to your other devices; while you type, the chat
  // sees "typing…" as it would from any Telegram app.
  property bool settingText: false
  property string savedDraft: ""
  property string draftBeforeEdit: ""
  property bool editingCaption: false
  property real lastTypingMs: 0

  signal forwardRequested(real fromChatId, var messageIds)
  signal searchInChatRequested()

  function focusComposer() {
    return ChatNavigation.focusComposer({ blockButton: blockButton, composer: composer, root: root, topicList: topicList })
  }

  // Where you cannot write, a bar stands in for the message box: join the group or channel, or mute a
  // channel only its admins post in.
  readonly property string composerBlock: Model.composerBlock(root.chat)
  readonly property bool canWrite: root.composerBlock === "" && !root.secretBlocked

  // Once Telegram says you are in, the message box comes back where the bar was, with the keys.
  property real joiningChatId: 0

  function joinChat() {
    return ChatNavigation.joinChat({ client: client, root: root })
  }

  onComposerBlockChanged: {
    if (root.composerBlock !== "" || !root.chat || root.chat.id !== root.joiningChatId) return
    root.joiningChatId = 0
    composer.forceActiveFocus()
  }

  function focusMessages() {
    return ChatNavigation.focusMessages({ ListView: ListView, messageList: messageList, root: root, topicList: topicList })
  }

  // A message found by search: the cursor goes to it if it is loaded.
  property int navigationSerial: 0

  function focusMessage(messageId) {
    return ChatNavigation.focusMessage({ ListView: ListView, messageList: messageList, root: root }, messageId)
  }

  function resetForChat() {
    return ChatNavigation.resetForChat({ app: app, attachDialog: attachDialog, autoDeleteMenu: autoDeleteMenu, composerMenu: composerMenu, draftTimer: draftTimer, headerMenu: headerMenu, messageMenu: messageMenu, muteMenu: muteMenu, rescheduleMenu: rescheduleMenu, root: root, sendMenu: sendMenu })
  }

  // Into a forum topic, or back to the topic list: what was being written stays with the topic
  // it was written in, and the topic opened brings back its own draft.
  function switchTopic() {
    return ChatNavigation.switchTopic({ app: app, attachDialog: attachDialog, root: root })
  }

  // ---------------------------------------------------------------- more to send: dice, contact cards, locations

  property bool locationOpen: false
  readonly property var typedLocation: root.locationOpen ? Model.parseLocation(locationField.text) : null

  function openMoreMenu(item) {
    return ChatAttachments.openMoreMenu({ Style: Style, composer: composer, moreMenu: moreMenu, root: root }, item)
  }

  function morePicked(id) {
    return ChatAttachments.morePicked({ Style: Style, app: app, composer: composer, diceMenu: diceMenu, peoplePicker: peoplePicker, pollComposer: pollComposer, root: root }, id)
  }

  // What the + button sends goes where a message would: into the open topic or thread, as a reply.
  function sendExtra(command, args, what) {
    return ChatAttachments.sendExtra({ client: client, root: root, videoNote: videoNote }, command, args, what)
  }

  function sendDice(emoji) {
    return ChatAttachments.sendDice({ root: root }, emoji)
  }

  function sendPoll(poll) {
    return ChatAttachments.sendPoll({ root: root }, poll)
  }

  function shareContact(chatId) {
    return ChatAttachments.shareContact({ app: app, root: root }, chatId)
  }

  property bool dateOpen: false
  readonly property var typedDay: root.dateOpen ? Model.parseDay(dateField.text, root.nowMs) : null

  function openDateBar() {
    return ChatAttachments.openDateBar({ dateField: dateField, root: root })
  }

  function closeDateBar() {
    return ChatAttachments.closeDateBar({ dateField: dateField, root: root })
  }

  // The last message sent by the end of that day: the chat opens around it.
  function jumpToDay() {
    return ChatAttachments.jumpToDay({ client: client, dateField: dateField, root: root })
  }

  function openLocationBar() {
    return ChatAttachments.openLocationBar({ locationField: locationField, root: root })
  }

  function closeLocationBar() {
    return ChatAttachments.closeLocationBar({ locationField: locationField, root: root })
  }

  function sendLocation() {
    return ChatAttachments.sendLocation({ locationField: locationField, root: root })
  }

  function attach(asPhoto) {
    return ChatAttachments.attach({ attachDialog: attachDialog, root: root }, asPhoto)
  }

  function urlToPath(url) {
    return ChatAttachments.urlToPath({  }, url)
  }

  // Files wait above the message box until sent, so a caption can go with them: the text in the box.
  // Ten at most, as many as an album holds; a stray drop of a whole folder does not become a flood.
  property var attachments: []            // [{ path, name, kind }]
  property bool attachAsMedia: true

  function addAttachments(paths, asMedia) {
    return ChatAttachments.addAttachments({ root: root }, paths, asMedia)
  }

  function removeAttachment(index) {
    return ChatAttachments.removeAttachment({ root: root }, index)
  }

  // Photos and videos go as albums, files and music as albums of their own, the caption on the first.
  function sendAttachments(options) {
    return ChatAttachments.sendAttachments({ client: client, composer: composer, root: root, videoNote: videoNote }, options)
  }

  function toggleStickers() {
    return ChatAttachments.toggleStickers({ root: root, stickerPicker: stickerPicker })
  }

  readonly property bool shortcutsOn: !!root.chat && !app.settingsOpen && !root.modalOpen && !root.blocked
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.attach"); enabled: root.shortcutsOn; onActivated: root.attach(true) }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.attachFiles"); enabled: root.shortcutsOn; onActivated: root.attach(false) }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.stickers"); enabled: root.shortcutsOn; onActivated: root.toggleStickers() }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.more"); enabled: root.shortcutsOn && root.canWrite; onActivated: root.openMoreMenu(null) }
  Shortcut {
    sequences: Keymap.keysFor(app.shortcuts, "window.voice")
    enabled: root.shortcutsOn && !videoNote.visible
    onActivated: root.recordingVoice ? root.stopVoice(true) : root.startVoice()
  }
  Shortcut {
    sequences: Keymap.keysFor(app.shortcuts, "window.videoNote")
    enabled: root.shortcutsOn && !root.recordingVoice && root.canWrite
    onActivated: root.openVideo()
  }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "voice.send"); enabled: root.recordingVoice && !app.settingsOpen; onActivated: root.stopVoice(true) }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "voice.cancel"); enabled: root.recordingVoice && !app.settingsOpen; onActivated: root.stopVoice(false) }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.emoji"); enabled: root.shortcutsOn; onActivated: root.openEmoji() }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.nextMention"); enabled: root.shortcutsOn; onActivated: root.nextMention() }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.nextReaction"); enabled: root.shortcutsOn; onActivated: root.nextReaction() }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.jumpToDate"); enabled: root.shortcutsOn; onActivated: root.openDateBar() }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.autoDelete"); enabled: root.shortcutsOn; onActivated: root.openAutoDeleteMenu() }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.silent"); enabled: root.shortcutsOn; onActivated: root.toggleSilent() }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.mute"); enabled: root.shortcutsOn; onActivated: app.toggleMute(root.chat.id) }
  Shortcut {
    sequences: Keymap.keysFor(app.shortcuts, "window.pinnedMessage")
    enabled: root.shortcutsOn && !!root.pinnedMessage
    onActivated: root.jumpTo(root.pinnedMessage.id)
  }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.chatInfo"); enabled: root.shortcutsOn; onActivated: root.toggleInfo() }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "window.topicList"); enabled: root.shortcutsOn && root.topicId > 0; onActivated: app.closeTopic() }

  // ---------------------------------------------------------------- voice messages

  readonly property bool recordingVoice: !!app.recording && app.recording.state === "voice"
                                         && !!root.chat && app.recording.chatId === root.chat.id
  property real recordingNow: Date.now()
  readonly property real recordingSeconds: root.recordingVoice ? Math.max(0, (root.recordingNow - app.recording.startedAt) / 1000) : 0

  Timer {
    interval: 200
    repeat: true
    running: root.recordingVoice
    onTriggered: root.recordingNow = Date.now()
  }

  function startVoice() {
    return ChatAttachments.startVoice({ app: app, client: client, root: root, videoNote: videoNote })
  }

  function stopVoice(send) {
    return ChatAttachments.stopVoice({ client: client, root: root, videoNote: videoNote }, send)
  }

  function composerAction(action, item) {
    return ChatAttachments.composerAction({ Style: Style, composerButtons: composerButtons, composerMenu: composerMenu, root: root }, action, item)
  }

  FilePicker {
    id: attachDialog
    objectName: "attachment-file-dialog"
    app: root.app
    multiple: true
    property bool asPhoto: true
    title: asPhoto ? "Send photos or files" : "Send as files"
    onAccepted: {
      var paths = []
      for (var i = 0; i < selectedFiles.length; i++) {
        var path = root.urlToPath(selectedFiles[i])
        if (path) paths.push(path)
      }
      root.addAttachments(paths, asPhoto)
    }
    onRejected: root.focusComposer()
  }

  DropArea {
    anchors.fill: parent
    enabled: !!root.chat
    onDropped: function (drop) {
      if (!drop.hasUrls) return
      root.addAttachments(drop.urls.map(root.urlToPath).filter(function (p) { return p !== "" }), true)
      drop.accept()
    }

    Rectangle {
      anchors.fill: parent
      visible: parent.containsDrag
      color: Qt.rgba(app.accent.r, app.accent.g, app.accent.b, 0.12)
      border.width: Math.max(1, Style.space(2))
      border.color: app.accent
      z: 10
      Text {
        anchors.centerIn: parent
        text: "Drop to send"
        color: app.foreground
        font.family: app.fontFamily
        font.pixelSize: Style.font.title
      }
    }
  }

  onChatChanged: {
    if (!root.chat || root.chat.id !== root.lastChatId) {
      if (root.lastChatId) root.leaveChat()
      root.lastChatId = root.chat ? root.chat.id : 0
      resetForChat()
    }
  }
  onTopicIdChanged: if (root.chat && root.chat.id === root.draftChatId && root.topicId !== root.draftTopicId) root.switchTopic()
  onThreadOpenChanged: if (root.chat && root.chat.id === root.draftChatId && root.threadOpen !== root.draftThread) root.switchTopic()
  property real lastChatId: 0

  // The rows shown, one per message id, edited in place as the messages change. A new array as
  // the model rebuilt every row on each message sent, received or edited, and the view spent a
  // frame at the wrong place before it was put back at the bottom.
  ListModel { id: rows }
  property var rowIds: []

  function syncRows() {
    var cursorId = root.cursor >= 0 && root.cursor < root.rowIds.length ? root.rowIds[root.cursor] : 0
    var ids = Model.syncRows(rows, root.rowIds, root.messages)
    root.rowIds = ids
    // The cursor stays on its message when older ones load above it.
    if (cursorId) {
      var at = ids.indexOf(cursorId)
      root.cursor = at >= 0 ? at : Math.min(root.cursor, ids.length - 1)
    }
  }

  onMessagesChanged: {
    root.syncRows()
    if (root.stickToBottom) messageList.positionViewAtEnd()   // before the next frame is drawn
  }

  // ---------------------------------------------------------------- what messages ask for

  function copyOf(map) {
    return ChatActions.copyOf({  }, map)
  }

  function reveal(id) {
    return ChatActions.reveal({ root: root }, id)
  }

  function stepFrom(index, delta) {
    return ChatActions.stepFrom({ root: root }, index, delta)
  }

  function jumpTo(messageId) {
    return ChatActions.jumpTo({ app: app, root: root }, messageId)
  }

  function openExternal(url) {
    return ChatActions.openExternal({ Quickshell: Quickshell, root: root }, url)
  }

  // The comments under a channel post, or the replies to a message: they open as a thread of the
  // chat they live in, and Alt+Left leads back here.
  function openThread(message) {
    return ChatActions.openThread({ app: app, root: root }, message)
  }

  function openUsername(username) {
    return ChatActions.openUsername({ app: app, client: client, root: root }, username)
  }

  function openUser(userId) {
    return ChatActions.openUser({ app: app, client: client, root: root }, userId)
  }

  function sendText(text) {
    return ChatActions.sendText({ client: client, root: root }, text)
  }

  function openLink(link, message) {
    return ChatActions.openLink({ app: app, client: client, root: root }, link, message)
  }

  function runPrompt(accept) {
    return ChatActions.runPrompt({ root: root }, accept)
  }

  function pressButton(message, button) {
    return ChatActions.pressButton({ client: client, root: root }, message, button)
  }

  function pressKey(button) {
    return ChatActions.pressKey({ root: root }, button)
  }

  function vote(message, index) {
    return ChatActions.vote({ client: client, root: root }, message, index)
  }

  function submitVote(message) {
    return ChatActions.submitVote({ client: client, root: root }, message)
  }

  function toggleReaction(message, reaction) {
    return ChatActions.toggleReaction({ client: client, root: root }, message, reaction)
  }

  // An album is selected as a whole, as it is shown.
  function toggleSelected(message) {
    return ChatActions.toggleSelected({ root: root }, message)
  }

  function clearSelection() {
    return ChatActions.clearSelection({ root: root })
  }

  readonly property bool promptKeysOn: !!root.prompt && !app.settingsOpen && !root.modalOpen && !root.blocked
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "prompt.accept"); enabled: root.promptKeysOn; onActivated: root.runPrompt(true) }
  Shortcut { sequences: Keymap.keysFor(app.shortcuts, "prompt.cancel"); enabled: root.promptKeysOn; onActivated: root.runPrompt(false) }

  // ---------------------------------------------------------------- the message menu

  function openMenu(message, x, y, selectedText) {
    return ChatActions.openMenu({ client: client, composer: composer, messageMenu: messageMenu, root: root }, message, x, y, selectedText)
  }

  // The selected message's menu, from the keyboard: beside its bubble.
  function openMenuAtCursor() {
    return ChatActions.openMenuAtCursor({ Style: Style, messageList: messageList, root: root })
  }

  function afterMenu() {
    return ChatActions.afterMenu({ root: root })
  }

  function menuPicked(id) {
    return ChatActions.menuPicked({ root: root }, id)
  }

  function favoriteSticker(message) {
    return ChatActions.favoriteSticker({ client: client, root: root }, message)
  }

  // A sticker's set, in the sticker picker: one of yours, or shown last with A to add it.
  function openStickerSet(message) {
    return ChatActions.openStickerSet({ root: root, stickerPicker: stickerPicker }, message)
  }
  readonly property var stickers: stickerPicker   // the sticker picker, for checks from outside

  function react(message, emoji) {
    return ChatActions.react({ client: client, root: root }, message, emoji)
  }

  function retractVote(message) {
    return ChatActions.retractVote({ client: client, root: root }, message)
  }

  // An album's caption is on one of its messages, not always the first.
  function captionHolder(message) {
    return ChatActions.captionHolder({ root: root }, message)
  }

  function copyText(text, done) {
    return ChatActions.copyText({ clipboard: clipboard, root: root }, text, done)
  }

  function copyLink(message) {
    return ChatActions.copyLink({ client: client, root: root }, message)
  }

  function setPinned(message, pinned) {
    return ChatScheduled.setPinned({ client: client, root: root }, message, pinned)
  }

  function loadPinned() {
    return ChatScheduled.loadPinned({ client: client, root: root })
  }

  Connections {
    target: root.app
    function onPinnedChanged(chatId) { if (root.chat && chatId === root.chat.id) root.loadPinned() }
    function onTopicsChanged(chatId) { if (root.forum && chatId === root.chat.id) topicList.refresh() }
    function onScheduledChanged(chatId) { if (root.scheduledOpen && root.chat && chatId === root.chat.id) scheduledReload.restart() }
  }

  // A scheduled message going out arrives as several updates at once: the list loads once for them.
  Timer { id: scheduledReload; interval: 300; onTriggered: root.loadScheduled() }

  // ---------------------------------------------------------------- sending later, scheduled messages

  // The other ways to send what is typed: without sound, at a time, once the other person is online.
  function openSendMenu(item) {
    return ChatScheduled.openSendMenu({ Style: Style, app: app, composer: composer, root: root, sendMenu: sendMenu }, item)
  }

  property int scheduledSerial: 0

  function openScheduled() {
    return ChatScheduled.openScheduled({ messageList: messageList, root: root })
  }

  function closeScheduled() {
    return ChatScheduled.closeScheduled({ root: root })
  }

  function loadScheduled() {
    return ChatScheduled.loadScheduled({ client: client, root: root })
  }

  // sendAt: a date in seconds, -1 once the other person is online, 0 to send it now.
  function reschedule(message, sendAt) {
    return ChatScheduled.reschedule({ client: client, root: root, scheduledReload: scheduledReload }, message, sendAt)
  }

  function openRescheduleMenu(message) {
    return ChatScheduled.openRescheduleMenu({ app: app, messageMenu: messageMenu, rescheduleMenu: rescheduleMenu, root: root }, message)
  }

  // A GIF from the picker: one of yours, or one the search found.
  function sendGif(item) {
    return ChatScheduled.sendGif({ root: root }, item)
  }

  // ---------------------------------------------------------------- translation

  // Into the language of this computer, shown under the original.
  function translate(message) {
    return ChatInfoActions.translate({ client: client, root: root }, message)
  }

  function untranslate(message) {
    return ChatInfoActions.untranslate({ root: root }, message)
  }

  // ---------------------------------------------------------------- forwarding and deleting

  function forward(ids) {
    return ChatInfoActions.forward({ root: root }, ids)
  }

  function forwardSelection() {
    return ChatInfoActions.forwardSelection({ root: root })
  }

  function deleteMessages(ids, revoke) {
    return ChatInfoActions.deleteMessages({ client: client, root: root }, ids, revoke)
  }

  function confirmDelete(ids, revoke) {
    return ChatInfoActions.confirmDelete({ root: root }, ids, revoke)
  }

  // Telegram deletes for everyone what it allows to be, and the rest only for you.
  function deleteSelection() {
    return ChatInfoActions.deleteSelection({ root: root })
  }

  function runSelection(action) {
    return ChatInfoActions.runSelection({ root: root }, action)
  }

  // ---------------------------------------------------------------- files, mentions, emoji, pasting

  function fileOf(message) {
    return ChatInfoActions.fileOf({ app: app }, message)
  }

  function openFile(message) {
    return ChatInfoActions.openFile({ app: app, client: client, root: root }, message)
  }

  function saveFile(message) {
    return ChatInfoActions.saveFile({ app: app, client: client, root: root }, message)
  }

  function openMuteMenu(item) {
    return ChatInfoActions.openMuteMenu({ Style: Style, muteMenu: muteMenu, root: root }, item)
  }

  property bool mentionBusy: false
  property int mentionSerial: 0
  property real pendingMention: 0
  function invalidateMention() {
    return ChatInfoActions.invalidateMention({ root: root })
  }
  function finishMention() {
    return ChatInfoActions.finishMention({ client: client, root: root })
  }
  onHistoryChanged: Qt.callLater(root.finishMention)
  function nextMention() {
    return ChatInfoActions.nextMention({ app: app, client: client, root: root })
  }

  // The next message of yours with a reaction you have not seen; seeing it reads its reactions.
  property int reactionSerial: 0
  function nextReaction() {
    return ChatInfoActions.nextReaction({ app: app, client: client, root: root })
  }

  function showReactions(message) {
    return ChatInfoActions.showReactions({ client: client, peopleList: peopleList, root: root }, message)
  }

  function showViewers(message) {
    return ChatInfoActions.showViewers({ client: client, peopleList: peopleList, root: root }, message)
  }

  function openPerson(row) {
    return ChatInfoActions.openPerson({ app: app, root: root }, row)
  }

  function toBottom() {
    return ChatInfoActions.toBottom({ app: app, messageList: messageList, root: root })
  }

  // ---------------------------------------------------------------- the chat's info

  function openInfo() {
    return ChatInfoActions.openInfo({ infoPanel: infoPanel, root: root })
  }

  function closeInfo() {
    return ChatInfoActions.closeInfo({ root: root })
  }

  function toggleInfo() {
    return ChatInfoActions.toggleInfo({ root: root })
  }

  function infoAction(id) {
    return ChatInfoActions.infoAction({ app: app, client: client, root: root }, id)
  }

  // Silent sending, Telegram's own setting of the chat: everything sent here goes without sound until it is off.
  function toggleSilent() {
    return ChatInfoActions.toggleSilent({ client: client, root: root })
  }

  // After how long messages in the open chat disappear: from its info, or its key.
  function openAutoDeleteMenu() {
    return ChatInfoActions.openAutoDeleteMenu({ Style: Style, autoDeleteMenu: autoDeleteMenu, root: root })
  }

  function setAutoDelete(chat, seconds) {
    return ChatInfoActions.setAutoDelete({ client: client, root: root }, chat, seconds)
  }

  function askLeaveChat(chat) {
    return ChatInfoActions.askLeaveChat({ app: app, client: client, root: root }, chat)
  }

  // Your copy only: the other side keeps theirs, as when you clear a chat in any Telegram app.
  function askClearChat(chat, removeFromList) {
    return ChatInfoActions.askClearChat({ app: app, client: client, root: root }, chat, removeFromList)
  }

  // A secret chat lives on this computer only, as Telegram's secret chats do on any device.
  function startSecretChat(chat) {
    return ChatInfoActions.startSecretChat({ app: app, client: client, root: root }, chat)
  }

  function askEndSecret(chat) {
    return ChatInfoActions.askEndSecret({ app: app, client: client, root: root }, chat)
  }

  // Omarchy's emoji picker types the emoji into whatever has the keyboard: the message box.
  function openEmoji() {
    return ChatReactions.openEmoji({ emojiPanel: emojiPanel, root: root })
  }

  function closeEmoji() {
    return ChatReactions.closeEmoji({ root: root })
  }

  // Every reaction the messages may get, found by name: from a message's menu, or from
  // Shift+R over a selection. Telegram is asked about the first of them -- a chat's reactions
  // are the chat's, and where a single message narrows them, reacting simply reports how many
  // it took rather than hiding a reaction the rest would have accepted.
  function openReactionPicker(messages) {
    return ChatReactions.openReactionPicker({ client: client, emojiPanel: emojiPanel, root: root }, messages)
  }

  // Nothing is being reacted to any more, and no answer still on its way may say otherwise.
  function forgetReactions() {
    return ChatReactions.forgetReactions({ root: root })
  }

  // One message, a list of them, or nothing, as a list.
  function reactionList(messages) {
    return ChatReactions.reactionList({  }, messages)
  }

  // The reaction picker over what you have selected, or over the message at the cursor.
  function reactToSelection() {
    return ChatReactions.reactToSelection({ root: root })
  }

  // One emoji across however many messages. Whether it goes on or comes off is decided once for
  // the whole group, so a selection cannot end up half reacted to by its own toggle; Telegram
  // still refuses individual messages, and what it took is reported as a single line rather than
  // one failure per message.
  function reactToAll(messages, emoji) {
    return ChatReactions.reactToAll({ client: client, root: root }, messages, emoji)
  }

  // Ctrl+V: files a file manager copied, or a copied picture, wait above the message box to be sent like attached ones;
  // anything else pastes as text. `asMedia` false (Ctrl+Shift+V) sends them as files, as they are.
  function pasteFromClipboard(asMedia) {
    return ChatReactions.pasteFromClipboard({ app: app, client: client, composer: composer, root: root }, asMedia)
  }

  // ---------------------------------------------------------------- drafts and typing

  function setComposerText(text) {
    return ChatComposition.setComposerText({ composer: composer, root: root }, text)
  }

  Timer { id: draftTimer; interval: 1500; onTriggered: root.saveDraft() }

  function saveDraft() {
    return ChatComposition.saveDraft({ client: client, composer: composer, root: root })
  }

  function composerEdited() {
    return ChatComposition.composerEdited({ client: client, composer: composer, draftTimer: draftTimer, previewLater: previewLater, root: root, suggestLater: suggestLater })
  }

  // ---------------------------------------------------------------- a link's preview while typing

  property var linkPreview: null              // what Telegram would show for the first link typed
  property string linkPreviewLink: ""         // the link it was asked about
  property string linkPreviewMode: "below"    // under the text, "above" it, or "none"

  Timer { id: previewLater; interval: 700; onTriggered: root.lookUpLinkPreview() }

  function lookUpLinkPreview() {
    return ChatComposition.lookUpLinkPreview({ client: client, composer: composer, root: root })
  }

  function cycleLinkPreview() {
    return ChatComposition.cycleLinkPreview({ root: root })
  }

  function forgetLinkPreview() {
    return ChatComposition.forgetLinkPreview({ previewLater: previewLater, root: root })
  }

  // Leaving where the composer's text belongs: its draft is saved and "typing…" ends there.
  function leaveChat() {
    return ChatComposition.leaveChat({ client: client, draftTimer: draftTimer, root: root })
  }

  function leaveAccount() {
    return ChatComposition.leaveAccount({ root: root, videoNote: videoNote })
  }

  function flash(text) {
    return ChatComposition.flash({ noticeTimer: noticeTimer, root: root }, text)
  }

  Timer { id: noticeTimer; interval: 3500; onTriggered: root.notice = "" }

  // What is sent from here goes into the open topic, or the open thread.
  function target(args) {
    return ChatComposition.target({ root: root }, args)
  }

  property bool sendPending: false
  property int sendSerial: 0
  property int compositionRevision: 0
  onAttachmentsChanged: root.compositionRevision++
  onAttachAsMediaChanged: root.compositionRevision++
  onReplyToIdChanged: root.compositionRevision++
  onEditingIdChanged: { root.compositionRevision++; root.navigationSerial++ }
  onLinkPreviewModeChanged: root.compositionRevision++

  function invalidateSend() {
    return ChatComposition.invalidateSend({ root: root })
  }

  function sendContext() {
    return ChatComposition.sendContext({ app: app, root: root })
  }

  function beginSend() {
    return ChatComposition.beginSend({ root: root })
  }

  function completeSend(snapshot, answer, later, replyOnly) {
    return ChatComposition.completeSend({ app: app, draftTimer: draftTimer, root: root }, snapshot, answer, later, replyOnly)
  }

  // `options`: { silent } sends without sound; { sendAt } at a date in seconds, or with -1 once the
  // other person is online.
  function send(options) {
    return ChatComposition.send({ client: client, composer: composer, root: root, videoNote: videoNote }, options)
  }

  function startReply(message) {
    return ChatComposition.startReply({ root: root }, message)
  }

  // Your own text messages and captions; `allowed` when Telegram said this one may be edited.
  function startEdit(message, allowed) {
    return ChatComposition.startEdit({ app: app, client: client, composer: composer, root: root }, message, allowed)
  }

  // Formatting is typed as Telegram's Markdown, as in its own apps: a formatting key puts the markers
  // around the selection (or at the cursor) and takes them off again.
  function wrapSelection(before, after) {
    return ChatComposition.wrapSelection({ composer: composer }, before, after)
  }

  // A link: the selection becomes its text, and the cursor waits where the address goes.
  function wrapLink() {
    return ChatComposition.wrapLink({ composer: composer, root: root })
  }

  // ---------------------------------------------------------------- suggestions while typing

  // @someone in a group, and the /commands of the chat's bots, completing what is typed at the cursor.
  property var suggestions: []            // [{ label, detail, insert, command }]
  property int suggestionCursor: 0
  property var suggestionToken: ({ kind: "", query: "", start: 0, end: 0 })
  property var botCommands: null          // the chat's bots' commands, once asked for
  property int suggestionSerial: 0
  property string dismissedToken: ""      // closed with Esc: it stays closed until what is typed changes

  Timer { id: suggestLater; interval: 150; onTriggered: root.updateSuggestions() }

  function closeSuggestions() {
    return ChatSuggestions.closeSuggestions({ root: root })
  }

  function updateSuggestions() {
    return ChatSuggestions.updateSuggestions({ client: client, composer: composer, emojiPanel: emojiPanel, root: root })
  }

  // `send`: a bot command goes at once, as picking one does in Telegram's apps.
  function pickSuggestion(index, send) {
    return ChatSuggestions.pickSuggestion({ composer: composer, emojiPanel: emojiPanel, root: root }, index, send)
  }

  function suggestionKey(event) {
    return ChatSuggestions.suggestionKey({ app: app, root: root }, event)
  }

  // Leaving an edit brings back what you were writing before it.
  function finishEdit() {
    return ChatSuggestions.finishEdit({ root: root })
  }

  // d twice deletes: for everyone when Telegram allows it, otherwise only for you.
  function askDelete(message) {
    return ChatSuggestions.askDelete({ client: client, root: root }, message)
  }

  function copy(message) {
    return ChatSuggestions.copy({ root: root }, message)
  }

  // Plain text only: copied text is never parsed as markup, which would drop tags from a message
  // and could even fetch the images it names.
  TextEdit { id: clipboard; visible: false; textFormat: TextEdit.PlainText }

  // ------------------------------------------------ no chat yet
  Text {
    anchors.centerIn: parent
    visible: !root.chat
    text: "Choose a chat   Alt+↑ / Alt+↓"
    color: app.muted
    font.family: app.fontFamily
    font.pixelSize: Style.font.body
  }

  ColumnLayout {
    id: chatColumn
    readonly property var chatOwner: root
    anchors.left: parent.left
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.right: infoPanel.visible && !root.infoOverlay ? infoPanel.left : parent.right
    spacing: 0
    visible: !!root.chat

    // ------------------------------------------------ header
    ChatHeader {
      root: chatColumn.chatOwner
      app: chatColumn.chatOwner.app
      id: headerView
    }
    // ------------------------------------------------ the pinned message
    Rectangle {
      Layout.fillWidth: true
      Layout.preferredHeight: visible ? Style.space(44) : 0
      visible: !!root.pinnedMessage && !root.showTopics && !root.scheduledOpen
      color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.03)

      Rectangle {
        x: Style.space(18)
        anchors.verticalCenter: parent.verticalCenter
        width: Style.space(3)
        height: parent.height - Style.space(14)
        radius: width / 2
        color: app.accent
      }
      Column {
        anchors.left: parent.left
        anchors.leftMargin: Style.space(30)
        anchors.right: parent.right
        anchors.rightMargin: Style.space(18)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Style.space(1)

        Text {
          width: parent.width
          elide: Text.ElideRight
          text: "Pinned message   " + Keymap.label(Keymap.keysFor(app.shortcuts, "window.pinnedMessage")[0] || "")
          color: app.muted
          font.family: app.fontFamily
          font.pixelSize: Style.font.caption
          font.bold: true
        }
        Text {
          width: parent.width
          elide: Text.ElideRight
          text: root.pinnedMessage ? Model.previewOf(root.pinnedMessage) : ""
          textFormat: Text.PlainText
          color: app.foreground
          font.family: app.fontFamily
          font.pixelSize: Style.font.bodySmall
        }
      }
      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: if (root.pinnedMessage) root.jumpTo(root.pinnedMessage.id)
      }
      Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: app.border; opacity: 0.25 }
    }

    // ------------------------------------------------ a forum's topics
    TopicList {
      id: topicList
      Layout.fillWidth: true
      Layout.fillHeight: true
      visible: root.showTopics
      app: root.app
      client: root.client
      chat: root.forum ? root.chat : null
      nowMs: root.nowMs
      onOpened: function (topic) {
        app.selectTopic(topic)
        Qt.callLater(root.focusComposer)
      }
    }

    // ------------------------------------------------ messages
    Item {
      Layout.fillWidth: true
      Layout.fillHeight: true
      visible: !root.showTopics

      ListView {
        id: messageList
        anchors.fill: parent
        clip: true
        model: rows
        spacing: Style.space(5)
        boundsBehavior: Flickable.StopAtBounds
        topMargin: Style.space(12)
        // Space under the newest message as a footer, not a bottom margin: going to the end counts
        // a footer, and would stop short of a margin.
        footer: Item { width: 1; height: Style.space(12) }

        WheelScroll {
          view: messageList
          onScrolled: {
            root.stickToBottom = messageList.atYEnd
            if (messageList.contentY <= messageList.originY + Style.space(200) && messageList.count > 0) root.loadOlder()
          }
        }

        onMovementEnded: root.stickToBottom = atYEnd
        // The message box growing or shrinking, or a bar above it coming and going, keeps the
        // newest message in view.
        onHeightChanged: if (root.stickToBottom) positionViewAtEnd()
        onAtYBeginningChanged: if (atYBeginning && count > 0 && moving) root.loadOlder()
        onContentYChanged: if (contentY <= originY + Style.space(200) && count > 0 && (moving || activeFocus)) root.loadOlder()

        Keys.onPressed: function (event) {
          var keys = root.app.shortcuts
          var selected = root.selectedMessage
          function is(id) { return Keymap.matches(keys, id, event) }
          // A scheduled message cannot be replied to, forwarded, selected, pinned or linked yet.
          if (root.scheduledOpen && ["messages.reply", "messages.react", "messages.forward", "messages.select", "messages.pin", "messages.link", "messages.thread"].some(is)) {
            event.accepted = true
            return
          }
          if (is("messages.down")) {
            root.cursor = root.stepFrom(root.cursor, 1)
            root.stickToBottom = root.cursor === root.messages.length - 1
            positionViewAtIndex(root.cursor, ListView.Contain)
          } else if (is("messages.up")) {
            root.cursor = root.stepFrom(root.cursor, -1)
            root.stickToBottom = false
            positionViewAtIndex(root.cursor, ListView.Contain)
            if (root.cursor < 5) root.loadOlder()
          } else if (is("messages.reply")) root.startReply(selected)
          else if (is("messages.react")) root.reactToSelection()
          else if (is("messages.edit")) root.startEdit(selected ? root.captionHolder(selected) : null)
          else if (is("messages.copy")) root.copy(selected)
          else if (is("messages.delete")) root.askDelete(selected)
          else if (is("messages.play") || is("messages.open")) {
            var item = messageList.itemAtIndex(root.cursor)
            if (item && item.mediaItem && item.mediaItem.media) {
              if (is("messages.play")) item.mediaItem.togglePlay()
              else item.mediaItem.activate()
            }
          }
          else if (is("messages.toComposer")) {
            if (root.selecting) root.clearSelection()
            else {
              root.closeScheduled()
              root.cursor = -1
              root.focusComposer()
            }
          }
          else if (is("messages.toList")) root.toList()
          else if (is("messages.last")) { root.cursor = root.messages.length - 1; root.stickToBottom = true; positionViewAtEnd() }
          else if (is("messages.menu")) root.openMenuAtCursor()
          else if (is("messages.forward")) {
            if (root.selecting) root.forwardSelection()
            else if (selected) root.forward(Model.albumIds(root.messages, selected))
          }
          else if (is("messages.select")) root.toggleSelected(selected)
          else if (is("messages.pin")) { if (selected) root.setPinned(selected, !selected.pinned) }
          else if (is("messages.save")) { if (selected) root.saveFile(selected) }
          else if (is("messages.link")) { if (selected) root.copyLink(selected) }
          else if (is("messages.thread")) root.openThread(selected)
          else if (is("messages.speed")) root.flash("Voice and video messages play at " + Model.speedLabel(app.cycleSpeed()))
          else return
          event.accepted = true
        }

        delegate: MessageRow {
          width: messageList.width
          view: root
          app: root.app
          messages: root.messages
        }
      }

      Text {
        anchors.centerIn: parent
        visible: root.scheduledOpen && messageList.count === 0
        text: root.scheduledLoading ? "Loading…" : "No scheduled messages"
        color: app.muted
        font.family: app.fontFamily
        font.pixelSize: Style.font.body
      }

      // Your unread mentions, and the way back to the newest messages with how many are unread.
      Column {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.rightMargin: Style.space(22)
        anchors.bottomMargin: Style.space(18)
        spacing: Style.space(14)

        FloatButton {
          visible: count > 0 && !root.scheduledOpen
          glyph: String.fromCodePoint(0xF02D5)   // md-heart-outline
          count: root.chat && root.chat.unreadReactions > 0 ? root.chat.unreadReactions : 0
          onActivated: root.nextReaction()
        }
        FloatButton {
          objectName: "unread-mentions-button"
          enabled: !root.mentionBusy
          label: root.mentionBusy ? "Finding next mention" : "Next unread mention"
          visible: !!root.chat && root.chat.mentions > 0 && !root.scheduledOpen
          glyph: String.fromCodePoint(0xF0065)   // md-at
          count: root.chat ? root.chat.mentions : 0
          onActivated: root.nextMention()
        }
        FloatButton {
          visible: !!root.chat && messageList.count > 0 && !messageList.atYEnd && !root.scheduledOpen
          glyph: String.fromCodePoint(0xF0140)   // md-chevron-down
          count: root.chat ? root.chat.unread : 0
          onActivated: root.toBottom()
        }
      }
    }

    // ------------------------------------------------ reply / edit / notice bar, and questions
    ChatPromptBar {
      root: chatColumn.chatOwner
      app: chatColumn.chatOwner.app
    }
    // ------------------------------------------------ selected messages
    Rectangle {
      Layout.fillWidth: true
      Layout.preferredHeight: visible ? selectionActions.y + selectionActions.implicitHeight + Style.space(6) : 0
      visible: root.selecting && !!root.chat
      color: Qt.rgba(app.accent.r, app.accent.g, app.accent.b, 0.1)

      Text {
        anchors.left: parent.left
        anchors.leftMargin: Style.space(18)
        anchors.verticalCenter: root.compactControls ? undefined : parent.verticalCenter
        anchors.top: root.compactControls ? parent.top : undefined
        anchors.topMargin: Style.space(7)
        text: Model.selectedIds(root.messages, root.selection).length + " selected"
        color: app.foreground
        font.family: app.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
      }

      Flow {
        id: selectionActions
        x: root.compactControls ? Style.space(10) : Style.space(120)
        y: root.compactControls ? Style.space(30) : Style.space(6)
        width: parent.width - x - Style.space(10)
        spacing: Style.space(4)

        Repeater {
          model: [
            { action: "react", label: "React", key: "messages.react" },
            { action: "forward", label: "Forward", key: "messages.forward" },
            { action: "copy", label: "Copy", key: "messages.copy" },
            { action: "delete", label: "Delete", key: "messages.delete" },
            { action: "clear", label: "Cancel", key: "messages.toComposer" }
          ]
          delegate: Rectangle {
            id: selectionAction
            required property var modelData
            width: selectionLabel.implicitWidth + Style.space(18)
            height: Style.space(28)
            radius: Style.cornerRadius
            color: selectionArea.containsMouse ? Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.12) : "transparent"
            Text {
              id: selectionLabel
              anchors.centerIn: parent
              text: selectionAction.modelData.label + (root.compactControls ? "" : "   " + Keymap.label(Keymap.keysFor(app.shortcuts, selectionAction.modelData.key)[0] || ""))
              color: selectionAction.modelData.action === "delete" ? app.urgent : app.foreground
              font.family: app.fontFamily
              font.pixelSize: Style.font.caption
            }
            MouseArea {
              id: selectionArea
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: root.runSelection(selectionAction.modelData.action)
            }
          }
        }
      }
    }

    // ------------------------------------------------ a bot's keyboard
    Rectangle {
      id: botKeyboard
      Layout.fillWidth: true
      visible: !!root.keyboard && root.keyboardHiddenFor !== root.keyboard.messageId && !root.recordingVoice && !root.showTopics
      Layout.preferredHeight: visible ? keyboardColumn.implicitHeight + Style.space(16) : 0
      color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.03)

      Column {
        id: keyboardColumn
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: Style.space(8)
        spacing: Style.space(4)

        Repeater {
          model: root.keyboard ? root.keyboard.rows : []
          delegate: Row {
            id: keyRow
            required property var modelData
            width: keyboardColumn.width
            spacing: Style.space(4)
            Repeater {
              model: keyRow.modelData
              delegate: Rectangle {
                required property var modelData
                width: (keyRow.width - keyRow.spacing * (keyRow.modelData.length - 1)) / keyRow.modelData.length
                height: Style.space(32)
                radius: Style.cornerRadius
                color: keyArea.containsMouse ? Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.16)
                                             : Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.08)
                Text {
                  anchors.centerIn: parent
                  width: parent.width - Style.space(12)
                  horizontalAlignment: Text.AlignHCenter
                  elide: Text.ElideRight
                  text: modelData.text
                  textFormat: Text.PlainText
                  color: app.foreground
                  font.family: app.fontFamily
                  font.pixelSize: Style.font.bodySmall
                }
                MouseArea {
                  id: keyArea
                  anchors.fill: parent
                  hoverEnabled: true
                  cursorShape: Qt.PointingHandCursor
                  onClicked: root.pressKey(modelData)
                }
              }
            }
          }
        }
      }
    }

    // ------------------------------------------------ stickers
    StickerPicker {
      id: stickerPicker
      objectName: "chat-sticker-picker"
      Layout.fillWidth: true
      Layout.preferredHeight: root.stickersOpen ? Math.min(Style.space(320), root.height * 0.55) : 0
      visible: root.stickersOpen
      app: root.app
      chatId: root.chat ? root.chat.id : 0
      onGifPicked: function (item) { root.sendGif(item) }
      onNotice: function (text) { root.flash(text) }
      onPicked: function (sticker) {
        if (!sticker || !sticker.file || !sticker.file.id) return
        root.sendExtra("message.sendSticker", { fileId: sticker.file.id, width: sticker.width || 0,
                       height: sticker.height || 0, emoji: sticker.emoji || "" }, "the sticker")
        // The picker stays open for another sticker; Esc closes it and goes back to the message box.
      }
      onClosed: {
        root.stickersOpen = false
        root.focusComposer()
      }
    }

    // ------------------------------------------------ emoji
    EmojiPanel {
      id: emojiPanel
      Layout.fillWidth: true
      Layout.preferredHeight: root.emojiOpen ? Math.min(Style.space(300), root.height * 0.6) : 0
      visible: root.emojiOpen
      app: root.app
      onInserted: function (text) {
        var at = composer.cursorPosition
        composer.insert(at, text)
        composer.cursorPosition = at + text.length
      }
      onReacted: function (emoji) {
        var targets = root.reactionTargets
        root.forgetReactions()
        root.emojiOpen = false
        root.focusMessages()
        root.reactToAll(targets, emoji)
      }
      onClosed: root.closeEmoji()
      onReadyChanged: if (ready) root.updateSuggestions()
    }

    // ------------------------------------------------ a date to go to
    Rectangle {
      Layout.fillWidth: true
      Layout.preferredHeight: visible ? dateColumn.implicitHeight + Style.space(20) : 0
      visible: root.dateOpen && !!root.chat && !root.showTopics
      color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.03)

      Rectangle { width: parent.width; height: 1; color: app.border; opacity: 0.35 }

      Column {
        id: dateColumn
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: Style.space(10)
        spacing: Style.space(4)

        Field {
          id: dateField
          width: parent.width
          app: root.app
          label: "Go to a date: today, yesterday, 1 Sep, 01.09.2026 or 2026-09-01   ·   Enter goes   ·   Esc cancels"
          placeholder: "1 Sep"
          maximumLength: 32
          error: dateField.text.trim() !== "" && root.typedDay === null ? "That is not a day to go to" : ""
          onAccepted: root.jumpToDay()
          Keys.onEscapePressed: root.closeDateBar()
        }
        Text {
          visible: root.typedDay !== null
          text: "Goes to the last message of " + Model.dayLabel(root.typedDay || 0, root.nowMs)
          textFormat: Text.PlainText
          color: app.muted
          font.family: app.fontFamily
          font.pixelSize: Style.font.caption
        }
      }
    }

    // ------------------------------------------------ a location to send
    Rectangle {
      Layout.fillWidth: true
      Layout.preferredHeight: visible ? locationColumn.implicitHeight + Style.space(20) : 0
      visible: root.locationOpen && !!root.chat && !root.showTopics
      color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.03)

      Rectangle { width: parent.width; height: 1; color: app.border; opacity: 0.35 }

      Column {
        id: locationColumn
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: Style.space(10)
        spacing: Style.space(4)

        Field {
          id: locationField
          width: parent.width
          app: root.app
          label: "A location: coordinates, or a link from Google Maps or OpenStreetMap   ·   Enter sends   ·   Esc cancels"
          placeholder: "50.4501, 30.5234"
          maximumLength: 2048
          error: locationField.text.trim() !== "" && !root.typedLocation ? "No coordinates in that: paste a map link, or type them like 50.4501, 30.5234" : ""
          onAccepted: root.sendLocation()
          Keys.onEscapePressed: root.closeLocationBar()
        }
        Text {
          visible: !!root.typedLocation
          text: "Sends the place at " + Model.locationText(root.typedLocation)
          textFormat: Text.PlainText
          color: app.muted
          font.family: app.fontFamily
          font.pixelSize: Style.font.caption
        }
      }
    }

    // ------------------------------------------------ files waiting to be sent
    ChatAttachmentStrip {
      root: chatColumn.chatOwner
      app: chatColumn.chatOwner.app
    }
    // ------------------------------------------------ suggestions while typing
    ChatSuggestionStrip {
      root: chatColumn.chatOwner
      app: chatColumn.chatOwner.app
      composer: composer
    }
    // ------------------------------------------------ a link's preview, as it will be sent
    ChatLinkPreviewBar {
      root: chatColumn.chatOwner
      app: chatColumn.chatOwner.app
    }
    // ------------------------------------------------ where you cannot write
    Rectangle {
      Layout.fillWidth: true
      Layout.preferredHeight: visible ? Math.max(Style.space(58), blockedLabel.implicitHeight + Style.space(20)) : 0
      visible: !!root.chat && root.composerBlock !== "" && !root.showTopics && !root.scheduledOpen
      color: "transparent"

      Rectangle { width: parent.width; height: 1; color: app.border; opacity: 0.35 }

      RowLayout {
        anchors.fill: parent
        anchors.margins: Style.space(10)
        spacing: Style.space(14)

        Text {
          id: blockedLabel
          Layout.fillWidth: true
          wrapMode: Text.Wrap
          text: root.composerBlock === "join" ? (root.chat && root.chat.kind === "channel" ? "You are not in this channel" : "You are not in this group")
              : root.composerBlock === "channel" ? "Only the channel's admins post here"
              : root.composerBlock === "left" ? "You left this group: someone in it can add you back"
              : "You can't send messages here"
          textFormat: Text.PlainText
          color: app.muted
          font.family: app.fontFamily
          font.pixelSize: Style.font.bodySmall
        }
        Button {
          id: blockButton
          visible: root.composerBlock === "join" || root.composerBlock === "channel"
          app: root.app
          primary: root.composerBlock === "join"
          text: root.composerBlock === "join" ? (root.chat && root.chat.kind === "channel" ? "Join channel" : "Join group")
              : (root.chat && root.chat.muted ? "Unmute" : "Mute")
          onClicked: {
            if (root.composerBlock === "join") root.joinChat()
            else if (root.chat) app.toggleMute(root.chat.id)
          }
        }
      }
    }

    RecordedSendBar {
      Layout.fillWidth: true
      visible: !!app.recordedSend
      app: root.app
      client: root.client
      record: app.recordedSend
      onNotice: function (text) { root.flash(text) }
    }

    // ------------------------------------------------ composer
    ChatComposer {
      root: chatColumn.chatOwner
      app: chatColumn.chatOwner.app
      id: composerView
      client: chatColumn.chatOwner.client
      videoNote: videoNote
    }
  }

  // ------------------------------------------------ the chat's info
  ChatInfo {
    id: infoPanel
    objectName: "chat-info-panel"
    z: root.infoOverlay ? 20 : 0
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    width: root.infoOverlay ? root.width : Math.min(Style.space(380), Math.max(Style.space(300), root.width * 0.38))
    visible: root.infoOpen && !!root.chat
    app: root.app
    client: root.client
    chat: root.chat
    nowMs: root.nowMs
    onClosed: root.closeInfo()
    onOpenUser: function (userId) { root.openUser(userId) }
    onOpenMessage: function (messageId) { root.jumpTo(messageId) }
    onLinkActivated: function (link) { root.openLink(link, null) }
    onActionRequested: function (id) { root.infoAction(id) }
    onCopyRequested: function (text, done) { root.copyText(text, done) }
  }

  // ------------------------------------------------ video messages
  function openVideo() {
    if (!root.chat || !root.canWrite || root.sendPending || root.recordingVoice || videoNote.visible || app.recordedSend) return
    var context = root.sendContext()
    context.serial = root.sendSerial
    videoNote.open(root.chat.id, context)
  }

  VideoNoteRecorder {
    id: videoNote
    objectName: "chat-video-recorder"
    anchors.fill: parent
    app: root.app
    onRecorded: function (chatId, path, context) {
      if (!context || context.chatId !== chatId || context.account !== app.activeAccount) {
        if (path) client.request("videonote.discard", { path: path })
        return
      }
      var args = { chatId: context.chatId, path: path }
      if (context.topicId) args[context.thread ? "threadId" : "topicId"] = context.topicId
      if (context.replyToId) args.replyToMessageId = context.replyToId
      // A recording owns its original destination, not a new draft under the overlay.
      var snapshot = context.serial === root.sendSerial && !root.sendPending ? root.copyOf(context) : null
      if (snapshot) {
        snapshot.serial = ++root.sendSerial
        snapshot.failurePrefix = "Could not send the video message: "
        root.sendPending = true
        root.flash("Preparing the video message…")
      }
      client.request("videonote.send", args, function (answer) {
        if (snapshot) root.completeSend(snapshot, answer, 0, true)
      })
      root.focusComposer()
    }
    onDiscarded: function (path) {
      if (path) client.request("videonote.discard", { path: path })
      root.focusComposer()
    }
    onFailed: function (message) { root.flash(message) }
  }

  // ------------------------------------------------ menus
  ContextMenu {
    id: composerMenu
    objectName: "composer-overflow-menu"
    anchors.fill: parent
    app: root.app
    upward: true
    items: [
      { id: "attach", label: "Attach photos or files" },
      { id: "emoji", label: "Emoji" },
      { id: "stickers", label: "Stickers" },
      { id: "video", label: "Video message" },
      { id: "voice", label: "Voice message" },
      { id: "later", label: "Send later or without sound" },
      { id: "silent", label: root.chat && root.chat.silent ? "Turn off silent sending" : "Turn on silent sending" },
      { id: "more", label: "Poll, dice, contact or location" }
    ]
    onDismissed: root.focusComposer()
    onPicked: function (id) { root.focusComposer(); root.composerAction(id, composerButtons) }
  }

  ContextMenu {
    id: headerMenu
    objectName: "chat-actions-menu"
    anchors.fill: parent
    app: root.app
    items: [{ id: "info", label: "Chat info" }, { id: "mute", label: "Notifications" }]
      .concat(root.chat && root.chat.hasScheduled && !root.scheduledOpen ? [{ id: "scheduled", label: "Scheduled messages" }] : [])
    onDismissed: root.focusComposer()
    onPicked: function (id) {
      if (id === "info") root.openInfo()
      else if (id === "scheduled") root.openScheduled()
      else root.openMuteMenu(headerButtons)
    }
  }

  ContextMenu {
    id: messageMenu
    objectName: "chat-context-menu"
    anchors.fill: parent
    app: root.app
    items: Model.messageMenu(root.menuMessage, root.menuProperties,
                             !!root.menuMessage && root.translations[root.captionHolder(root.menuMessage).id] !== undefined, root.chat)
           .map(function (item) { return item.id === "copy" && root.menuSelectedText ? { id: "copy", label: "Copy selected text" } : item })
           .concat(root.menuReactions.length ? [{ id: "moreReactions", label: "More reactions…" }] : [])
    reactions: root.menuReactions
    chosen: root.menuMessage ? (root.menuMessage.reactions || []).filter(function (r) { return r.chosen }).map(function (r) { return r.emoji }) : []
    onDismissed: root.afterMenu()
    onPicked: function (id) { root.afterMenu(); root.menuPicked(id) }
    onReacted: function (emoji) { root.afterMenu(); root.react(root.menuMessage, emoji) }
  }

  ContextMenu {
    id: muteMenu
    anchors.fill: parent
    app: root.app
    items: Model.muteMenu(root.chat)
    onDismissed: root.focusComposer()
    onPicked: function (id) {
      root.focusComposer()
      if (root.chat && Model.muteSeconds(id) >= 0) app.muteChat(root.chat.id, Model.muteSeconds(id))
    }
  }

  ContextMenu {
    id: autoDeleteMenu
    anchors.fill: parent
    app: root.app
    items: Model.autoDeleteMenu(root.chat)
    onDismissed: root.focusComposer()
    onPicked: function (id) {
      root.focusComposer()
      if (root.chat && Model.autoDeleteSeconds(id) >= 0) root.setAutoDelete(root.chat, Model.autoDeleteSeconds(id))
    }
  }

  PeopleList {
    id: peopleList
    anchors.fill: parent
    app: root.app
    onPicked: function (row) { root.openPerson(row) }
    onDismissed: root.focusMessages()
  }

  // The + button: a poll, dice, a person's contact card, a location.
  PollComposer {
    id: pollComposer
    anchors.fill: parent
    app: root.app
    onSendRequested: function (poll) { root.sendPoll(poll) }
    onDismissed: root.focusComposer()
  }

  ContextMenu {
    id: moreMenu
    anchors.fill: parent
    app: root.app
    upward: true
    items: Model.moreMenu(root.chat, app.meId)
    onDismissed: root.focusComposer()
    onPicked: function (id) { root.morePicked(id) }
  }

  ContextMenu {
    id: diceMenu
    anchors.fill: parent
    app: root.app
    upward: true
    items: Model.diceMenu()
    onDismissed: root.focusComposer()
    onPicked: function (id) {
      root.focusComposer()
      if (id.indexOf("dice:") === 0) root.sendDice(id.slice(5))
    }
  }

  // Whose contact card to share: the forward picker, people only.
  ForwardPicker {
    id: peoplePicker
    anchors.fill: parent
    app: root.app
    chats: root.app && root.app.chats ? root.app.chats : []
    peopleOnly: true
    title: "Share whose contact card?"
    onPicked: function (chatId, title) { root.shareContact(chatId) }
    onDismissed: root.focusComposer()
  }

  ContextMenu {
    id: sendMenu
    anchors.fill: parent
    app: root.app
    upward: true
    items: root.sendMenuItems
    onDismissed: root.focusComposer()
    onPicked: function (id) {
      root.focusComposer()
      if (id === "scheduled") { root.openScheduled(); return }
      var choice = Model.sendChoice(id)
      if (choice) root.send(choice)
    }
  }

  ContextMenu {
    id: rescheduleMenu
    anchors.fill: parent
    app: root.app
    items: root.rescheduleItems
    onDismissed: root.focusMessages()
    onPicked: function (id) {
      root.focusMessages()
      var choice = Model.sendChoice(id)
      if (choice && root.rescheduleMessage) root.reschedule(root.rescheduleMessage, choice.sendAt)
    }
  }

  // A round button floating over the messages, with a count on it.
  component FloatButton: Rectangle {
    id: floatButton
    property string glyph: ""
    property int count: 0
    property string label: "Jump to message"
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: label
    Keys.onReturnPressed: activated()
    Keys.onSpacePressed: activated()
    signal activated()

    width: Style.space(42)
    height: width
    radius: width / 2
    color: root.app.background
    border.width: 1
    border.color: activeFocus ? root.app.accent : Qt.rgba(root.app.foreground.r, root.app.foreground.g, root.app.foreground.b, floatArea.containsMouse ? 0.4 : 0.18)

    Icon {
      anchors.centerIn: parent
      glyph: floatButton.glyph
      color: floatArea.containsMouse ? root.app.accentText : root.app.foreground
      size: Style.font.title
    }
    Rectangle {
      visible: floatButton.count > 0
      anchors.horizontalCenter: parent.horizontalCenter
      anchors.verticalCenter: parent.top
      height: Style.space(18)
      width: Math.max(height, countText.implicitWidth + Style.space(10))
      radius: height / 2
      color: root.app.accent
      Text {
        id: countText
        anchors.centerIn: parent
        text: floatButton.count > 999 ? "999+" : String(floatButton.count)
        color: root.app.onAccent
        font.family: root.app.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }
    }
    MouseArea {
      id: floatArea
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: floatButton.activated()
    }
  }
}
