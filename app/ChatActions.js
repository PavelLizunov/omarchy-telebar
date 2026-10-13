.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function copyOf(context, map) {
    var next = {}
    for (var k in map) next[k] = map[k]
    return next
  }

function reveal(context, id) {
  var root = context.root
    var next = root.copyOf(root.revealed)
    next[id] = true
    root.revealed = next
  }

function stepFrom(context, index, delta) {
  var root = context.root
    var i = index + delta
    while (i > 0 && i < root.messages.length - 1 && Model.inAlbumAfterFirst(root.messages, i)) i += delta
    if (delta < 0) while (i > 0 && Model.inAlbumAfterFirst(root.messages, i)) i--
    return Math.max(0, Math.min(root.messages.length - 1, i))
  }

function jumpTo(context, messageId) {
  var app = context.app
  var root = context.root
    if (!root.focusMessage(messageId) && root.chat) app.openChatAt(root.chat.id, messageId)
  }

function openExternal(context, url) {
  var Quickshell = context.Quickshell

  var root = context.root
    var safe = Model.safeUrl(url)
    if (!safe) { root.flash("That link cannot be opened"); return }
    Quickshell.execDetached(["/usr/bin/xdg-open", safe])
  }

function openThread(context, message) {
  var app = context.app
  var root = context.root
    if (!message || !root.chat || message.sendAt || message.sending) return
    if (root.threadOpen && (message.id === root.topicId || message.threadId === root.topicId)) return   // this thread
    if (!message.replies && !message.threadId) { root.flash("This message has no comments or replies"); return }
    app.openThread(message.chatId, message.id)
  }

function openUsername(context, username) {
  var app = context.app
  var client = context.client
  var root = context.root
    client.request("username.chat", { username: username }, function (answer) {
      if (answer.ok && answer.result.chatId) app.openChatById(answer.result.chatId, false)
      else root.flash("No one on Telegram is @" + username)
    })
  }

function openUser(context, userId) {
  var app = context.app
  var client = context.client
  var root = context.root
    if (!userId) return
    client.request("user.chat", { userId: userId }, function (answer) {
      if (answer.ok && answer.result.chatId) app.openChatById(answer.result.chatId, false)
      else root.flash(answer.error || "That chat cannot be opened")
    })
  }

function sendText(context, text) {
  var client = context.client
  var root = context.root
    if (!root.chat || !text) return
    client.request("message.send", root.target({ chatId: root.chat.id, text: text }), function (answer) {
      if (!answer.ok) root.flash("Could not send: " + (answer.error || "unknown error"))
    })
    root.stickToBottom = true
  }

function openLink(context, link, message) {
  var app = context.app
  var client = context.client
  var root = context.root
    var s = String(link || "")
    if (s === "omagram:spoiler") { if (message) root.reveal(message.id); return }
    if (s.indexOf("omagram:mention:") === 0) { root.openUsername(decodeURIComponent(s.slice(16))); return }
    if (s.indexOf("omagram:user:") === 0) { root.openUser(Number(s.slice(13))); return }
    if (s.indexOf("omagram:search:") === 0) { root.searchRequested(decodeURIComponent(s.slice(15))); return }
    if (s.indexOf("omagram:command:") === 0) { root.sendText(decodeURIComponent(s.slice(16))); return }
    var url = Model.safeUrl(s)
    if (!url) return
    if (/^mailto:/i.test(url)) { root.openExternal(url); return }
    client.request("link.open", { url: url }, function (answer) {
      if (!answer.ok) { root.flash(answer.error || "That link cannot be opened"); return }
      var r = answer.result
      if (r.kind === "external") {
        root.openExternal(r.url)
      } else if (r.kind === "chat" && r.chatId) {
        if (r.messageId) app.openChatAt(r.chatId, r.messageId)
        else app.openChatById(r.chatId, false)
        if (r.botStart) root.prompt = { text: "Start the bot?", action: "Start",
                                        run: function () { client.request("bot.start", { chatId: r.chatId, parameter: r.botStart }) } }
      } else if (r.kind === "invite") {
        if (r.chatId) { app.openChatById(r.chatId, false); return }
        root.prompt = { text: "Join “" + r.title + "”" + (r.members ? " (" + r.members + " members)" : "") + "?", action: "Join",
                        run: function () {
                          client.request("chat.joinLink", { link: r.link }, function (joined) {
                            if (joined.ok && joined.result.chatId) app.openChatById(joined.result.chatId, false)
                            else root.flash(joined.error || "Could not join")
                          })
                        } }
      } else if (r.kind === "proxy") {
        root.prompt = { text: "Connect to Telegram through the " + (Model.PROXY_TYPE_NAMES[r.type] || "") + " proxy " + r.server + ":" + r.port + "?",
                        action: "Use it",
                        run: function () {
                          client.request("proxy.addLink", { link: r.link }, function (added) {
                            root.flash(added.ok ? "Connecting through " + r.server + ":" + r.port : (added.error || "Could not add the proxy"))
                          })
                        } }
      }
    })
  }

