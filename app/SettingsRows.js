.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function loadVisibleSettings(context) {
  var settings = context.settings
    if (!settings.visible) return
    settings.recording = ""
    settings.error = ""
    settings.confirm = null
    settings.editing = null
    settings.flow = null
    settings.editorError = ""
    settings.folderOpen = null
    settings.folderSerial++
    settings.flowSerial++
    settings.nowMs = Date.now()
    settings.forceActiveFocus()
    settings.loadProfile()
    settings.loadPrivacy()
    settings.loadNotifications()
    settings.loadProxies()
    settings.loadFolders()
    settings.loadStorage()
    settings.loadSessions()
  }

function loadProfile(context) {
  var app = context.app
  var settings = context.settings
    settings.app.request("profile.get", {}, function (answer) { if (answer.ok) settings.profile = answer.result })
  }

function loadStorage(context) {
  var app = context.app
  var settings = context.settings
    settings.app.request("storage.stats", {}, function (answer) { if (answer.ok) settings.storage = answer.result })
  }

function loadSessions(context) {
  var app = context.app
  var settings = context.settings
    settings.app.request("sessions.list", {}, function (answer) { if (answer.ok) settings.sessions = answer.result.sessions || [] })
  }

function buildRows(context) {
  var app = context.app
  var settings = context.settings
    if (settings.folderOpen) return settings.folderRows()
    var out = [{ kind: "header", title: "Profile", note: "" },
               { kind: "profilePhoto", label: "Photo" },
               { kind: "profileField", field: "firstName", label: "First name" },
               { kind: "profileField", field: "lastName", label: "Last name" },
               { kind: "profileField", field: "username", label: "Username" },
               { kind: "profileField", field: "bio", label: "Bio" },
               { kind: "profilePhone", label: "Phone number" },
               { kind: "header", title: "Privacy and security", note: "" },
               { kind: "privacy", id: "status", label: "Last seen and online" },
               { kind: "privacy", id: "photo", label: "Profile photo" },
               { kind: "privacy", id: "phone", label: "Phone number" },
               { kind: "privacy", id: "findByPhone", label: "Finding you by your number" },
               { kind: "privacy", id: "bio", label: "Bio" },
               { kind: "privacy", id: "birthdate", label: "Date of birth" },
               { kind: "privacy", id: "forwards", label: "Your name on messages others forward" },
               { kind: "privacy", id: "calls", label: "Calls" },
               { kind: "privacy", id: "invites", label: "Adding you to groups and channels" },
               { kind: "blocked", label: "Blocked" }]
    if (settings.blockedOpen && settings.blocked)
      for (var b = 0; b < settings.blocked.senders.length; b++) out.push({ kind: "blockedSender", sender: settings.blocked.senders[b] })
    out.push({ kind: "password", label: "Two-step verification" })
    if (settings.password && settings.password.emailCodePattern) out.push({ kind: "passwordCode", label: "Type the code from the email" })
    if (settings.password && settings.password.hasPassword) out.push({ kind: "passwordOff", label: "Turn two-step verification off" })
    out.push({ kind: "accountTtl", label: "Delete my account if I am away for" },
             { kind: "autoDelete", label: "Auto-delete messages in chats you start" },
             { kind: "header", title: "Notifications", note: "For chats that have no notification setting of their own" },
             { kind: "scope", id: "private", label: "Private chats" },
             { kind: "scope", id: "groups", label: "Groups" },
             { kind: "scope", id: "channels", label: "Channels" },
             { kind: "previews", label: "Message text in notifications" },
             { kind: "sound", label: "A sound of their own for each person" },
             { kind: "soundHear", label: "Hear yours" },
             { kind: "header", title: "Chats", note: "" },
             { kind: "reactionsSeen", label: "Reactions to your messages" },
             { kind: "showStories", label: "Show Telegram stories" },
             { kind: "header", title: "Automatic downloads", note: "Stickers and voice messages always download: they are small" },
             { kind: "download", id: "photos", label: "Photos" },
             { kind: "download", id: "gifs", label: "GIFs and round video messages" },
             { kind: "download", id: "videos", label: "Videos" },
             { kind: "download", id: "files", label: "Files and music" },
             { kind: "header", title: "Chat folders", note: "Enter opens a folder  ·  [ and ] move it earlier or later" })
    for (var f = 0; f < settings.folders.length; f++) out.push({ kind: "folder", folder: settings.folders[f], label: settings.folders[f].name })
    out.push({ kind: "newFolder", label: "New folder" },
             { kind: "header", title: "Connection",
               note: (Model.connectionText(settings.app.connection) ? Model.connectionText(settings.app.connection) + "  ·  " : "")
                     + "Enter uses a proxy or stops using it  ·  Backspace removes it" })
    for (var p = 0; p < settings.proxies.length; p++)
      out.push({ kind: "proxy", proxy: settings.proxies[p], label: settings.proxies[p].server + ":" + settings.proxies[p].port })
    out.push({ kind: "bridge", label: "Built-in MTProto → WebSocket bridge" },
             { kind: "bridgeTest", label: "Test the built-in bridge without switching" },
             { kind: "proxyAdd", type: "socks5", label: "Add a SOCKS5 proxy" },
             { kind: "proxyAdd", type: "mtproto", label: "Add an MTProto proxy" },
             { kind: "proxyAdd", type: "http", label: "Add an HTTP proxy" },
             { kind: "proxyAdd", type: "link", label: "Add a proxy from its link" },
             { kind: "header", title: "Account", note: "" },
             { kind: "storage", label: "Storage on this computer" },
             { kind: "sessions", label: "Devices signed in" })
    if (settings.sessionsOpen) {
      for (var i = 0; i < settings.sessions.length; i++) out.push({ kind: "session", session: settings.sessions[i] })
      if (settings.sessions.some(function (s) { return !s.current })) out.push({ kind: "otherSessions", label: "Sign out every other device" })
    }
    out.push({ kind: "logout", label: "Sign out of Telegram on this computer" })
    out.push({ kind: "header", title: "Shortcuts that work anywhere",
               note: "Registered with Hyprland, never written into your config. They need Super, Ctrl or Alt." },
             { kind: "global", id: "global.quickReply", label: "The quick view, as an overlay" },
             { kind: "global", id: "global.panel", label: "The quick view, in the bar's panel" },
             { kind: "global", id: "global.openWindow", label: "Open Telebar" })
    for (var s = 0; s < Keymap.SECTIONS.length; s++) {
      var section = Keymap.SECTIONS[s]
      out.push({ kind: "header", title: section.title,
                 note: section.app === "shell" ? "In Omarchy's shell" : "" })
      for (var a = 0; a < Keymap.ACTIONS.length; a++) {
        var action = Keymap.ACTIONS[a]
        if (action.id.split(".")[0] === section.id) out.push({ kind: "action", id: action.id, label: action.label })
      }
    }
    return out
  }

