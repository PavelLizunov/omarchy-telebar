.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function fileState(context, file) {
  var omagram = context.omagram
    omagram.filesRevision
    if (!file) return null
    return omagram.files[file.id] || file
  }

function setFile(context, view) {
  var omagram = context.omagram
    if (!view || !view.id) return
    if (!omagram.files[view.id] && Object.keys(omagram.files).length >= 4000) {
      // Bounded: only downloads still running are kept; the rest come back from their messages.
      var kept = {}
      for (var id in omagram.files) if (omagram.files[id].active) kept[id] = omagram.files[id]
      omagram.files = kept
    }
    omagram.files[view.id] = view
    omagram.filesRevision++
  }

function download(context, fileId, priority) {
  var omagram = context.omagram
  var service = context.service
    service.request("file.download", { fileId: fileId, priority: priority || 16 }, function (answer) {
      if (answer.ok) omagram.setFile(answer.result)
    })
  }

function lottiePath(context, fileId, callback) {
  var omagram = context.omagram
  var service = context.service
    if (omagram.lottieCache[fileId]) { callback(omagram.lottieCache[fileId]); return }
    service.request("sticker.lottie", { fileId: fileId }, function (answer) {
      var path = answer.ok && answer.result && answer.result.path ? answer.result.path : ""
      if (path) {
        if (Object.keys(omagram.lottieCache).length >= 2000) omagram.lottieCache = ({})
        omagram.lottieCache[fileId] = path
      }
      callback(path)
    })
  }

function customEmojiImages(context, ids) {
  var omagram = context.omagram
    omagram.customEmojiRevision
    omagram.filesRevision
    var images = {}
    for (var i = 0; i < ids.length; i++) {
      var still = Model.stillStickerFile(omagram.customEmoji[ids[i]])
      var file = still ? omagram.fileState(still) : null
      if (file && file.path) images[ids[i]] = Model.fileUrl(file.path)
    }
    return images
  }

function requestCustomEmoji(context, ids) {
  var omagram = context.omagram
  var service = context.service
    if (omagram.auth.state !== "ready") return
    if (Object.keys(omagram.customEmoji).length > 3000) omagram.customEmoji = ({})
    var missing = ids.filter(function (id) { return !(id in omagram.customEmoji) }).slice(0, 200)
    if (!missing.length) return
    missing.forEach(function (id) { omagram.customEmoji[id] = null })
    service.request("customEmoji.get", { ids: missing }, function (answer) {
      var stickers = answer.ok ? answer.result.emoji || [] : []
      stickers.forEach(function (sticker) {
        omagram.customEmoji[sticker.id] = sticker
        var still = Model.stillStickerFile(sticker)
        if (still && !still.path) omagram.download(still.id, 1)
      })
      omagram.customEmojiRevision++
    })
  }

function declineCall(context, callId) {
  var screen = context.screen
  var service = context.service
    service.request("call.decline", { callId: callId }, function (answer) {
      if (!answer.ok && screen.item && screen.item.notify) screen.item.notify(answer.error || "Could not decline the call")
    })
  }

function openStories(context, chatId) {
  var omagram = context.omagram
  var storyViewer = context.storyViewer
    if (!omagram.showStories) return
    var list = omagram.storyChats
    var active = chatId ? Model.findStories(list, chatId) : (list.filter(Model.storiesUnread)[0] || list[0])
    if (active) storyViewer.show(active.chatId, Model.firstStoryId(active))
  }

function messagesFor(context, chatId) {
  var omagram = context.omagram
    omagram.messagesRevision
    return omagram.messages[chatId === omagram.openChatId ? omagram.openKey : Model.historyKey(chatId, 0)] || []
  }

function setMessages(context, key, list) {
  var omagram = context.omagram
    omagram.messages[key] = list
    omagram.messagesRevision++
  }
