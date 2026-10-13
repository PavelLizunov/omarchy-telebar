import QtQuick
import qs.Commons

// Actual service/quick consumer with a checked source overlay changing only
// Process/FileView bindings to PreviewIo and a blank client socket default.
// Native Socket stays unchanged and only connects after explicit path setup.
Rectangle {
  id: fixture
  width: 720
  height: 650
  color: "#101315"
  property string pluginRoot: ""
  property string serviceEntry: "shell/Service.qml"
  property string quickEntry: "shell/QuickView.qml"
  property string fixtureSocket: ""
  property int themeRadius: 0
  property bool settled: false
  property string failure: ""
  readonly property bool ready: settled && failure === ""
  property var serviceOwner: null
  property var wire: null
  property var quick: null
  property int ticks: 0
  property bool opened: false
  function require(condition, reason) {
    if (!condition) {failure=reason;throw new Error(reason)}
  }
  Component.onCompleted: {
    require(pluginRoot.length > 0 && pluginRoot[0] === "/", "explicit plugin source root")
    require((serviceEntry === "shell/Service.qml" && quickEntry === "shell/QuickView.qml")
      || (serviceEntry === "shell/TelebarService.qml" && quickEntry === "shell/TelebarQuickView.qml"), "explicit known consumer type pair")
    require(fixtureSocket.length > 0 && fixtureSocket.length <= 100, "explicit short fixture socket")
    Style.cornerRadius = themeRadius
    var component=Qt.createComponent("file://"+pluginRoot+"/"+serviceEntry)
    require(component.status === Component.Ready, component.errorString())
    serviceOwner=component.createObject(fixture, {stopped:true})
    require(serviceOwner !== null,"actual shell service missing")
    var inertProcesses=0
    for (var child of serviceOwner.children) {
      if (child.command !== undefined) {
        require(child.previewIoInert === true,"service Process must be explicit inert PreviewIo")
        require(child.executions === 0,"inert Process must never execute")
        inertProcesses++
      }
      if (typeof child.reconnect === "function") wire=child
    }
    require(inertProcesses === 2,"both service process owners must be inert")
    require(wire !== null,"actual service client missing")
    require(wire.socketPath === "" && !wire.connected,"no default production socket connection")
    wire.autoStart=false
    wire.socketPath=fixtureSocket
    wire.reconnect()
    component=Qt.createComponent("file://"+pluginRoot+"/"+quickEntry)
    require(component.status === Component.Ready, component.errorString())
    quick=component.createObject(fixture,{width:fixture.width-24,height:fixture.height-24,x:12,y:12,compact:false,opened:false,service:serviceOwner})
    require(quick !== null,"actual quick view missing")
    settle.start()
  }
  Timer {
    id: settle
    interval: 25
    repeat: true
    onTriggered: {
      if (++fixture.ticks >= 180) {stop();fixture.require(false,"finite service/quick readiness")}
      if (!serviceOwner.ready || serviceOwner.chats.length !== 1) return
      fixture.require(serviceOwner.connected && serviceOwner.activeAccount === "default", "actual service snapshot identity")
      fixture.require(serviceOwner.accounts.length === 1,"actual account list")
      if (!fixture.opened) {fixture.opened=true;quick.opened=true;quick.reset(101,0);return}
      if (quick.history.length !== 3) return
      fixture.require(quick.replyChatId === 101 && quick.ready,"actual quick history routing")
      fixture.require(wire.connected,"native Socket remains usable")
      fixture.settled=true
      stop()
    }
  }
}
