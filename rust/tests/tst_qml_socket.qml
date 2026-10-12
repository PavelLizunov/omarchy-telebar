import QtQuick
import QtTest
import Quickshell
import "." as App

// Actual Quickshell Socket/TelebarClient against an explicitly supplied inert
// Rust service. No process autostart, Telegram profile, keys or desktop surface.
TestCase {
  id: tests
  name: "RustSocketActualQmlClient"
  when: windowShown
  width: 500
  height: 250
  visible: true
  property var helloView: null
  property var lastAnswer: null
  property var settingsEvent: null
  property var order: []
  Component {
    id: scene
    App.CandidateClient {
      autoStart: false
      socketPath: Quickshell.env("TELEBAR_FIXTURE_SOCKET")
      onHello: function(view) { tests.helloView = view }
      onServiceEvent: function(name, event) {
        if (name === "settings") { tests.settingsEvent = event; tests.order.push("event") }
      }
    }
  }
  function test_hello_settings_order_and_window_shape() {
    verify(Quickshell.env("TELEBAR_ISOLATED_CONSUMER") === "1")
    var client = createTemporaryObject(scene, tests)
    verify(client !== null)
    tryVerify(function() { return tests.helloView !== null }, 5000)
    compare(helloView.activeAccount, "default")
    compare(helloView.auth.state, "needCredentials")
    compare(helloView.chats.length, 0)
    compare(helloView.allChats, undefined)
    client.request("settings.set", {settings: {globalShortcuts: {"global.panel": "SUPER + P"}}}, function(answer) {
      tests.lastAnswer = answer
      tests.order.push("reply")
    })
    tryVerify(function() { return tests.settingsEvent !== null }, 5000)
    compare(lastAnswer.ok, true)
    compare(lastAnswer.result.globalStatus["global.panel"], "unavailable")
    compare(order.join(","), "reply,event")
    lastAnswer = null
    client.request("hello", {window: true}, function(answer) { tests.lastAnswer = answer })
    tryVerify(function() { return tests.lastAnswer !== null }, 3000)
    compare(lastAnswer.result.allChats.length, 0)
    compare(lastAnswer.result.stories.length, 0)
    compare(Object.keys(client.callbacks).length, 0)
    lastAnswer = null
    client.request("app.quit", {}, function(answer) { tests.lastAnswer = answer })
    tryVerify(function() { return tests.lastAnswer !== null }, 3000)
    compare(lastAnswer.ok, true)
  }
}
