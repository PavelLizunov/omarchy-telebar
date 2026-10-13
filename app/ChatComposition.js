.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function setComposerText(context, text) {
  var composer = context.composer
  var root = context.root
    root.settingText = true
    composer.text = text
    root.settingText = false
  }

function saveDraft(context) {
  var client = context.client
  var composer = context.composer
  var root = context.root
    if (!root.draftChatId) return
    var text = (root.editingId ? root.draftBeforeEdit : composer.text).replace(/\s+$/, "").slice(0, 4096)
    if (text === root.savedDraft) return
    root.savedDraft = text
    var args = { chatId: root.draftChatId, text: text }
    if (root.draftTopicId) args[root.draftThread ? "threadId" : "topicId"] = root.draftTopicId
    if (root.replyToId && !root.editingId) args.replyToMessageId = root.replyToId
    client.request("chat.draft", args)
  }

function composerEdited(context) {
  var client = context.client
  var composer = context.composer
  var draftTimer = context.draftTimer
  var previewLater = context.previewLater
  var root = context.root
  var suggestLater = context.suggestLater
    root.compositionRevision++
    suggestLater.restart()
    previewLater.restart()
    if (root.settingText || !root.chat) return
    draftTimer.restart()
    if (root.editingId) return
    if (composer.text === "") {
      if (root.lastTypingMs) client.request("chat.action", root.target({ chatId: root.chat.id, action: "cancel" }))
      root.lastTypingMs = 0
    } else if (Date.now() - root.lastTypingMs > 4500) {   // Telegram shows an action for about five seconds
      root.lastTypingMs = Date.now()
      client.request("chat.action", root.target({ chatId: root.chat.id, action: "typing" }))
    }
  }

function lookUpLinkPreview(context) {
  var client = context.client
  var composer = context.composer
  var root = context.root
    var link = root.editingId || root.attachments.length ? "" : Model.composerLink(composer.text)
    if (link === root.linkPreviewLink) return
    root.linkPreviewLink = link
    root.linkPreview = null
    if (!link) { root.linkPreviewMode = "below"; return }
    client.request("message.linkPreview", { text: composer.text.slice(0, 8192) }, function (answer) {
      if (root.linkPreviewLink === link) root.linkPreview = answer.ok ? answer.result.preview : null
    })
  }

function cycleLinkPreview(context) {
  var root = context.root
    if (!root.linkPreview && root.linkPreviewMode === "below") return
    root.linkPreviewMode = ({ below: "above", above: "none", none: "below" })[root.linkPreviewMode]
  }

function forgetLinkPreview(context) {
  var previewLater = context.previewLater
  var root = context.root
    previewLater.stop()
    root.linkPreview = null
    root.linkPreviewLink = ""
    root.linkPreviewMode = "below"
  }

function leaveChat(context) {
  var client = context.client
  var draftTimer = context.draftTimer
  var root = context.root
    draftTimer.stop()
    root.saveDraft()
    if (root.lastTypingMs && root.draftChatId) {
      var args = { chatId: root.draftChatId, action: "cancel" }
      if (root.draftTopicId) args[root.draftThread ? "threadId" : "topicId"] = root.draftTopicId
      client.request("chat.action", args)
    }
    root.lastTypingMs = 0
  }

function leaveAccount(context) {
  var root = context.root
  var videoNote = context.videoNote
    root.invalidateSend()
    root.invalidateMention()
    root.leaveChat()
    if (root.recordingVoice) root.stopVoice(false)
    videoNote.finish(false)
  }

function flash(context, text) {
  var noticeTimer = context.noticeTimer
  var root = context.root
    root.notice = text
    noticeTimer.restart()
  }

function target(context, args) {
  var root = context.root
    if (root.topicId) args[root.threadOpen ? "threadId" : "topicId"] = root.topicId
    return args
  }

function invalidateSend(context) {
  var root = context.root
    root.sendSerial++
    root.sendPending = false
  }

function sendContext(context) {
  var app = context.app
  var root = context.root
    return { revision: root.compositionRevision, chatId: root.chat.id,
             topicId: root.topicId, thread: root.threadOpen, account: app.activeAccount,
             editingId: root.editingId, scheduled: root.scheduledOpen, replyToId: root.replyToId }
  }

function beginSend(context) {
  var root = context.root
    root.sendPending = true
    var snapshot = root.sendContext()
    snapshot.serial = ++root.sendSerial
    return snapshot
  }

