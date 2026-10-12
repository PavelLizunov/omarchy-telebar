import QtQuick
import qs.Commons
import "../../app" as App

// Complete Main with its native surfaces unshown. Reparent only the actual
// window content subtree. Socket/autoStart are explicit inert capabilities.
Rectangle {
  id: fixture
  width: 1100
  height: 760
  color: "#101315"
  property string fixtureSocket: ""
  property string expectedAuth: "noLibrary"
  property string expectedRecord: ""
  property bool resolveRecord: false
  property bool resolutionRequested: false
  property int themeRadius: 0
  property bool ready: false
  property var main: null
  property var surface: null
  property int checks: 0
  property bool openRequested: false
  function require(condition,reason) { if (!condition) throw new Error(reason); checks++ }
  function findVisual(item,name) {
    if (item.objectName === name) return item
    for (var i=0;i<item.children.length;i++) {var found=findVisual(item.children[i],name);if(found)return found}
    return null
  }
  App.Main {
    id: app
    autoStart: false
    surfaceVisible: false
    socketPath: fixture.fixtureSocket
  }
  Component.onCompleted: {
    fixture.require(fixtureSocket.length > 0 && fixtureSocket.length <= 100,"explicit short Unix fixture socket required")
    Style.cornerRadius = themeRadius
    fixture.main = app
    surface=app.contentSurface
    fixture.require(surface !== null,"actual Main window missing")
    surface.contentItem.parent = fixture
    surface.contentItem.width = fixture.width
    surface.contentItem.height = fixture.height
    settle.start()
  }
  Timer {
    id: settle
    interval: 25
    repeat: true
    property int ticks: 0
    onTriggered: {
      if (++ticks >= 120) {stop();fixture.require(false,"finite Main readiness budget")}
      if (app.auth.state !== expectedAuth) return
      if (expectedAuth === "ready") {
        if (app.chats.length === 0) return
        if (!fixture.openRequested) {fixture.openRequested=true;app.openChatById(101,false);return}
        if (app.messagesFor(101).length !== 3) return
        fixture.require(!app.windowFocused,"inert Main must not acknowledge reads")
      }
      if (expectedRecord !== "" && !fixture.resolutionRequested) {
        if (!app.recordedSend || app.recordedSend.state !== expectedRecord) return
        var bar=findVisual(surface.contentItem,"recorded-send-bar")
        if (!bar || !bar.visible || bar.height <= 0) return
        var retry=findVisual(bar,"recorded-send-retry")
        var discard=findVisual(bar,"recorded-send-discard")
        fixture.require(retry !== null && discard !== null,"actual retained controls missing")
        fixture.require(retry.enabled === (expectedRecord === "prepareFailed" || expectedRecord === "rejected"),"retry uncertainty admission")
        fixture.require(discard.enabled,"explicit resolution remains reachable")
        fixture.require(bar.radius === themeRadius,"native radius must remain reactive")
        if (fixture.resolveRecord) {fixture.resolutionRequested=true;bar.act(false);return}
      }
      if (fixture.resolveRecord && app.recordedSend !== null) return
      fixture.require(surface.visible === false,"Main surface must stay unshown")
      fixture.require(surface.contentItem.parent === fixture,"actual Main content subtree")
      fixture.require(app.accounts.length === 1 && app.activeAccount === "default","account snapshot missing")
      fixture.ready = true
      stop()
    }
  }
}
