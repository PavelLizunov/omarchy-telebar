import QtQuick
import QtQuick.Window
import QtMultimedia
import Quickshell
import "../../app" as App

// Dedicated test display, synthetic round-cadence.mp4, absolute TELEBAR_TEST_OUTPUT.
// No other animation and no image grab until natural presentation has passed.
ShellRoot {
  id: test
  property int phase: 0
  property int trial: 0
  property int swaps: 0
  property int decoded: 0
  property int beforeSwaps: 0
  property int beforeDecoded: 0
  property bool grabbing: false
  property real sampledAt: 0
  property real deadline: Date.now() + 10000
  readonly property string output: Quickshell.env("TELEBAR_TEST_OUTPUT")
  readonly property string clip: decodeURIComponent(String(Qt.resolvedUrl("round-cadence.mp4")).replace("file://", ""))
  FixtureApp { id: model }
  FloatingWindow {
    visible: true
    implicitWidth: 400; implicitHeight: 400
    color: model.background
    App.MediaView {
      id: media
      anchors.centerIn: parent
      width: implicitWidth; height: implicitHeight
      app: model
      message: ({id: 701, content: {kind: "videoNote", media: {duration: 14, length: 480,
        file: {id: 71, path: test.clip, active: false}}}})
      animationsEnabled: true
    }
  }
  function require(condition, reason) {
    if (condition) return true
    console.error("INLINE_FAIL", trial, phase, reason)
    phase = -1; Qt.quit(); return false
  }
  Connections { target: media.Window.window; function onFrameSwapped() { test.swaps++ } }
  Connections { target: media.playback ? media.playback.videoSink : null; function onVideoFrameChanged() { test.decoded++ } }
  Timer {
    interval: 100; repeat: true; running: test.phase >= 0
    onTriggered: {
      var now = Date.now(), player = media.playback
      if (!test.require(now < test.deadline, "finite deadline") || test.grabbing) return
      if (test.phase === 3) {
        if (!test.require(!player, "released decoder")) return
        test.trial++
        if (test.trial === 3) {
          console.log("INLINE_LIFECYCLE_PASS", "3 natural-playback opens; inspect circular PNGs separately")
          test.phase = -1; Qt.quit(); return
        }
        media.togglePlay()
        test.phase = 0; test.deadline = now + 10000; return
      }
      if (!player || player.position < 300) return
      if (test.phase === 0) {
        if (!test.require(test.output.charAt(0) === "/", "absolute output required")) return
        if (test.trial === 0) media.togglePlay()
        test.sampledAt = now; test.phase = 1; return
      }
      if (test.phase === 1) {
        if (now - test.sampledAt < 400) return
        test.sampledAt = now; test.beforeSwaps = test.swaps; test.beforeDecoded = test.decoded; test.phase = 2; return
      }
      if (test.phase === 2) {
        var elapsed = now - test.sampledAt
        if (elapsed < 1600) return
        var swaps = test.swaps - test.beforeSwaps, decoded = test.decoded - test.beforeDecoded
        console.log("INLINE_NATURAL", JSON.stringify({trial: test.trial, elapsedMs: elapsed,
          swaps: swaps, decoded: decoded, position: player.position, error: player.error, api: media.GraphicsInfo.api}))
        if (!test.require(player.error === MediaPlayer.NoError && player.videoSink.videoSize.width === 480 && !player.muted,
              "audible original video") ||
            !test.require(elapsed < 4000 && swaps * 1000 / elapsed >= 10 && decoded >= 10,
              "natural presentation without forced image grabs")) return
        test.grabbing = true
        test.require(media.grabToImage(function(image) {
          test.require(image.saveToFile(test.output + "/inline-" + test.trial + ".png"), "save painted circle")
          media.releasePlayback()
          test.grabbing = false; test.phase = 3
        }), "queue painted circle")
      }
    }
  }
}
