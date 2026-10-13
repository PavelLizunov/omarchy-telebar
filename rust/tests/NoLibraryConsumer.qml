import QtQuick
import "../../app" as App

// Actual product client and Quickshell socket. Only an explicit disposable
// no-native owner is allowed; all test operations are local or rejected.
Rectangle {
  id: fixture
  width: 500
  height: 180
  color: "#101315"
  property string fixtureSocket: ""
  property bool settled: false
  property string failure: ""
  readonly property bool ready: settled && failure === ""
  property string stage: "Connecting to the isolated no-library owner"
  property var order: []
  function require(condition, reason) { if (!condition) { failure = reason; throw new Error(reason) } }
  App.TelebarClient {
    id: client
    autoStart: false
    socketPath: fixture.fixtureSocket
    onHello: function(view) {
      fixture.require(view.auth.state === "noLibrary" && view.chats.length === 0, "missing-library state not delivered")
      fixture.stage = "Checking local settings without TDLib"
      client.request("settings.set", {settings:{globalShortcuts:{"global.panel":"SUPER + P"}}},function(answer) {
        fixture.require(answer.ok && answer.result.globalStatus["global.panel"] === "unavailable", "local worker did not reply")
        fixture.order.push("reply")
      })
    }
    onServiceEvent: function(name,event) {
      if (name !== "settings") return
      fixture.order.push("event")
      fixture.require(fixture.order.join(",") === "reply,event","local reply/event order changed")
      client.request("proxies.list",{},function(answer) {
        fixture.require(!answer.ok && answer.error === "Telegram is not available","no-native query not rejected")
        client.request("hello",{window:true},function(answer) {
          fixture.require(answer.ok && answer.result.auth.state === "noLibrary","peer lost after refused query")
          fixture.require(Object.keys(client.callbacks).length === 0,"local callbacks remain")
          fixture.stage = "Actual QML client stays usable without TDLib. Checks passed."
          fixture.settled = true
        })
      })
    }
  }
  Text {
    anchors.fill: parent
    anchors.margins: 16
    text: fixture.stage
    wrapMode: Text.Wrap
    textFormat: Text.PlainText
    color: "#cacccc"
    font.pixelSize: 16
  }
}