function runPrompt(context, accept) {
  var root = context.root
    var p = root.prompt
    root.prompt = null
    if (accept && p && p.run) p.run()
  }

function pressButton(context, message, button) {
  var client = context.client
  var root = context.root
    if (!message || !button) return
    if (button.kind === "callback") {
      client.request("button.callback", { chatId: message.chatId, messageId: message.id, data: button.data }, function (answer) {
        if (!answer.ok) { root.flash(answer.error || "The bot did not answer"); return }
        if (answer.result.url) root.openLink(answer.result.url, message)
        else if (answer.result.text) root.flash(answer.result.text)
      })
    } else if (button.kind === "url") {
      root.openLink(button.url, message)
    } else if (button.kind === "user") {
      root.openUser(button.userId)
    } else if (button.kind === "copy") {
      root.copyText(button.copyText)
    } else {
      root.flash("“" + button.text + "” needs an official Telegram app")
    }
  }

function pressKey(context, button) {
  var root = context.root
    if (!button || !root.keyboard) return
    if (button.kind !== "text") { root.flash("“" + button.text + "” needs an official Telegram app"); return }
    root.sendText(button.text)
    if (root.keyboard.oneTime) root.keyboardHiddenFor = root.keyboard.messageId
  }

function vote(context, message, index) {
  var client = context.client
  var root = context.root
    var poll = message && message.content ? message.content.poll : null
    if (!poll || poll.closed) return
    if (!poll.multiple) {
      client.request("poll.vote", { chatId: message.chatId, messageId: message.id, optionIds: [index] }, function (answer) {
        if (!answer.ok) root.flash(answer.error || "Could not vote")
      })
      return
    }
    var chosen = (root.pollChoices[message.id] || []).slice()
    var at = chosen.indexOf(index)
    if (at >= 0) chosen.splice(at, 1)
    else chosen.push(index)
    var next = root.copyOf(root.pollChoices)
    next[message.id] = chosen
    root.pollChoices = next
  }

function submitVote(context, message) {
  var client = context.client
  var root = context.root
    var chosen = root.pollChoices[message.id] || []
    if (!chosen.length) return
    client.request("poll.vote", { chatId: message.chatId, messageId: message.id, optionIds: chosen }, function (answer) {
      if (!answer.ok) root.flash(answer.error || "Could not vote")
    })
    var next = root.copyOf(root.pollChoices)
    delete next[message.id]
    root.pollChoices = next
  }

function toggleReaction(context, message, reaction) {
  var client = context.client
  var root = context.root
    if (!message || !reaction || !reaction.emoji) return
    client.request("reaction.set", { chatId: message.chatId, messageId: message.id, emoji: reaction.emoji, chosen: !reaction.chosen },
                   function (answer) { if (!answer.ok) root.flash(answer.error || "Could not react") })
  }

function toggleSelected(context, message) {
  var root = context.root
    if (!message || root.scheduledOpen) return
    root.selection = Model.toggleSelection(root.selection, Model.albumIds(root.messages, message))
  }

function clearSelection(context) {
  var root = context.root; root.selection = ({}) }

function openMenu(context, message, x, y, selectedText) {
  var client = context.client
  var composer = context.composer
  var messageMenu = context.messageMenu
  var root = context.root
    if (!message || !root.chat || message.content.kind === "service") return
    root.menuToComposer = composer.activeFocus
    root.menuMessage = message
    root.menuSelectedText = selectedText || ""
    root.menuProperties = null
    root.menuReactions = []
    messageMenu.open(x, y)
    if (message.sending || message.sendAt) return   // not sent yet: there is nothing to ask
    var id = message.id
    client.request("message.properties", { chatId: message.chatId, messageId: id }, function (answer) {
      if (answer.ok && messageMenu.visible && root.menuMessage && root.menuMessage.id === id) root.menuProperties = answer.result
    })
    client.request("reactions.available", { chatId: message.chatId, messageId: id }, function (answer) {
      if (answer.ok && messageMenu.visible && root.menuMessage && root.menuMessage.id === id)
        root.menuReactions = (answer.result.emoji || []).slice(0, 8)
    })
  }