function move(context, delta) {
  var ListView = context.ListView

  var list = context.list
  var settings = context.settings
    var i = Math.max(0, Math.min(settings.rows.length - 1, settings.cursor + delta))
    var dir = delta < 0 ? -1 : 1
    while (settings.rows[i] && settings.rows[i].kind === "header" && i + dir >= 0 && i + dir < settings.rows.length) i += dir
    while (settings.rows[i] && settings.rows[i].kind === "header" && i - dir >= 0 && i - dir < settings.rows.length) i -= dir
    settings.cursor = i
    list.positionViewAtIndex(i, ListView.Contain)
  }

function editable(context, row) {
    return !!row && (row.kind === "action" || row.kind === "global")
  }

function keysOf(context, row) {
  var settings = context.settings
    if (!settings.editable(row)) return []
    if (row.kind === "global") return settings.globals[row.id] ? [settings.globals[row.id]] : []
    return Keymap.keysFor(settings.overrides, row.id)
  }

function changed(context, row) {
  var settings = context.settings
    if (!settings.editable(row)) return false
    if (row.kind === "global") return !!settings.globals[row.id]
    return Object.prototype.hasOwnProperty.call(settings.overrides, row.id)
  }

function startRecording(context, adds) {
  var settings = context.settings
    var row = settings.current
    if (!settings.editable(row)) return
    settings.error = ""
    settings.recordingAdds = adds && row.kind === "action"
    settings.recording = row.id
  }

function send(context, overrides, globals) {
  var app = context.app
  var settings = context.settings
    settings.error = ""
    settings.app.request("settings.set", { settings: { shortcuts: overrides, globalShortcuts: globals } }, function (answer) {
      if (!answer.ok) settings.error = answer.error || "The settings could not be saved"
    })
  }

function withGlobal(context, id, combo) {
  var settings = context.settings
    var next = {}
    for (var k in settings.globals) if (k !== id) next[k] = settings.globals[k]
    if (combo) next[id] = combo
    return next
  }

function capture(context, event) {
  var settings = context.settings
    var mods = event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.ShiftModifier | Qt.MetaModifier)
    if (event.key === Qt.Key_Escape && !mods) {
      settings.recording = ""
      return
    }
    var sequence = Keymap.fromEvent(event.key, event.modifiers)
    if (sequence === "") return   // a modifier on its own: wait for the key
    var id = settings.recording
    settings.recording = ""
    if (id.indexOf("global.") === 0) {
      var combo = Keymap.toHyprland(sequence)
      if (!combo) {
        settings.error = Keymap.label(sequence) + " cannot work anywhere: use Super, Ctrl or Alt with a letter, digit or named key"
        return
      }
      settings.send(settings.overrides, settings.withGlobal(id, combo))
      return
    }
    var keys = settings.recordingAdds ? Keymap.keysFor(settings.overrides, id).concat([sequence]) : [sequence]
    settings.send(Keymap.withKeys(settings.overrides, id, keys), settings.globals)
  }

function removeLast(context) {
  var settings = context.settings
    var row = settings.current
    if (row && row.kind === "profilePhoto") { settings.askRemovePhoto(); return }
    if (row && row.kind === "proxy") { settings.askRemoveProxy(row.proxy); return }
    if (!settings.editable(row)) return
    if (row.kind === "global") {
      settings.send(settings.overrides, settings.withGlobal(row.id, ""))
      return
    }
    var keys = Keymap.keysFor(settings.overrides, row.id)
    keys.pop()
    settings.send(Keymap.withKeys(settings.overrides, row.id, keys), settings.globals)
  }

function reset(context) {
  var settings = context.settings
    var row = settings.current
    if (!settings.editable(row)) return
    if (row.kind === "global") settings.send(settings.overrides, settings.withGlobal(row.id, ""))
    else settings.send(Keymap.withKeys(settings.overrides, row.id, Keymap.defaultsFor(row.id)), settings.globals)
  }

function statusText(context, id) {
  var settings = context.settings
    var state = settings.globalStatus[id] || "off"
    if (!settings.globals[id]) return "Not set"
    return ({ active: "Active", taken: "Taken by another Hyprland binding", failed: "Hyprland refused it",
              unavailable: "Only inside Hyprland", off: "Not active" })[state] || state
  }
