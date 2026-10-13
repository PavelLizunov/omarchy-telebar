.pragma library
.import "Model.js" as Model
.import "Keymap.js" as Keymap

function loadProxies(context) {
  var app = context.app
  var settings = context.settings
    settings.app.request("bridge.status", {}, function (answer) { if (answer.ok) settings.bridge = answer.result })
    settings.app.request("proxies.list", {}, function (answer) {
      if (!answer.ok) return
      settings.proxies = answer.result.proxies || []
      settings.proxies.forEach(function (p) { settings.pingProxy(p.id) })
    })
  }

function toggleBridge(context) {
  var app = context.app
  var settings = context.settings
    if (settings.bridgeBusy) return
    settings.bridgeBusy = true
    settings.app.request("bridge.set", { enabled: !settings.bridge.enabled }, function (answer) {
      settings.bridgeBusy = false
      if (!answer.ok) settings.error = answer.error || "Could not change the built-in bridge"
      settings.loadProxies()
    })
  }

function pingProxy(context, id) {
  var app = context.app
  var settings = context.settings
    settings.app.request("proxy.ping", { id: id }, function (answer) {
      var pings = Object.assign({}, settings.proxyPings)
      pings[id] = answer.ok ? { seconds: answer.result.seconds } : { error: true }
      settings.proxyPings = pings
    })
  }

function toggleProxy(context, proxy) {
  var app = context.app
  var settings = context.settings
    settings.error = ""
    settings.app.request(proxy.enabled ? "proxy.disable" : "proxy.enable", proxy.enabled ? {} : { id: proxy.id }, function (answer) {
      if (!answer.ok) settings.error = answer.error || "Telegram did not take the change"
      settings.loadProxies()
    })
  }

function askRemoveProxy(context, proxy) {
  var app = context.app
  var settings = context.settings
    settings.ask("Remove the proxy " + proxy.server + ":" + proxy.port + "?", function () {
      settings.app.request("proxy.remove", { id: proxy.id }, function (answer) {
        if (!answer.ok) settings.error = answer.error || "Telegram did not take the change"
        settings.loadProxies()
      })
    })
  }

function startProxyFlow(context, kind) {
  var settings = context.settings
    settings.error = ""
    settings.confirm = null
    settings.flowSerial++
    settings.flow = { kind: "proxy", proxyType: kind, steps: Model.proxySteps(kind), step: 0, values: {} }
    settings.showFlowStep()
  }

function addProxy(context, kind, values) {
  var app = context.app
  var settings = context.settings
    var args
    if (kind === "link") {
      args = { link: Model.proxyLinkUrl(values.link) }
    } else {
      var address = Model.parseProxyAddress(values.address)
      args = { type: kind, server: address.server, port: address.port }
      if (kind === "mtproto") args.secret = values.secret.trim()
      else { args.username = values.username || ""; args.password = values.password || "" }
    }
    settings.app.request(kind === "link" ? "proxy.addLink" : "proxy.add", args, function (answer) {
      if (!answer.ok) settings.error = answer.error || "Telegram did not take the proxy"
      settings.loadProxies()
    })
  }
