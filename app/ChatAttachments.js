.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function openMoreMenu(context, item) {
  var Style = context.Style

  var composer = context.composer
  var moreMenu = context.moreMenu
  var root = context.root
    if (!root.chat || !root.canWrite) return
    var at = item ? item.mapToItem(root, 0, 0) : composer.mapToItem(root, composer.cursorRectangle.x, composer.cursorRectangle.y)
    moreMenu.open(at.x, at.y - Style.space(4))
  }

function morePicked(context, id) {
  var Style = context.Style

  var app = context.app
  var composer = context.composer
  var diceMenu = context.diceMenu
  var peoplePicker = context.peoplePicker
  var pollComposer = context.pollComposer
  var root = context.root
    if (!root.chat) return
    if (id === "poll") {
      pollComposer.open(Model.chatTitle(root.chat, app.meId), root.chat.kind === "channel")
    } else if (id === "dice") {
      var at = composer.mapToItem(root, composer.cursorRectangle.x, composer.cursorRectangle.y)
      diceMenu.open(at.x, at.y - Style.space(4))
    } else if (id === "contact") {
      peoplePicker.open(0, [])
    } else if (id === "location") {
      root.openLocationBar()
    }
  }

function sendExtra(context, command, args, what) {
  var client = context.client
  var root = context.root
  var videoNote = context.videoNote
    if (!root.chat || root.sendPending || videoNote.visible) return
    args.chatId = root.chat.id
    root.target(args)
    if (root.replyToId) args.replyToMessageId = root.replyToId
    var snapshot = root.beginSend()
    snapshot.failurePrefix = "Could not send " + what + ": "
    client.request(command, args, function (answer) {
      root.completeSend(snapshot, answer, 0, true)
    })
  }

function sendDice(context, emoji) {
  var root = context.root
    root.sendExtra("message.sendDice", { emoji: emoji }, "the dice")
  }

function sendPoll(context, poll) {
  var root = context.root
    root.focusComposer()
    root.sendExtra("message.sendPoll", { question: poll.question, options: poll.options, anonymous: poll.anonymous,
                                         multiple: poll.multiple, quiz: poll.quiz, correct: poll.correct,
                                         explanation: poll.explanation }, "the poll")
  }

function shareContact(context, chatId) {
  var app = context.app
  var root = context.root
    root.focusComposer()
    var person = Model.findChat(app.chats || [], chatId)
    if (person && person.userId) root.sendExtra("message.sendContact", { userId: person.userId }, "the contact card")
  }

function openDateBar(context) {
  var dateField = context.dateField
  var root = context.root
    if (!root.chat) return
    root.locationOpen = false
    dateField.text = ""
    root.dateOpen = true
    dateField.forceActiveFocus()
  }

function closeDateBar(context) {
  var dateField = context.dateField
  var root = context.root
    root.dateOpen = false
    dateField.text = ""
    root.focusComposer()
  }

function jumpToDay(context) {
  var client = context.client
  var dateField = context.dateField
  var root = context.root
    var start = Model.parseDay(dateField.text, root.nowMs)
    if (start === null || !root.chat) return
    var chatId = root.chat.id
    client.request("chat.messageByDate", { chatId: chatId, date: start + 86399 }, function (answer) {
      if (!root.chat || root.chat.id !== chatId) return
      if (answer.ok && answer.result.messageId) root.jumpTo(answer.result.messageId)
      else root.flash("No messages on or before that day")
    })
    root.closeDateBar()
  }

function openLocationBar(context) {
  var locationField = context.locationField
  var root = context.root
    locationField.text = ""
    root.dateOpen = false
    root.locationOpen = true
    locationField.forceActiveFocus()
  }

function closeLocationBar(context) {
  var locationField = context.locationField
  var root = context.root
    root.locationOpen = false
    locationField.text = ""
    root.focusComposer()
  }

function sendLocation(context) {
  var locationField = context.locationField
  var root = context.root
    var place = Model.parseLocation(locationField.text)
    if (!place) return
    root.sendExtra("message.sendLocation", { latitude: place.latitude, longitude: place.longitude }, "the location")
    root.closeLocationBar()
  }

