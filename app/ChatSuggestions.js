.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function closeSuggestions(context) {
  var root = context.root
    var t = root.suggestionToken
    root.dismissedToken = t.kind + ":" + t.start + ":" + t.query
    root.suggestions = []
  }

function updateSuggestions(context) {
  var client = context.client
  var composer = context.composer
  var emojiPanel = context.emojiPanel
  var root = context.root
    var token = Model.suggestToken(composer.text, composer.cursorPosition)
    root.suggestionToken = token
    if (!root.chat || !composer.activeFocus || !token.kind || token.kind + ":" + token.start + ":" + token.query === root.dismissedToken) {
      root.suggestions = []
      return
    }
    var chatId = root.chat.id
    var group = root.chat.kind === "group"
    var serial = ++root.suggestionSerial
    if (token.kind === "command") {
      if (root.botCommands === null) {
        root.botCommands = []
        client.request("chat.commands", { chatId: chatId }, function (answer) {
          if (!root.chat || root.chat.id !== chatId) return
          root.botCommands = answer.ok ? (answer.result.commands || []) : []
          if (root.botCommands.length) root.updateSuggestions()
        })
      }
      root.suggestions = Model.matchCommands(root.botCommands, token.query).map(function (c) {
        return { label: "/" + c.command, detail: c.description + (group && c.bot ? "  ·  @" + c.bot : ""),
                 insert: Model.commandText(c, group), command: true }
      })
      root.suggestionCursor = 0
    } else if (token.kind === "emoji") {
      root.suggestions = emojiPanel.suggest(token.query, 6)
      root.suggestionCursor = 0
    } else if (token.kind === "mention" && group) {
      client.request("chat.mentions", root.target({ chatId: chatId, query: token.query }), function (answer) {
        if (serial !== root.suggestionSerial || !root.chat || root.chat.id !== chatId) return
        root.suggestions = (answer.ok ? answer.result.people || [] : []).map(function (p) {
          return { label: p.name, detail: p.username ? "@" + p.username : (p.bot ? "bot" : ""), insert: Model.mentionText(p), command: false }
        })
        root.suggestionCursor = 0
      })
    } else {
      root.suggestions = []
    }
  }

function pickSuggestion(context, index, send) {
  var composer = context.composer
  var emojiPanel = context.emojiPanel
  var root = context.root
    var item = root.suggestions[index]
    var token = root.suggestionToken
    if (!item) return
    composer.remove(token.start, token.end)
    composer.insert(token.start, item.insert)
    composer.cursorPosition = token.start + item.insert.length
    if (item.emojiKey) emojiPanel.record(item.emojiKey)
    root.suggestions = []
    if (send && item.command) root.send()
  }

function suggestionKey(context, event) {
  var app = context.app
  var root = context.root
    var keys = root.app.shortcuts
    function is(id) { return Keymap.matchesInText(keys, id, event) }
    if (is("suggest.next")) root.suggestionCursor = Math.min(root.suggestions.length - 1, root.suggestionCursor + 1)
    else if (is("suggest.previous")) root.suggestionCursor = Math.max(0, root.suggestionCursor - 1)
    else if (is("suggest.pick")) root.pickSuggestion(root.suggestionCursor, false)
    else if (is("suggest.send")) root.pickSuggestion(root.suggestionCursor, true)
    else if (is("suggest.close")) root.closeSuggestions()
    else return false
    return true
  }

function finishEdit(context) {
  var root = context.root
    root.editingId = 0
    root.editingCaption = false
    root.setComposerText(root.draftBeforeEdit)
    root.draftBeforeEdit = ""
  }

function askDelete(context, message) {
  var client = context.client
  var root = context.root
    if (root.selecting) { root.deleteSelection(); return }
    if (!message || !root.chat) return
    if (root.confirmDeleteId === message.id) {
      root.deleteMessages(Model.albumIds(root.messages, message), root.confirmDeleteRevoke)
      root.confirmDeleteId = 0
      return
    }
    var id = message.id
    root.confirmDeleteId = id
    root.confirmDeleteRevoke = true
    root.flash("Press again to delete")
    if (message.sending || message.sendAt) return
    client.request("message.properties", { chatId: message.chatId, messageId: id }, function (answer) {
      if (!answer.ok || root.confirmDeleteId !== id) return
      var p = answer.result
      root.confirmDeleteRevoke = p.canDeleteForAll
      if (!p.canDeleteForAll && !p.canDeleteForMe) {
        root.confirmDeleteId = 0
        root.flash("This message cannot be deleted")
      } else {
        root.flash(p.canDeleteForAll ? "Press again to delete for everyone" : "Press again to delete it for you")
      }
    })
  }

function copy(context, message) {
  var root = context.root
    if (root.selecting) {
      var count = Model.selectedIds(root.messages, root.selection).length
      root.copyText(Model.selectionText(root.messages, root.selection), count === 1 ? "Copied the message" : "Copied " + count + " messages")
      return
    }
    if (message) root.copyText(root.captionHolder(message).content.text || Model.previewOf(message))
  }
