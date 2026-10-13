.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function startEditing(context, row) {
  var editor = context.editor
  var settings = context.settings
    if (!settings.profile) { settings.error = "Your profile is still loading"; return }
    settings.error = ""
    settings.confirm = null
    settings.editing = { field: row.field, label: row.label }
    editor.text = settings.profile[row.field] || ""
    editor.forceActiveFocus()
  }

function cancelEditing(context) {
  var editor = context.editor
  var list = context.list
  var settings = context.settings
    settings.flowSerial++
    settings.editing = null
    settings.flow = null          // with what was typed for two-step verification
    settings.editorError = ""
    editor.text = ""
    list.forceActiveFocus()
  }

function saveEditing(context) {
  var app = context.app
  var editor = context.editor
  var settings = context.settings
    if (settings.flow) { settings.flowNext(); return }
    if (settings.editing && settings.editing.field === "folderName") {
      var folderName = editor.text.trim()
      if (Model.profileProblem("folderName", folderName) !== "") return   // the field shows what is wrong
      settings.changeFolder({ name: folderName })
      settings.cancelEditing()
      return
    }
    var editing = settings.editing
    var profile = settings.profile
    if (!editing || !profile) return
    var text = editor.text.trim()
    if (Model.profileProblem(editing.field, text) !== "") return   // the field shows what is wrong
    var command = "profile.setBio"
    var args = { bio: text }
    if (editing.field === "firstName" || editing.field === "lastName") {
      command = "profile.setName"
      args = { firstName: editing.field === "firstName" ? text : profile.firstName,
               lastName: editing.field === "lastName" ? text : profile.lastName }
    } else if (editing.field === "username") {
      text = text.replace(/^@/, "")
      command = "profile.setUsername"
      args = { username: text }
    }
    settings.cancelEditing()
    if (text === (profile[editing.field] || "")) return
    settings.app.request(command, args, function (answer) {
      var problem = answer.ok ? "" : Model.profileError(answer.error)
      if (problem !== "") settings.error = problem
      settings.loadProfile()
    })
  }

function urlToPath(context, url) {
    var s = String(url)
    return s.indexOf("file://") === 0 ? decodeURIComponent(s.slice(7)) : ""
  }

function setPhoto(context, path) {
  var app = context.app
  var settings = context.settings
    settings.forceActiveFocus()
    if (!path) return
    settings.error = ""
    settings.photoBusy = true
    settings.app.request("profile.setPhoto", { path: path }, function (answer) {
      settings.photoBusy = false
      if (!answer.ok) settings.error = Model.profileError(answer.error)
      settings.loadProfile()
    })
  }

function askRemovePhoto(context) {
  var app = context.app
  var settings = context.settings
    if (!settings.profile || !(settings.profile.photo || settings.profile.photoId)) return
    settings.ask("Remove your profile photo? If you had earlier ones, the one before it shows instead.", function () {
      settings.app.request("profile.deletePhoto", {}, function (answer) {
        if (!answer.ok) settings.error = Model.profileError(answer.error)
        settings.loadProfile()
      })
    })
  }
