.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function loadPrivacy(context) {
  var app = context.app
  var settings = context.settings
    settings.app.request("privacy.get", {}, function (answer) { if (answer.ok) settings.privacy = answer.result.settings || ({}) })
    settings.app.request("password.get", {}, function (answer) { if (answer.ok) settings.password = answer.result })
    settings.app.request("account.ttl", {}, function (answer) { if (answer.ok) settings.accountTtl = answer.result.days || 0 })
    settings.app.request("autoDelete.default", {}, function (answer) { if (answer.ok) settings.defaultAutoDelete = answer.result.seconds })
    settings.loadBlocked()
  }

function loadBlocked(context) {
  var app = context.app
  var settings = context.settings
    settings.app.request("blocked.list", {}, function (answer) { if (answer.ok) settings.blocked = answer.result })
  }

function changePrivacy(context, row) {
  var app = context.app
  var settings = context.settings
    var current = settings.privacy[row.id]
    if (!current) { settings.error = "Telegram has not said who can see that yet"; return }
    settings.error = ""
    settings.app.request("privacy.set", { setting: row.id, base: Model.nextPrivacy(row.id, current.base) }, function (answer) {
      if (!answer.ok) { settings.error = answer.error || "Telegram did not take the change"; return }
      var next = {}
      for (var k in settings.privacy) next[k] = settings.privacy[k]
      next[row.id] = answer.result
      settings.privacy = next
    })
  }

function askUnblock(context, sender) {
  var app = context.app
  var settings = context.settings
    settings.ask("Unblock " + sender.name + "?", function () {
      settings.app.request("blocked.unblock", { type: sender.type, id: sender.id }, function (answer) {
        if (!answer.ok) settings.error = answer.error || "Could not unblock"
        settings.loadBlocked()
      })
    })
  }

function changeAccountTtl(context) {
  var app = context.app
  var settings = context.settings
    settings.app.request("account.setTtl", { days: Model.nextTtl(settings.accountTtl) }, function (answer) {
      if (answer.ok) settings.accountTtl = answer.result.days
      else settings.error = answer.error || "Telegram did not take the change"
    })
  }

function changeDefaultAutoDelete(context) {
  var app = context.app
  var settings = context.settings
    if (settings.defaultAutoDelete < 0) return
    settings.app.request("autoDelete.setDefault", { seconds: Model.nextAutoDelete(settings.defaultAutoDelete) }, function (answer) {
      if (answer.ok) settings.defaultAutoDelete = answer.result.seconds
      else settings.error = answer.error || "Telegram did not take the change"
    })
  }

function startPasswordFlow(context, kind) {
  var settings = context.settings
    settings.error = ""
    settings.confirm = null
    settings.flowSerial++
    settings.flow = { kind: kind, steps: Model.passwordSteps(kind), step: 0, values: {} }
    settings.showFlowStep()
  }

function showFlowStep(context) {
  var editor = context.editor
  var settings = context.settings
    var step = settings.flow.steps[settings.flow.step]
    settings.editorError = ""
    settings.editing = { field: step.key, label: step.label, secret: step.secret === true, placeholder: step.placeholder || "" }
    editor.text = ""
    editor.forceActiveFocus()
  }

function flowNext(context) {
  var app = context.app
  var editor = context.editor
  var settings = context.settings
    var flow = settings.flow
    var step = flow.steps[flow.step]
    var problem = flow.kind === "proxy" ? Model.proxyStepProblem(step, editor.text) : Model.passwordStepProblem(step, editor.text, flow.values)
    if (problem !== "") { settings.editorError = problem; return }
    flow.values[step.key] = step.secret ? editor.text : editor.text.trim()
    if (flow.step + 1 < flow.steps.length) {
      flow.step++
      settings.showFlowStep()
      return
    }
    var values = flow.values
    var kind = flow.kind
    settings.cancelEditing()
    var serial = settings.flowSerial
    var answered = function (answer) { settings.passwordAnswered(answer, serial) }
    if (kind === "proxy") {
      settings.addProxy(flow.proxyType, values)
      return
    }
    if (kind === "code") {
      settings.app.request("password.checkEmailCode", { code: values.code }, answered)
      return
    }
    var args = { oldPassword: values.oldPassword || "", newPassword: kind === "off" ? "" : values.newPassword, hint: values.hint || "" }
    if (values.email) args.email = values.email
    settings.app.request("password.set", args, answered)
  }

function passwordAnswered(context, answer, serial) {
  var settings = context.settings
    if (answer.ok) settings.password = answer.result
    if (serial !== settings.flowSerial) return
    if (!answer.ok) { settings.error = Model.passwordError(answer.error); return }
    if (answer.result.emailCodePattern) settings.startPasswordFlow("code")
  }
