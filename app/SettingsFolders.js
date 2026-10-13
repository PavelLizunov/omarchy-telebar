.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function loadNotifications(context) {
  var app = context.app
  var settings = context.settings
    settings.app.request("notifications.get", {}, function (answer) { if (answer.ok) settings.scopes = answer.result.scopes || ({}) })
  }

function changeScope(context, scope, change) {
  var app = context.app
  var settings = context.settings
    if (!settings.scopes[scope]) { settings.error = "Telegram has not said these notifications yet"; return }
    settings.error = ""
    var args = { scope: scope }
    for (var key in change) args[key] = change[key]
    settings.app.request("notifications.set", args, function (answer) {
      if (!answer.ok) { settings.error = answer.error || "Telegram did not take the change"; return }
      var next = {}
      for (var s in settings.scopes) next[s] = settings.scopes[s]
      next[scope] = { muted: answer.result.muted, preview: answer.result.preview }
      settings.scopes = next
    })
  }

function togglePreviews(context) {
  var settings = context.settings
    var show = Model.previewsText(settings.scopes) !== "Shown"
    var all = ["private", "groups", "channels"]
    for (var i = 0; i < all.length; i++) settings.changeScope(all[i], { preview: show })
  }

function loadFolders(context) {
  var app = context.app
  var settings = context.settings
    settings.app.request("folders.get", {}, function (answer) { if (answer.ok) settings.folders = answer.result.folders || [] })
  }

function folderRows(context) {
  var list = context.list
  var settings = context.settings
    var f = settings.folderOpen
    var out = [{ kind: "header", title: f.id ? "Folder “" + f.name + "”" : "New folder", note: "Enter changes a row  ·  Esc goes back without saving" },
               { kind: "folderName", label: "Name" },
               { kind: "header", title: "The chats it takes", note: "" }]
    for (var k = 0; k < Model.FOLDER_KINDS.length; k++) out.push({ kind: "folderFlag", key: Model.FOLDER_KINDS[k][0], label: Model.FOLDER_KINDS[k][1] })
    out.push({ kind: "header", title: "What it leaves out", note: "" })
    var leaves = [["excludeMuted", "Muted chats"], ["excludeRead", "Read chats"], ["excludeArchived", "Archived chats"]]
    for (var l = 0; l < leaves.length; l++) out.push({ kind: "folderFlag", key: leaves[l][0], label: leaves[l][1] })
    var lists = [["included", "Always in it", "Enter on a chat takes it out"], ["excluded", "Never in it", "Enter on a chat takes it off this list"]]
    for (var i = 0; i < lists.length; i++) {
      out.push({ kind: "header", title: lists[i][1], note: lists[i][2] })
      var ids = Model.toList(f[lists[i][0]])
      for (var j = 0; j < ids.length; j++) out.push({ kind: "folderChat", list: lists[i][0], chatId: ids[j] })
      out.push({ kind: "folderAdd", list: lists[i][0], label: "Add a chat…" })
    }
    out.push({ kind: "header", title: "", note: "" }, { kind: "folderSave", label: f.id ? "Save the folder" : "Create the folder" })
    if (f.id) out.push({ kind: "folderDelete", label: "Delete the folder" })
    return out
  }

function openFolder(context, folder) {
  var list = context.list
  var settings = context.settings
    settings.folderSerial++
    settings.cancelEditing()
    settings.error = ""
    var copy = Model.newFolder()
    if (folder) for (var key in copy) if (folder[key] !== undefined) copy[key] = Model.isList(folder[key]) ? Model.toList(folder[key]).slice() : folder[key]
    settings.folderOpen = copy
    settings.cursor = 1
    list.positionViewAtBeginning()
  }

function closeFolder(context) {
  var settings = context.settings
    settings.folderSerial++
    settings.cancelEditing()
    settings.folderOpen = null
    settings.cursor = 1
  }

function changeFolder(context, change) {
  var settings = context.settings
    var next = {}
    for (var key in settings.folderOpen) next[key] = settings.folderOpen[key]
    for (var c in change) next[c] = change[c]
    settings.folderOpen = next
  }

function startEditingFolderName(context) {
  var editor = context.editor
  var settings = context.settings
    settings.error = ""
    settings.editing = { field: "folderName", label: "The folder's name, up to 12 characters" }
    editor.text = settings.folderOpen.name || ""
    editor.forceActiveFocus()
  }

function startAddingChat(context, listName) {
  var chatPicker = context.chatPicker
  var settings = context.settings
    settings.pickerFor = listName
    chatPicker.open(0, [])
  }

function addFolderChat(context, chatId) {
  var settings = context.settings
    var f = settings.folderOpen
    if (!f || !settings.pickerFor) return
    var other = settings.pickerFor === "included" ? "excluded" : "included"
    var change = {}
    change[settings.pickerFor] = Model.toList(f[settings.pickerFor]).filter(function (id) { return id !== chatId }).concat([chatId])
    change[other] = Model.toList(f[other]).filter(function (id) { return id !== chatId })
    settings.changeFolder(change)
    settings.forceActiveFocus()
  }

function removeFolderChat(context, listName, chatId) {
  var settings = context.settings
    var change = {}
    change[listName] = Model.toList(settings.folderOpen[listName]).filter(function (id) { return id !== chatId })
    if (listName === "included") change.pinned = Model.toList(settings.folderOpen.pinned).filter(function (id) { return id !== chatId })
    settings.changeFolder(change)
  }

function saveFolder(context) {
  var app = context.app
  var settings = context.settings
    var f = settings.folderOpen
    var problem = Model.folderProblem(f)
    if (problem !== "") { settings.error = problem; return }
    var args = {}
    for (var key in f) if (key !== "id" && key !== "icon") args[key] = f[key]
    args.name = String(f.name).trim()
    if (f.icon) args.icon = f.icon
    if (f.id) args.id = f.id
    var serial = settings.folderSerial
    settings.app.request("folder.save", args, function (answer) {
      if (answer.ok) settings.loadFolders()
      if (serial !== settings.folderSerial) return
      if (!answer.ok) { settings.error = answer.error || "Telegram did not take the folder"; return }
      settings.closeFolder()
    })
  }

function askDeleteFolder(context) {
  var app = context.app
  var settings = context.settings
    var f = settings.folderOpen
    if (!f || !f.id) return
    var serial = settings.folderSerial
    settings.ask("Delete the folder “" + f.name + "”? The chats in it stay where they are.", function () {
      settings.app.request("folder.delete", { id: f.id }, function (answer) {
        if (answer.ok) settings.loadFolders()
        if (serial !== settings.folderSerial) return
        if (!answer.ok) { settings.error = answer.error || "Could not delete the folder"; return }
        settings.closeFolder()
      })
    })
  }

function moveFolder(context, folderId, delta) {
  var app = context.app
  var settings = context.settings
    var ids = settings.folders.map(function (f) { return f.id })
    var moved = Model.movedFolders(ids, folderId, delta)
    if (moved.join(",") === ids.join(",")) return
    settings.app.request("folders.reorder", { ids: moved }, function (answer) {
      if (!answer.ok) settings.error = answer.error || "Could not move the folder"
      settings.loadFolders()
    })
  }
