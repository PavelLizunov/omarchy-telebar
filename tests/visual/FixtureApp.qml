import QtQuick
import "../../app/Model.js" as Model

QtObject {
  id: fixture
  property color background: "#101315"
  property color foreground: "#cacccc"
  property color border: "#39434a"
  property color accent: "#8ab4b8"
  property color accentText: "#a9d5d9"
  readonly property color onAccent: Qt.rgba(0.063, 0.075, 0.082, 1)
  property color muted: "#8a969f"
  property color urgent: "#dd8e8e"
  property color selected: "#283539"
  property string fontFamily: "Sans Serif"
  property string glyphFamily: "Sans Serif"
  property var auth: ({ state: "phone" })
  property var accounts: [{ id: "default", name: "Alex Demo", phone: "+10000000000", unread: 4 },
                          { id: "work", name: "Work Demo", phone: "", unread: 17 }]
  property string activeAccount: "default"
  property bool accountBusy: false
  property string accountError: ""
  property bool windowFocused: false
  property bool settingsOpen: false
  property var shortcuts: ({})
  property var globalShortcuts: ({})
  property var globalStatus: ({})
  property var chatActions: ({})
  property var userStatuses: ({})
  property real clockMs: 1790726400000
  property real nowMs: clockMs
  property real meId: 900
  property var openTopic: null
  property var recording: ({ state: "idle" })
  property string connection: "ready"
  property bool ready: true
  property bool connected: true
  property real quickChatId: 0
  property string quickDraft: ""
  property real quickClosedAt: 0
  signal messageEvent(string name, var event)
  signal accountChanging()
  property string sendError: ""
  property bool holdSend: false
  property var pendingSend: null
  property var readLog: []
  function sendText(chatId, text, callback) { request("message.send", { chatId: chatId, text: text }, callback) }
  function completeSend() {
    var callback = pendingSend
    pendingSend = null
    if (callback) callback({ ok: sendError === "", error: sendError, result: {} })
  }
  property real playbackRate: 1
  property string soundStyle: "drop"
  property bool reactionsSeen: true
  property bool showStories: true
  property var autoDownloadRules: ({ photos: false, gifs: false, videos: 0, files: 0 })
  property var emojiState: ({ tone: 0, recents: {} })
  property var history: []
  property string imagePath: decodeURIComponent(String(Qt.resolvedUrl("sample-photo.svg")).replace("file://", ""))
  property var photoMessage: ({ id: 50, chatId: 101, date: 1790726000, outgoing: false,
    content: { kind: "photo", text: "Synthetic media preview", media: { width: 960, height: 640,
      file: { id: 50, size: 1500, path: imagePath, active: false } } } })
  property var chats: [
    { id: 101, title: "Alex Demo", kind: "private", userId: 101, unread: 4, mentions: 0, muted: false,
      order: "300", lists: ["main"], positions: { main: { order: "300", pinned: false } },
      myStatus: "member", draft: "", lastReadInbox: 0, lastReadOutbox: 8,
      lastMessage: { id: 8, date: 1790726200, text: "Let's check the new interface", outgoing: false } },
    { id: -102, title: "Community · topics and discussions", kind: "group", forum: true, supergroup: true,
      unread: 17, mentions: 2, muted: false, order: "200", lists: ["main"], positions: { main: { order: "200", pinned: true } },
      myStatus: "member", memberCount: 284, draft: "", lastMessage: { id: 9, date: 1790726300, text: "Design discussion", outgoing: false } },
    { id: 900, title: "Saved Messages", kind: "private", userId: 900, unread: 0, muted: false,
      markedUnread: false,
      order: "100", lists: ["main"], positions: { main: { order: "100", pinned: false } },
      draft: "", lastMessage: { id: 10, date: 1790726100, text: "A note to self", outgoing: true } }
  ]
  property var topicData: [
    { id: 1, chatId: -102, name: "General", general: true, order: "30", pinned: true, unread: 2,
      color: 7322096, mentions: 0, draft: "", lastMessage: { date: 1790726300, text: "Welcome", senderName: "Alex" } },
    { id: 2, chatId: -102, name: "Design and interface feedback", order: "20", unread: 12, mentions: 1,
      color: 13338331, draft: "", lastMessage: { date: 1790726200, text: "New SVG controls", senderName: "Morgan" } },
    { id: 3, chatId: -102, name: "Builds and releases", order: "10", unread: 0, closed: true,
      color: 9432728, draft: "", lastMessage: { date: 1790726000, text: "Release notes", senderName: "Team" } }
  ]
  property var storyChats: [{ chatId: 101, title: "Alex Demo", order: "1", unread: true,
    stories: [{ id: 1, date: 1790726000, unread: true }] }]
  signal pinnedChanged(real chatId)
  signal topicsChanged(real chatId)
  signal scheduledChanged(real chatId)
  property bool photoCanSave: true
  property string photoError: ""
  property bool holdPhotoRequest: false
  property var pendingPhotoRequest: null
  function completePhotoRequest() {
    var callback = pendingPhotoRequest
    pendingPhotoRequest = null
    if (callback) callback({ ok: photoError === "", error: photoError, result: { canSave: photoCanSave } })
  }
  property var lastRequest: null
  function request(cmd, args, callback) {
    lastRequest = { cmd: cmd, args: args }
    if (cmd === "message.properties") {
      pendingPhotoRequest = callback
      if (!holdPhotoRequest) Qt.callLater(completePhotoRequest)
      return
    }
    if (cmd === "file.copyImage" || cmd === "file.save") {
      if (callback) Qt.callLater(function () { callback({ ok: photoError === "", error: photoError, result: {} }) })
      return
    }
    if (cmd === "chat.read") readLog = readLog.concat([args])
    if (cmd === "message.send" || cmd === "message.sendFiles" || cmd === "voice.stop" || cmd === "videonote.stop" || cmd === "message.sendSticker") {
      pendingSend = callback
      if (!holdSend) Qt.callLater(completeSend)
      return
    }
    var result = {}
    if (cmd === "settings.set") result = { settings: { showStories: args.settings.showStories } }
    if (cmd === "topics.list") result = { topics: topicData, next: null }
    else if (cmd === "folders.get") result = { folders: [{ id: 1, name: "Work", groups: true, included: [], pinned: [], excluded: [] }], mainPosition: 0 }
    else if (cmd === "chat.history" || cmd === "topic.history") result = { messages: history }
    else if (cmd === "privacy.get") result = { settings: { status: { base: "contacts", allowed: 0, restricted: 0 }, photo: { base: "everybody" }, phone: { base: "contacts" }, findByPhone: { base: "contacts" }, bio: { base: "everybody" }, birthdate: { base: "contacts" }, forwards: { base: "everybody" }, calls: { base: "contacts" }, invites: { base: "contacts" } } }
    else if (cmd === "notifications.get") result = { scopes: { private: { muted: false, preview: true }, groups: { muted: false, preview: true }, channels: { muted: true, preview: false } } }
    else if (cmd === "blocked.list") result = { total: 0, senders: [] }
    else if (cmd === "password.get") result = { hasPassword: true, hint: "Fictional hint", hasRecoveryEmail: true }
    else if (cmd === "account.ttl") result = { days: 180 }
    else if (cmd === "autoDelete.default") result = { seconds: 0 }
    else if (cmd === "storage.stats") result = { size: 1234567, count: 12, byType: [] }
    else if (cmd === "sessions.list") result = { sessions: [{ id: "1", current: true, device: "Preview desktop", platform: "Linux", application: "Omagram", date: 1790726000 }] }
    else if (cmd === "proxies.list") result = { proxies: [{ id: 1, server: "127.0.0.1", port: 1443, type: "mtproto", enabled: true }], connection: "ready" }
    else if (cmd === "bridge.status") result = { running: true, enabled: false, port: 1443 }
    else if (cmd === "proxy.ping") result = { seconds: 0.045 }
    else if (cmd === "profile.get") result = { firstName: "Alex", lastName: "Demo", username: "demo_account", bio: "Fictional preview profile", phone: "+10000000000" }
    else if (cmd === "contacts.list") result = { contacts: [{ userId: 101, name: "Alex Demo", username: "alexdemo" }, { userId: 102, name: "Morgan Example", username: "morgan" }] }
    else if (cmd === "chat.info") result = { chatId: 101, kind: "private", bio: { text: "Fictional user for UI review", entities: [] }, userId: 101 }
    else if (cmd === "chat.mediaCounts") result = { counts: {} }
    else if (cmd === "stickers.recent") result = { stickers: [] }
    if (callback) Qt.callLater(function () { callback({ ok: true, result: result }) })
  }
  function fileState(file) { return file }
  function download() {}
  function customEmojiImages() { return {} }
  function requestCustomEmoji() {}
  function lottiePath(id, callback) { callback("") }
  function openPhoto() {}
  function openFile() {}
  function openChatById() {}
  function openChatAt() {}
  function openThread() {}
  function expandChatList() {}
  function switchAccount(id) { activeAccount = id }
  function addAccount() {}
  function selectTopic(topic) { openTopic = topic }
  function cycleSpeed() { return 1 }
  function setAutoDownload() {}
  function saveEmojiState() {}
  function clearFileStates() {}
  function declineCall() {}
}