function completeSend(context, snapshot, answer, later, replyOnly) {
  var app = context.app
  var draftTimer = context.draftTimer
  var root = context.root
    if (snapshot.serial !== root.sendSerial) return
    root.sendSerial++
    root.sendPending = false
    if (!root.chat || root.chat.id !== snapshot.chatId || root.topicId !== snapshot.topicId
        || root.threadOpen !== snapshot.thread || app.activeAccount !== snapshot.account) return
    if (!answer.ok) {
      root.flash((snapshot.failurePrefix || (snapshot.editingId && !replyOnly ? "Could not edit: " : "Could not send: ")) + (answer.error || "unknown error"))
      return
    }
    if (replyOnly) {
      if (snapshot.revision === root.compositionRevision) {
        root.replyToId = 0
        root.stickToBottom = true
      }
      return
    }
    if (snapshot.editingId) {
      if (snapshot.revision === root.compositionRevision && root.editingId === snapshot.editingId) {
        root.finishEdit()
        if (snapshot.scheduled && root.scheduledOpen) root.focusMessages()
      }
      if (snapshot.scheduled && root.scheduledOpen) root.loadScheduled()
      return
    }
    if (snapshot.revision === root.compositionRevision) {
      root.attachments = []
      draftTimer.stop()
      root.savedDraft = ""
      root.lastTypingMs = 0
      root.setComposerText("")
      root.replyToId = 0
      root.forgetLinkPreview()
      root.stickToBottom = true
    }
    if (later) root.flash("It will be sent " + Model.scheduleText(later, Date.now()))
  }

function send(context, options) {
  var client = context.client
  var composer = context.composer
  var root = context.root
  var videoNote = context.videoNote
    var text = composer.text.replace(/\s+$/, "")
    if (!root.chat || root.sendPending || videoNote.visible) return
    if (root.editingId) {
      if (!root.editingCaption && !text.trim()) return
      // The service checks the length once the formatting markers are read.
      if (text.length > (root.editingCaption ? 2048 : 8192)) { root.flash((root.editingCaption ? "That caption" : "That message") + " is too long."); return }
      var editSnapshot = root.beginSend()
      client.request("message.edit", { chatId: root.chat.id, messageId: root.editingId, text: text, caption: root.editingCaption }, function (answer) {
        root.completeSend(editSnapshot, answer, 0)
      })
      return
    }
    if (root.attachments.length) {
      root.sendAttachments(options)
      return
    }
    if (!text.trim()) return
    if (text.length > 8192) { root.flash("That message is too long."); return }
    var args = root.target({ chatId: root.chat.id, text: text })
    if (root.replyToId) args.replyToMessageId = root.replyToId
    if (root.linkPreviewMode !== "below") args.linkPreview = root.linkPreviewMode
    var later = options && options.sendAt ? options.sendAt : 0
    if (options && typeof options.silent === "boolean") args.silent = options.silent
    if (later) args.sendAt = later
    var snapshot = root.beginSend()
    client.request("message.send", args, function (answer) {
      root.completeSend(snapshot, answer, later)
    })
  }

function startReply(context, message) {
  var root = context.root
    if (!message) return
    if (root.editingId) root.finishEdit()
    root.replyToId = message.id
    root.focusComposer()
  }

function startEdit(context, message, allowed) {
  var app = context.app
  var client = context.client
  var composer = context.composer
  var root = context.root
    if (!message || !message.content || (!allowed && !message.outgoing)) return
    var kind = message.content.kind
    if (kind !== "text" && Model.CAPTION_KINDS.indexOf(kind) < 0) return
    if (!root.editingId) root.draftBeforeEdit = composer.text
    root.replyToId = 0
    root.editingId = message.id
    root.editingCaption = kind !== "text"
    root.setComposerText(message.content.text || "")
    root.focusComposer()
    composer.cursorPosition = composer.length
    // Its formatting comes back as the Markdown it can be typed in, once the service has written it.
    var id = message.id
    var plain = composer.text
    var revision = root.compositionRevision
    var account = app.activeAccount
    var chatId = root.chat ? root.chat.id : 0
    var topic = root.topicId
    var thread = root.threadOpen
    if (message.sending || !(message.content.entities || []).length) return
    client.request("message.markdown", { chatId: message.chatId, messageId: id }, function (answer) {
      if (!answer.ok || root.editingId !== id || composer.text !== plain || root.compositionRevision !== revision
          || app.activeAccount !== account || !root.chat || root.chat.id !== chatId
          || root.topicId !== topic || root.threadOpen !== thread) return
      root.setComposerText(answer.result.text)
      composer.cursorPosition = composer.length
    })
  }

function wrapSelection(context, before, after) {
  var composer = context.composer
    var a = composer.selectionStart
    var b = composer.selectionEnd
    var r = Model.markdownToggle(composer.text, a, b, before, after)
    if (r.wrapped) {
      composer.insert(b, after)
      composer.insert(a, before)
    } else {
      composer.remove(b, b + after.length)
      composer.remove(a - before.length, a)
    }
    composer.select(r.start, r.end)
    return r
  }

function wrapLink(context) {
  var composer = context.composer
  var root = context.root
    var hadText = composer.selectionEnd > composer.selectionStart
    var r = root.wrapSelection("[", "]()")
    if (r.wrapped && hadText) composer.cursorPosition = r.end + 2
  }