function attach(context, asPhoto) {
  var attachDialog = context.attachDialog
  var root = context.root
    if (!root.chat || !root.canWrite) return
    attachDialog.asPhoto = asPhoto
    attachDialog.open()
  }

function urlToPath(context, url) {
    var s = String(url)
    return s.indexOf("file://") === 0 ? decodeURIComponent(s.slice(7)) : ""
  }

function addAttachments(context, paths, asMedia) {
  var root = context.root
    if (!root.chat || !paths.length || !root.canWrite) return
    if (!root.attachments.length) root.attachAsMedia = asMedia !== false
    var next = root.attachments.slice()
    for (var i = 0; i < paths.length; i++) {
      if (next.some(function (a) { return a.path === paths[i] })) continue
      if (next.length >= 10) {
        root.flash("Ten files go at once: the rest were left out")
        break
      }
      next.push({ path: paths[i], name: paths[i].split("/").pop(), kind: Model.attachmentKind(paths[i]) })
    }
    root.attachments = next
    root.focusComposer()
  }

function removeAttachment(context, index) {
  var root = context.root
    var next = root.attachments.slice()
    next.splice(index, 1)
    root.attachments = next
  }

function sendAttachments(context, options) {
  var client = context.client
  var composer = context.composer
  var root = context.root
  var videoNote = context.videoNote
    if (!root.chat || root.sendPending || videoNote.visible) return
    var caption = composer.text.replace(/\s+$/, "")
    if (caption.length > 2048) { root.flash("That caption is too long."); return }
    var args = root.target({ chatId: root.chat.id, paths: root.attachments.map(function (a) { return a.path }),
                             asMedia: root.attachAsMedia, caption: caption })
    if (root.replyToId) args.replyToMessageId = root.replyToId
    if (options && typeof options.silent === "boolean") args.silent = options.silent
    if (options && options.sendAt) args.sendAt = options.sendAt
    var snapshot = root.beginSend()
    client.request("message.sendFiles", args, function (answer) {
      root.completeSend(snapshot, answer, options && options.sendAt ? options.sendAt : 0)
    })
  }

function toggleStickers(context) {
  var root = context.root
  var stickerPicker = context.stickerPicker
    if (!root.chat || !root.canWrite) return
    root.stickersOpen = !root.stickersOpen
    if (root.stickersOpen) Qt.callLater(function () { stickerPicker.open() })
    else root.focusComposer()
  }

function startVoice(context) {
  var app = context.app
  var client = context.client
  var root = context.root
  var videoNote = context.videoNote
    if (!root.chat || root.recordingVoice || !root.canWrite || videoNote.visible || app.recordedSend) return
    root.recordingNow = Date.now()
    client.request("voice.start", { chatId: root.chat.id }, function (answer) {
      if (!answer.ok) root.flash("Could not record: " + (answer.error || "no microphone"))
    })
  }

function stopVoice(context, send) {
  var client = context.client
  var root = context.root
  var videoNote = context.videoNote
    if (send && (!root.chat || root.sendPending || videoNote.visible)) return
    var args = root.target({ send: send })
    if (send && root.replyToId) args.replyToMessageId = root.replyToId
    var snapshot = send ? root.beginSend() : null
    client.request("voice.stop", args, function (answer) {
      if (snapshot) root.completeSend(snapshot, answer, 0, true)
      else if (!answer.ok) root.flash(answer.error || "Could not discard the voice message")
    })
    Qt.callLater(root.focusComposer)
  }

function composerAction(context, action, item) {
  var Style = context.Style

  var composerButtons = context.composerButtons
  var composerMenu = context.composerMenu
  var root = context.root
    if (!root.chat || !root.canWrite) return
    if (action === "overflow") {
      var at = (item || composerButtons).mapToItem(root, 0, 0)
      composerMenu.open(at.x, at.y - Style.space(4))
    }
    else if (action === "silent") root.toggleSilent()
    else if (action === "send") root.send()
    else if (action === "later") root.openSendMenu(item)
    else if (action === "attach") root.attach(true)
    else if (action === "emoji") root.openEmoji()
    else if (action === "stickers") root.toggleStickers()
    else if (action === "video") root.openVideo()
    else if (action === "voice") root.startVoice()
    else if (action === "more") root.openMoreMenu(item)
  }
