.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function openEmoji(context) {
  var emojiPanel = context.emojiPanel
  var root = context.root
    if (!root.chat) return
    if (root.emojiOpen && !emojiPanel.reacting) {
      root.closeEmoji()
      return
    }
    root.forgetReactions()
    root.emojiOpen = true
    Qt.callLater(function () { emojiPanel.open() })
  }

function closeEmoji(context) {
  var root = context.root
    root.emojiOpen = false
    root.forgetReactions()
    root.focusComposer()
  }

function openReactionPicker(context, messages) {
  var client = context.client
  var emojiPanel = context.emojiPanel
  var root = context.root
    var targets = root.reactionList(messages)
    if (!targets.length || !root.chat) return
    var token = ++root.reactionToken
    root.reactionTargets = targets
    var first = targets[0]
    client.request("reactions.available", { chatId: first.chatId, messageId: first.id, all: true }, function (answer) {
      if (root.reactionToken !== token) return   // you moved on to another message, or closed it
      if (!answer.ok) {
        root.reactionTargets = []
        root.flash(answer.error || "Telegram did not say which reactions it takes")
        return
      }
      root.emojiOpen = true
      Qt.callLater(function () { emojiPanel.openReactions(answer.result.emoji || []) })
    })
  }

function forgetReactions(context) {
  var root = context.root
    root.reactionTargets = []
    root.reactionToken++
  }

function reactionList(context, messages) {
    if (!messages) return []
    return Array.isArray(messages) ? messages.filter(function (m) { return !!m }) : [messages]
  }

function reactToSelection(context) {
  var root = context.root
    if (root.scheduledOpen) return
    root.openReactionPicker(root.selecting ? Model.selectedMessages(root.messages, root.selection)
                                           : root.selectedMessage)
  }

function reactToAll(context, messages, emoji) {
  var client = context.client
  var root = context.root
    var targets = root.reactionList(messages).slice(0, 100)
    if (!targets.length || !emoji) return
    var adding = Model.reactionAdds(targets, emoji)
    var many = targets.length > 1
    var done = 0
    var failed = 0
    var refusal = ""
    var finish = function () {
      if (done + failed < targets.length) return
      if (!failed) { if (many) root.flash((adding ? "Reacted to " : "Took the reaction off ") + targets.length + " messages") }
      else if (!done) root.flash(refusal || "Could not react")
      else root.flash((adding ? "Reacted to " : "Took the reaction off ") + done + " of " + targets.length + ": " + (refusal || "Telegram refused the rest"))
    }
    for (var i = 0; i < targets.length; i++) {
      client.request("reaction.set", { chatId: targets[i].chatId, messageId: targets[i].id, emoji: emoji, chosen: adding },
                     function (answer) {
                       if (answer.ok) done++
                       else { failed++; refusal = refusal || answer.error || "" }
                       finish()
                     })
    }
  }

function pasteFromClipboard(context, asMedia) {
  var app = context.app
  var client = context.client
  var composer = context.composer
  var root = context.root
    if (!root.chat || root.editingId || !root.canWrite) {
      composer.paste()
      return
    }
    var snapshot = root.sendContext()
    var serial = root.sendSerial
    client.request("clipboard.files", {}, function (answer) {
      if (!root.chat || serial !== root.sendSerial || snapshot.chatId !== root.chat.id || snapshot.topicId !== root.topicId || snapshot.account !== app.activeAccount || snapshot.revision !== root.compositionRevision || root.editingId || !root.canWrite) return
      var found = answer.ok && answer.result ? answer.result : { paths: [], skipped: 0 }
      var paths = Array.isArray(found.paths) ? found.paths : []
      if (found.skipped > 0)
        root.flash(found.skipped === 1 ? "A folder or an empty or unreadable file was left out"
                                       : found.skipped + " folders or empty or unreadable files were left out")
      if (paths.length) {
        root.addAttachments(paths, asMedia)
        if (asMedia === false) root.attachAsMedia = false
      } else if (!(found.skipped > 0)) {
        composer.paste()
      }
    })
  }