function openMenuAtCursor(context) {
  var Style = context.Style

  var messageList = context.messageList
  var root = context.root
    if (!root.selectedMessage) return
    var item = messageList.itemAtIndex(root.cursor)
    var at = item ? item.bubbleItem.mapToItem(root, Math.min(item.bubbleItem.width, Style.space(48)), Math.min(item.bubbleItem.height, Style.space(28)))
                  : Qt.point(root.width / 2, root.height / 2)
    root.openMenu(root.selectedMessage, at.x, at.y)
  }

function afterMenu(context) {
  var root = context.root
    if (root.menuToComposer) root.focusComposer()
    else root.focusMessages()
  }

function menuPicked(context, id) {
  var root = context.root
    var message = root.menuMessage
    if (!message || !root.chat) return
    if (id === "reply") root.startReply(message)
    else if (id === "thread") root.openThread(message)
    else if (id === "favoriteSticker") root.favoriteSticker(message)
    else if (id === "stickerSet") root.openStickerSet(message)
    else if (id === "moreReactions") root.openReactionPicker(message)
    else if (id === "reactions") root.showReactions(message)
    else if (id === "viewers") root.showViewers(message)
    else if (id === "copy") root.copyText(root.menuSelectedText || message.content.text)
    else if (id === "link") root.copyLink(message)
    else if (id === "edit") root.startEdit(root.captionHolder(message), true)
    else if (id === "forward") root.forward(Model.albumIds(root.messages, message))
    else if (id === "pin" || id === "unpin") root.setPinned(message, id === "pin")
    else if (id === "select") root.toggleSelected(message)
    else if (id === "open") root.openFile(message)
    else if (id === "save") root.saveFile(message)
    else if (id === "retract") root.retractVote(message)
    else if (id === "deleteAll" || id === "deleteMe") root.confirmDelete(Model.albumIds(root.messages, message), id === "deleteAll")
    else if (id === "translate") root.translate(root.captionHolder(message))
    else if (id === "untranslate") root.untranslate(root.captionHolder(message))
    else if (id === "sendNow") root.reschedule(message, 0)
    else if (id === "reschedule") root.openRescheduleMenu(message)
  }

function favoriteSticker(context, message) {
  var client = context.client
  var root = context.root
    var media = message && message.content ? message.content.media : null
    if (!media || !media.file) return
    client.request("sticker.favorite", { fileId: media.file.id, favorite: true }, function (answer) {
      root.flash(answer.ok ? "Added to your favorite stickers" : (answer.error || "Could not add it to your favorites"))
    })
  }

function openStickerSet(context, message) {
  var root = context.root
  var stickerPicker = context.stickerPicker
    var media = message && message.content ? message.content.media : null
    if (!media || !media.setId || !root.chat) return
    root.stickersOpen = true
    Qt.callLater(function () { stickerPicker.openSet(media.setId) })
  }

function react(context, message, emoji) {
  var client = context.client
  var root = context.root
    if (!message || !emoji) return
    client.request("reaction.set", { chatId: message.chatId, messageId: message.id, emoji: emoji, chosen: !Model.reactionChosen(message, emoji) },
                   function (answer) { if (!answer.ok) root.flash(answer.error || "Could not react") })
  }

function retractVote(context, message) {
  var client = context.client
  var root = context.root
    client.request("poll.vote", { chatId: message.chatId, messageId: message.id, optionIds: [] }, function (answer) {
      if (!answer.ok) root.flash(answer.error || "Could not retract the vote")
    })
  }

function captionHolder(context, message) {
  var root = context.root
    if (!message.albumId) return message
    for (var i = 0; i < root.messages.length; i++) {
      var m = root.messages[i]
      if (m.albumId === message.albumId && m.content.text) return m
    }
    return message
  }

function copyText(context, text, done) {
  var clipboard = context.clipboard
  var root = context.root
    clipboard.text = text
    clipboard.selectAll()
    clipboard.copy()
    root.flash(done || "Copied")
  }

function copyLink(context, message) {
  var client = context.client
  var root = context.root
    if (!message) return
    client.request("message.link", { chatId: message.chatId, messageId: message.id }, function (answer) {
      if (answer.ok && answer.result.link)
        root.copyText(answer.result.link, answer.result.public ? "Link copied" : "Link copied: it opens only for members of this chat")
      else root.flash(answer.error || "This message has no link")
    })
  }
