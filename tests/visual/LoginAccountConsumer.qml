import QtQuick
import QtTest
import "../../app" as App

// Complete Main, synthetic second-account auth events, unshown native surfaces.
Rectangle {
  id: fixture
  width: 900
  height: 650
  color: app.background
  property bool ready: false
  property string authStep: "phone"
  property string qrImage: ""
  property bool exerciseMethodSwitch: false
  property int methodCancellations: 0
  property bool exerciseTransitions: true
  property string failure: ""
  property var surface: null
  property var client: null
  property int phase: 0
  TestResult { id: lookup }
  function require(value, reason) {
    if (!value) { failure = reason; throw new Error(reason) }
  }
  function findLogin(item) {
    if (item.objectName === "account-login") return item
    var children = item.children || []
    for (var i = 0; i < children.length; i++) {
      var found = findLogin(children[i])
      if (found) return found
    }
    return null
  }
  QtObject {
    id: wire
    property bool connected: true
    function flush() {}
    function write(line) {
      var request = JSON.parse(line)
      var result = request.cmd === "account.list" ? { accounts: [{ id: "default", state: "ready" }, { id: "additional", state: fixture.authStep }] } : {}
      if (request.cmd === "auth.logout") {
        fixture.require(request.account === "additional", "Only the pending additional account may be cancelled")
        fixture.methodCancellations++
        Qt.callLater(function () {
          client.handleLine(JSON.stringify({ id: request.id, ok: true, result: {} }))
          client.handleLine(JSON.stringify({ event: "auth", account: "additional", auth: { state: "phone" } }))
        })
        return
      }
      Qt.callLater(function () { client.handleLine(JSON.stringify({ id: request.id, ok: true, result: result })) })
    }
  }
  App.Main { id: app; autoStart: false; surfaceVisible: false; socketPath: "" }
  Component.onCompleted: {
    surface = app.contentSurface
    require(surface && !surface.visible && !app.autoStart && app.socketPath === "", "Inert Main only")
    surface.contentItem.parent = fixture
    surface.contentItem.width = Qt.binding(function () { return fixture.width })
    surface.contentItem.height = Qt.binding(function () { return fixture.height })
    client = lookup.findChild(app, "window-transport")
    require(client !== null, "Actual transport required")
    client.sock = wire
    app.applyAccountSnapshot({ activeAccount: "additional", accountRevision: 2, auth: { state: "starting" },
      accounts: [{ id: "default", state: "ready" }, { id: "additional", state: "starting" }], allChats: [] })
    settle.start()
  }
  Timer {
    id: settle
    interval: 30
    repeat: true
    property int ticks: 0
    onTriggered: {
      if (++ticks > 60) { stop(); fixture.require(false, "Auth transition never restored login: " + app.auth.state + ", transition=" + app.accountTransition + ", account=" + app.activeAccount + ", found=" + !!fixture.findLogin(surface.contentItem)); return }
      if (phase === 0) {
        client.handleLine(JSON.stringify({ event: "auth", account: "additional", auth: { state: fixture.authStep, image: fixture.qrImage } }))
        phase = 1
        return
      }
      var login = fixture.findLogin(surface.contentItem)
      if (!login || !login.visible || app.accountTransition) return
      fixture.require(app.activeAccount === "additional" && app.accounts.length === 2, "Other account must remain available")
      if (fixture.exerciseTransitions && phase < 4) {
        var next = phase === 1 ? "code" : phase === 2 ? "password" : fixture.authStep
        client.handleLine(JSON.stringify({ event: "auth", account: "additional", auth: { state: next, hint: "Synthetic hint", phone: "+10000000000", image: fixture.qrImage } }))
        phase++
        return
      }
      if (fixture.exerciseMethodSwitch && phase < 10) {
        fixture.require(login.step === "qr", "Start from actual QR login")
        login.switchMethod()
        phase = 10
        return
      }
      fixture.require(login.step === (fixture.exerciseMethodSwitch ? "phone" : fixture.authStep), "Login step must follow auth events")
      if (fixture.exerciseMethodSwitch) fixture.require(fixture.methodCancellations === 1 && login.askingPhone, "Cancellation must restore phone on same account")
      fixture.ready = true
      stop()
    }
  }
}
