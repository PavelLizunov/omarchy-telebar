import QtQuick
import QtQuick.Window
import QtMultimedia
import QtTest
import Quickshell
import "../../app" as App

// Synthetic video feeds the actual recorder preview; its hardware loader stays off.
// Natural window presentation must pass before any image grab. No recording/send.
ShellRoot {
  id: test
  property int phase: 0
  property int trial: 0
  property int swaps: 0
  property int decoded: 0
  property int beforeSwaps: 0
  property int beforeDecoded: 0
  property real sampledAt: 0
  property real deadline: Date.now() + 10000
  property var capture: null
  property var output: null
  readonly property string directory: Quickshell.env("TELEBAR_TEST_OUTPUT")
  FixtureApp { id: model }
  TestCase { id: finder; when: false }
  FloatingWindow {
    visible: true
    implicitWidth: 600; implicitHeight: 600
    color: model.background
    App.VideoNoteRecorder { id: recorder; anchors.fill: parent; app: model }
  }
  MediaPlayer {
    id: player
    source: Qt.resolvedUrl("round-cadence.mp4")
    videoOutput: test.output
    loops: MediaPlayer.Infinite
  }
  Connections { target: recorder.Window.window; function onFrameSwapped() { test.swaps++ } }
  Connections { target: test.output ? test.output.videoSink : null; function onVideoFrameChanged() { test.decoded++ } }
  function require(condition, reason) {
    if (condition) return true
    console.error("RECORDER_FAIL", trial, phase, reason)
    phase = -1; Qt.quit(); return false
  }
  function videoOutput(item) {
    if (item.videoSink !== undefined) return item
    for (var i = 0; i < item.children.length; i++) {
      var found = videoOutput(item.children[i])
      if (found) return found
    }
    return null
  }
  function open() {
    recorder.open(42, {account: model.activeAccount, chatId: 42})
    output = videoOutput(recorder)
    if (!require(output && capture && !capture.item, "actual preview without hardware")) return
    player.play()
    phase = 1; deadline = Date.now() + 10000
  }
  Component.onCompleted: {
    capture = finder.findChild(recorder, "video-capture-session")
    capture.active = false
    Qt.callLater(open)
  }
  Timer {
    interval: 100; repeat: true; running: test.phase >= 0
    onTriggered: {
      var now = Date.now()
      if (!test.require(now < test.deadline, "finite deadline")) return
      if (test.phase === 4) {
        if (!test.require(!test.capture.item && !recorder.visible, "closed with hardware still unloaded")) return
        test.trial++
        if (test.trial === 3) {
          console.log("RECORDER_LIFECYCLE_PASS", "3 natural-preview opens; synthetic feed, not physical camera acceptance")
          test.phase = -1; Qt.quit(); return
        }
        test.open(); return
      }
      if (test.phase === 1 && player.position > 700) {
        test.sampledAt = now; test.beforeSwaps = test.swaps; test.beforeDecoded = test.decoded; test.phase = 2
      }
      if (test.phase !== 2 || now - test.sampledAt < 1600) return
      var elapsed = now - test.sampledAt, swaps = test.swaps - test.beforeSwaps, decoded = test.decoded - test.beforeDecoded
      console.log("RECORDER_NATURAL", JSON.stringify({trial: test.trial, elapsedMs: elapsed, swaps: swaps, decoded: decoded, error: player.error}))
      if (!test.require(player.error === MediaPlayer.NoError && decoded >= 10 && elapsed < 4000 && swaps * 1000 / elapsed >= 10,
            "natural presentation without forced image grabs")) return
      test.phase = 3
      test.require(test.output.parent.parent.grabToImage(function(image) {
        test.require(image.saveToFile(test.directory + "/recorder-" + test.trial + ".png"), "save painted preview")
        player.stop(); test.output = null; recorder.finish(false); test.phase = 4
      }), "queue painted preview")
    }
  }
}
