import QtQuick
import Quickshell
import Quickshell.Io
import "../app"
import "../app/Model.js" as Model
import Quickshell.Hyprland

// Omagram inside Omarchy's shell: keeps the Telegram service running for as long as the
// shell is, and holds the one connection the bar panel and the quick-reply overlay share.
//
// Only what those need lives here -- the chat list, who you are, whether you are signed in.
// Messages are asked for when a panel shows them, and the window keeps its own connection.
Item {
  id: service

  property var shell: null
  property var manifest: null

  readonly property string python: "/usr/bin/python3"
  readonly property string binDir: decodeURIComponent(String(Qt.resolvedUrl("../bin/")).replace("file://", ""))

  property var auth: ({ state: "connecting" })
  property var chats: []
  property real meId: 0
  property var shortcuts: ({})   // your shortcut choices; Keymap.js has the defaults
  property var accounts: []
  property string activeAccount: "default"
  property var pendingChats: ({})
  property int accountRevision: -1
  // Notifications and sounds held back from the bar menu. Telegram's own settings and the
  // unread count are untouched: only what pops up and what sounds.
  property bool quiet: false
  // Stopped from the bar menu: the service is not asked for again until something wants it.
  property bool stopped: false
  property bool quitting: false
  readonly property bool connected: client.connected
  readonly property bool ready: client.connected && service.auth.state === "ready"
  readonly property int totalUnread: {
    var sum = 0
    if (service.accounts && service.accounts.length) {
      for (var i = 0; i < service.accounts.length; i++) {
        sum += (service.accounts[i].unread || 0)
      }
    }
    return service.accounts.length ? sum : Model.unreadTotal(service.chats)
  }
  readonly property int unread: service.totalUnread
  // For the quick view: the voice or round video message being listened to ({ fileId: 0 } when none), one
  // being recorded ({ state: "idle" | "voice" | "video", startedAt, preview }), and what the service last said
  // about a file (a sticker or photo coming down), by file id.
  property var playing: ({ fileId: 0 })
  property var recording: ({ state: "idle" })
  property var files: ({})
  // Where the quick view was when it closed, in the panel or the overlay alike: the chat and the words not yet
  // sent. It opens there again within the hour, so a conversation carried on through it picks up where it was.
  property real quickChatId: 0
  property string quickDraft: ""
  property real quickClosedAt: 0

  function noteFile(file) {
    if (!file || !file.id) return
    var files = Object.keys(service.files).length > 500 ? {} : Object.assign({}, service.files)
    files[file.id] = file
    service.files = files
  }

  function download(fileId) {
    if (!fileId) return
    client.request("file.download", { fileId: fileId, priority: 8 }, function (answer) {
      if (answer.ok && answer.result) service.noteFile(answer.result)
    })
  }

  // Messages as the service reports them (message, messageSent, messageFailed,
  // messageContent, messageEdited, messagesDeleted), for a panel showing a chat's history.
  signal messageEvent(string name, var event)

  // ---------------------------------------------------------------- the service process

  // With --with-parent it ends with the shell. If a copy started by the window already holds
  // the lock this one exits at once, and trying again later takes over when that copy ends.
  // A child process starts with whatever the shell was started with, and the shell is
  // long-lived, so LD_PRELOAD, PYTHONPATH and PYTHONHOME would all reach an interpreter
  // this plugin then trusts. Both helpers are handed an explicit environment instead, and
  // python runs isolated on top of it (-I, which is -E, -P and -s together — both scripts
  // put their own directory on sys.path themselves, so nothing depends on -P's default).
  //
  // What is passed is what they actually use: the runtime directory holds the socket, the
  // display and bus variables are what xdg-open needs to open a link somebody sent you,
  // and PYTHONIOENCODING is not optional when the messages are in any alphabet at all.
  function envWith(names, extra) {
    const env = { "PATH": "/usr/bin:/bin", "PYTHONIOENCODING": "utf-8" }
    for (const name of names) {
      const value = Quickshell.env(name)
      if (value) env[name] = value
    }
    for (const key in extra) if (extra[key]) env[key] = extra[key]
    return env
  }

  readonly property var daemonEnv: service.envWith([
    "HOME", "LANG", "XDG_RUNTIME_DIR", "XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME",
    "HYPRLAND_INSTANCE_SIGNATURE", "WAYLAND_DISPLAY", "DISPLAY",
    "DBUS_SESSION_BUS_ADDRESS", "XDG_CURRENT_DESKTOP", "OMARCHY_PATH",
  ], {})

  Process {
    id: daemon
    clearEnvironment: true
    environment: service.daemonEnv
    command: [service.python, "-I", service.binDir + "omagramd", "--with-parent"]
    running: !service.stopped
    onExited: if (!service.stopped && !service.quitting) restart.restart()
  }

  Timer {
    id: restart
    interval: 30000
    onTriggered: if (!daemon.running && !service.stopped && !service.quitting) daemon.running = true
  }

  // Omagram in the app launcher: Omarchy's launcher lists desktop entries, not plugins, so each start
  // checks ~/.local/share/applications/omagram.desktop and rewrites it only when it is out of date.
  Process {
    clearEnvironment: true
    environment: service.daemonEnv
    command: [service.python, "-I", service.binDir + "omagram", "--desktop-entry"]
    running: true
  }

  // ---------------------------------------------------------------- the connection

  OmagramClient {
    id: client
    binDir: service.binDir
    autoStart: false
    accountId: service.activeAccount
    uiContext: ({ chats: service.chats.length, files: Object.keys(service.files).length })

    onHello: function (result) {
      service.applyAccountSnapshot(result)
      service.shortcuts = result.settings ? (result.settings.shortcuts || ({})) : ({})
      service.quiet = !!(result.settings && result.settings.quiet)
      service.chats = Model.sortChats(result.chats || [])
      service.accounts = result.accounts || []
      service.activeAccount = result.activeAccount || "default"
    }

    onServiceEvent: function (name, e) {
      if (name === "quit") { service.quitting = true; return }
      if (name === "accounts") { service.accounts = e.accounts || []; return }
      if (e.account && typeof e.account === "string" && e.account !== service.activeAccount) {
        if (name === "chat" || name === "auth" || name === "me") accountsDelay.restart()
        return
      }
      if (name === "auth") {
        if (!e.account || e.account === service.activeAccount) {
          service.auth = e.auth
          if (e.auth.state !== "ready") service.chats = []
        }
        accountsDelay.restart()
      } else if (name === "accountSwitched") {
        service.reloadAccount()
      } else if (name === "accountAdded" || name === "accountRemoved") {
        service.reloadAccount()
      } else if (name === "settings") {
        service.shortcuts = e.settings ? (e.settings.shortcuts || ({})) : ({})
        service.quiet = !!(e.settings && e.settings.quiet)
      } else if (name === "me") {
        if (!e.account || e.account === service.activeAccount) service.meId = e.meId || 0
        accountsDelay.restart()
      } else if (name === "chat") {
        if (!e.account || e.account === service.activeAccount) {
          service.pendingChats[e.chat.id] = e.chat
          if (!chatFlush.running) chatFlush.start()
        }
        accountsDelay.restart()
      } else if (name.indexOf("message") === 0) {
        if (!e.account || e.account === service.activeAccount) {
          service.messageEvent(name, e)
        }
      } else if (name === "playing") {
        service.playing = e
      } else if (name === "recording") {
        service.recording = e
      } else if (name === "file") {
        service.noteFile(e.file)
      }
    }

    onConnectedChanged: if (!connected) {
      service.auth = { state: "connecting" }
      service.accountRevision = -1
      if (service.quitting) { service.stopped = true; service.quitting = false }
    }
  }

  function refreshAccounts() {
    client.request("account.list", {}, function (ans) {
      if (ans.ok && ans.result) {
        service.accounts = ans.result.accounts || []
      }
    })
  }

  function switchAccount(accountId) {
    client.request("account.switch", { accountId: accountId }, function (ans) {
      if (ans.ok && ans.result) {
        service.applyAccountSnapshot(ans.result)
        service.refreshAccounts()
      }
    })
  }

  signal accountChanging()

  Timer {
    id: chatFlush
    interval: 100
    onTriggered: {
      var updates = Object.keys(service.pendingChats).map(function (key) { return service.pendingChats[key] })
      service.pendingChats = ({})
      service.chats = Model.mergeChatUpdates(service.chats, updates, "main")
    }
  }

  Timer {
    id: accountsDelay
    interval: 100
    onTriggered: service.refreshAccounts()
  }

  function applyAccountSnapshot(result) {
    if (result.accountRevision !== undefined && result.accountRevision < service.accountRevision) return
    service.accountRevision = result.accountRevision === undefined ? service.accountRevision : result.accountRevision
    if (result.activeAccount && result.activeAccount !== service.activeAccount) {
      service.accountChanging()
      service.files = ({})
      service.playing = { fileId: 0 }
      service.recording = { state: "idle" }
      service.quickChatId = 0
      service.quickDraft = ""
      service.quickClosedAt = 0
    }
    service.activeAccount = result.activeAccount || service.activeAccount
    chatFlush.stop()
    service.pendingChats = ({})
    service.auth = result.auth || { state: "starting" }
    service.meId = result.meId || 0
    service.chats = Model.sortChats(result.chats || [])
    if (result.accounts) service.accounts = result.accounts
  }

  function reloadAccount() {
    client.request("hello", {}, function (answer) {
      if (answer.ok) service.applyAccountSnapshot(answer.result)
    })
  }

  // A Hyprland config reload drops runtime bindings: have the service register your global
  // shortcuts again.
  Connections {
    target: Hyprland
    function onRawEvent(event) {
      if (event && String(event.name) === "configreloaded") client.request("shortcuts.apply", {})
    }
  }

  function request(cmd, args, callback) {
    client.request(cmd, args, callback)
  }

  function sendText(chatId, text, callback) {
    client.request("message.send", { chatId: chatId, text: text }, callback || function () {})
  }

  // ---------------------------------------------------------------- the window

  function setQuiet(quiet) {
    client.request("settings.quiet", { quiet: quiet === true }, function () {})
  }

  // Quit from the bar menu: the window is asked to close, then the service is let go. It
  // comes back when Omagram is opened again, which starts it the way the launcher does.
  function quit() {
    if (service.quitting) return
    service.quitting = true
    restart.stop()
    client.request("app.quit", {}, function (answer) {
      if (!answer.ok) { service.quitting = false; console.warn("Omagram: quit request failed") }
    })
  }

  function openWindow() {
    service.quitting = false
    service.stopped = false
    Quickshell.execDetached([service.python, service.binDir + "omagram"])
  }

  function openChat(chatId) {
    var id = Number(chatId)
    if (!Number.isSafeInteger(id) || id === 0) return service.openWindow()
    Quickshell.execDetached([service.python, service.binDir + "omagram", "--chat", String(id)])
  }
}
