import QtQuick
import "." as App

// Actual client/SplitParser/Socket. The renderer applies an isolated socket path
// before creation. This is a protocol fixture, not a reconstructed product UI.
Rectangle {
  id: fixture
  width: 500
  height: 180
  color: "#101315"
  property string fixtureSocket: ""
  property bool ready: false
  property string failure: ""
  property string stage: "Connecting to the isolated Rust socket"
  property var order: []
  function require(condition, reason) { if (!condition) { failure = reason; throw new Error(reason) } }
  function changed(answer) {
    require(answer.ok, "settings response failed")
    require(answer.result.globalStatus["global.panel"] === "unavailable", "unexpected shortcut status")
    fixture.order.push("reply")
  }
  App.CandidateClient {
    id: client
    autoStart: false
    socketPath: fixture.fixtureSocket
    onHello: function(view) {
      fixture.require(view.activeAccount === "default" && view.chats.length === 0, "incompatible hello")
      fixture.require(view.auth.state === "needCredentials", "unexpected isolated authorization state")
      fixture.require(view.allChats === undefined, "shell hello includes window-only data")
      fixture.stage = "Checking settings response and event order"
      client.request("settings.set", {settings: {globalShortcuts: {"global.panel": "SUPER + P"}}}, fixture.changed)
    }
    onServiceEvent: function(name, event) {
      if (name !== "settings") return
      fixture.order.push("event")
      fixture.require(fixture.order.join(",") === "reply,event", "response event order changed")
      client.request("hello", {window: true}, function(answer) {
        fixture.require(answer.ok && answer.result.allChats.length === 0 && answer.result.stories.length === 0, "incompatible window hello")
        fixture.require(Object.keys(client.callbacks).length === 0, "pending callbacks remain")
        fixture.stage = "Actual QML client connected. Rust protocol checks passed."
        fixture.ready = true
      })
    }
  }
  Text {
    anchors.fill: parent
    anchors.margins: 16
    text: fixture.failure || fixture.stage
    wrapMode: Text.Wrap
    textFormat: Text.PlainText
    color: "#cacccc"
    font.pixelSize: 16
  }
}
