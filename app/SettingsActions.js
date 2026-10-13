.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function ask(context, text, run) {
  var list = context.list
  var settings = context.settings
    settings.error = ""
    settings.confirm = { text: text, run: run }
    list.forceActiveFocus()
  }

function answer(context, yes) {
  var settings = context.settings
    var question = settings.confirm
    settings.confirm = null
    if (yes && question) question.run()
  }

function activate(context, row) {
  var app = context.app
  var list = context.list
  var photoDialog = context.photoDialog
  var settings = context.settings
    if (!row) return
    if (row.kind === "profileField") {
      settings.startEditing(row)
    } else if (row.kind === "profilePhoto") {
      settings.error = ""
      photoDialog.open()
    } else if (row.kind === "profilePhone") {
      settings.error = "Your phone number is changed in Telegram's app on your phone."
    } else if (row.kind === "privacy") {
      settings.changePrivacy(row)
    } else if (row.kind === "blocked") {
      settings.blockedOpen = !settings.blockedOpen
      if (settings.blockedOpen) settings.loadBlocked()
    } else if (row.kind === "blockedSender") {
      settings.askUnblock(row.sender)
    } else if (row.kind === "password") {
      settings.startPasswordFlow(settings.password && settings.password.hasPassword ? "change" : "on")
    } else if (row.kind === "passwordOff") {
      settings.ask("Turn two-step verification off? Signing in on a new device then takes only the code Telegram sends.",
                   function () { settings.startPasswordFlow("off") })
    } else if (row.kind === "passwordCode") {
      settings.startPasswordFlow("code")
    } else if (row.kind === "accountTtl") {
      settings.changeAccountTtl()
    } else if (row.kind === "autoDelete") {
      settings.changeDefaultAutoDelete()
    } else if (row.kind === "proxy") {
      settings.toggleProxy(row.proxy)
    } else if (row.kind === "bridge") {
      settings.toggleBridge()
    } else if (row.kind === "bridgeTest") {
      if (settings.bridgeBusy) return
      settings.bridgeBusy = true
      settings.bridgeTestResult = "Testing…"
      settings.app.request("bridge.test", {}, function (answer) {
        settings.bridgeBusy = false
        settings.bridgeTestResult = answer.ok ? "Telegram responded in " + Math.round(answer.result.seconds * 1000) + " ms" : (answer.error || "Could not reach Telegram through the bridge")
        settings.loadProxies()
      })
    } else if (row.kind === "proxyAdd") {
      settings.startProxyFlow(row.type)
    } else if (row.kind === "reactionsSeen") {
      settings.app.request("settings.reactions", { seen: !settings.app.reactionsSeen }, function (answer) {
        if (!answer.ok) settings.error = answer.error || "The setting could not be saved"
      })
    } else if (row.kind === "sound") {
      settings.app.request("settings.sounds", { style: Model.nextSoundStyle(settings.app.soundStyle) }, function (answer) {
        if (!answer.ok) settings.error = answer.error || "The setting could not be saved"
      })
    } else if (row.kind === "showStories") {
      settings.app.request("settings.set", { settings: { showStories: !settings.app.showStories } }, function (answer) {
        if (!answer.ok) settings.error = answer.error || "Could not change story visibility"
        else settings.app.showStories = answer.result.settings.showStories
      })
    } else if (row.kind === "soundHear") {
      settings.app.request("sounds.play", { userId: settings.app.meId }, function () {})
    } else if (row.kind === "scope") {
      settings.changeScope(row.id, { muted: !(settings.scopes[row.id] && settings.scopes[row.id].muted) })
    } else if (row.kind === "previews") {
      settings.togglePreviews()
    } else if (row.kind === "download") {
      settings.app.setAutoDownload(Model.nextDownloadRule(settings.app.autoDownloadRules, row.id))
    } else if (row.kind === "folder") {
      settings.openFolder(row.folder)
    } else if (row.kind === "newFolder") {
      settings.openFolder(null)
    } else if (row.kind === "folderName") {
      settings.startEditingFolderName()
    } else if (row.kind === "folderFlag") {
      var flag = {}
      flag[row.key] = !settings.folderOpen[row.key]
      settings.changeFolder(flag)
    } else if (row.kind === "folderChat") {
      settings.removeFolderChat(row.list, row.chatId)
    } else if (row.kind === "folderAdd") {
      settings.startAddingChat(row.list)
    } else if (row.kind === "folderSave") {
      settings.saveFolder()
    } else if (row.kind === "folderDelete") {
      settings.askDeleteFolder()
    } else if (row.kind === "storage") {
      settings.ask("Clear the cache? Downloaded photos, videos and files are deleted from this computer; they download again when you open them.", function () {
        settings.app.request("storage.clear", {}, function (answer) {
          if (!answer.ok) { settings.error = answer.error || "The cache could not be cleared"; return }
          settings.app.clearFileStates()
          settings.loadStorage()
        })
      })
    } else if (row.kind === "sessions") {
      settings.sessionsOpen = !settings.sessionsOpen
      if (settings.sessionsOpen) settings.loadSessions()
    } else if (row.kind === "session") {
      if (row.session.current) { settings.error = "That is this computer: to sign out here, use the last row of the account."; return }
      settings.ask("Sign out " + Model.sessionTitle(row.session) + (row.session.device ? " on " + row.session.device : "") + "?", function () {
        settings.app.request("session.terminate", { id: row.session.id }, function (answer) {
          if (!answer.ok) settings.error = answer.error || "That device could not be signed out"
          settings.loadSessions()
        })
      })
    } else if (row.kind === "otherSessions") {
      settings.ask("Sign out every device except this computer?", function () {
        settings.app.request("sessions.terminateOthers", {}, function (answer) {
          if (!answer.ok) settings.error = answer.error || "The other devices could not be signed out"
          settings.loadSessions()
        })
      })
    } else if (row.kind === "logout") {
      settings.ask("Sign out of Telegram here? Your local profile and encryption key stay available for signing in again.", function () {
        settings.app.request("auth.logout", {}, function (answer) {
          if (!answer.ok) settings.error = answer.error || "Could not sign out"
          else settings.closed()
        })
      })
    } else if (settings.editable(row)) {
      settings.startRecording(false)
    }
  }
