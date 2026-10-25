import QtQuick
import QtQuick.Window
import QtMultimedia
import Quickshell
import "../../app" as App
import "../../shell" as Shell

// Dedicated test desktop only. No Telegram socket or capture devices.
// Supply round-cadence.mp4 and a new TELEBAR_TEST_OUTPUT directory.
// PNGs and natural window cadence must be checked separately; no pixel PASS here.
ShellRoot {
  id: test
  readonly property string route: Quickshell.env("TELEBAR_TEST_ROUTE") || "quick"
  readonly property string output: Quickshell.env("TELEBAR_TEST_OUTPUT")
  readonly property string clip: decodeURIComponent(String(Qt.resolvedUrl("round-cadence.mp4")).replace("file://", ""))
  readonly property var note: ({ id: 701, chatId: 101, outgoing: true, date: 1790726000,
    content: { kind: "videoNote", media: { duration: 14, length: 480,
      file: { id: 71, path: test.clip, active: false } } } })
  property int phase: 0
  property int trial: 0
  property real deadline: Date.now() + 20000
  property real started: 0
  property real sampleStarted: 0
  property int swaps: 0
  property int initialSwaps: 0
  property int images: 0
  property bool grabbing: false
  property var viewer: null
  property var surface: null
  property int pausedPosition: 0
  property int requests: 0

  FixtureApp { id: model; history: [test.note]; onRequestObserved: function(cmd, args) { if (cmd === "media.stop") test.requests++ } }
  Loader { id: mainLoader; active: test.route === "main"; sourceComponent: App.Main { autoStart: false; socketPath: "" } }
  Loader {
    id: inlineLoader
    active: !!mainLoader.item
    sourceComponent: App.MediaView {
      parent: mainLoader.item.contentSurface.contentItem
      x: 20; y: 20; width: implicitWidth; height: implicitHeight
      app: mainLoader.item; message: test.note; animationsEnabled: true
    }
  }
  FloatingWindow {
    id: quickWindow
    visible: test.route === "quick"
    implicitWidth: 500; implicitHeight: 650
    color: model.background
    Shell.TelebarQuickView {
      id: quick
      anchors.fill: parent
      service: model
      opened: test.route === "quick"
      Component.onCompleted: if (opened) reset(101, 0)
    }
  }
  function find(item, name) {
    if (!item) return null
    if (item.objectName === name) return item
    for (var child of item.children || []) { var value = find(child, name); if (value) return value }
    return null
  }
  function require(condition, reason) {
    if (condition) return true
    console.error("ROUND_FAIL", route, trial, phase, reason)
    phase = -1
    Qt.quit()
    return false
  }
  function capture(name, item) {
    grabbing = true
    require(item.grabToImage(function(image) {
      require(image.saveToFile(output + "/" + route + "-" + trial + "-" + name + ".png"), "save painted image")
      grabbing = false
    }, Qt.size(Math.round(item.width / 2), Math.round(item.height / 2))), "queue painted image")
  }
  Connections {
    target: test.viewer ? test.viewer.Window.window : null
    function onFrameSwapped() { test.swaps++ }
  }
  Timer {
    interval: 100; repeat: true; running: test.phase >= 0
    onTriggered: {
      var now = Date.now()
      if (!test.require(now < test.deadline, "finite deadline")) return
      if (test.grabbing) return
      if (test.phase === 0) {
        if (!test.require(test.output.charAt(0) === "/", "absolute output required")) return
        var row = test.route === "quick" ? test.find(quick, "quick-round-video-701")
          : inlineLoader.item && inlineLoader.item.playback && inlineLoader.item.playback.position > 200
            ? test.find(inlineLoader.item, "round-video-enlarge") : null
        if (!row) return
        if (test.route === "main") inlineLoader.item.togglePlay()
        row.Accessible.pressAction()
        test.started = now
        test.phase = 1
        return
      }
      if (test.phase === 1) {
        test.surface = test.route === "quick" ? quick.mediaSurface : mainLoader.item.mediaSurface
        test.viewer = test.surface ? test.find(test.surface.contentItem, "round-video-viewer") : null
        var p = test.viewer ? test.viewer.player : null
        if (!p || p.position < 300 || p.videoSink.videoSize.width <= 0) return
        if (!test.require(p.error === MediaPlayer.NoError, "decoder error") ||
            !test.require(p.duration >= 13000 && p.videoSink.videoSize.width === 480 && !p.muted, "audible original video, not thumbnail") ||
            !test.require(test.route !== "main" || !inlineLoader.item.playback, "inline decoder released") ||
            !test.require(test.route !== "quick" || test.requests === test.trial + 1, "compact stops prior audio")) return
        var frame = test.find(test.viewer, "round-video-frame")
        console.log("ROUND_OPEN", JSON.stringify({route: test.route, trial: test.trial, firstMs: now - test.started,
          api: test.viewer.GraphicsInfo.api, width: test.viewer.width, height: test.viewer.height,
          frame: {x: frame.x, y: frame.y, diameter: frame.width}, muted: p.muted, source: String(p.source)}))
        test.images = 0; test.initialSwaps = test.swaps; test.sampleStarted = now
        test.phase = 2
        return
      }
      if (test.phase === 2) {
        if (test.images < 15) { test.capture("motion-" + test.images++, test.viewer); return }
        var elapsed = now - test.sampleStarted, swaps = test.swaps - test.initialSwaps
        console.log("ROUND_MOTION", JSON.stringify({route: test.route, trial: test.trial, images: test.images, elapsedMs: elapsed, swaps: swaps}))
        if (!test.require(elapsed < 4000 && swaps * 1000 / elapsed >= 10, "continuing presentation cadence")) return
        test.viewer.togglePlay()
        test.pausedPosition = test.viewer.player.position
        test.sampleStarted = now; test.phase = 3
        return
      }
      if (test.phase === 3) {
        if (now - test.sampleStarted < 400) return
        if (!test.require(test.viewer.player.playbackState === MediaPlayer.PausedState &&
            Math.abs(test.viewer.player.position - test.pausedPosition) < 100, "pause")) return
        test.viewer.setZoom(2)
        test.viewer.panX = test.viewer.panLimit / 2
        test.capture("zoom", test.viewer)
        test.viewer.togglePlay()
        test.sampleStarted = now; test.phase = 4
        return
      }
      if (test.phase === 4) {
        if (now - test.sampleStarted < 400) return
        if (!test.require(test.viewer.player.position > test.pausedPosition + 100, "resume")) return
        test.viewer.resetZoom()
        test.viewer.closed()
        if (!test.require(test.route === "quick" ? quick.viewingId === 0 : mainLoader.item.viewerMessageId === 0, "actual close handler")) return
        test.sampleStarted = now; test.phase = 5
        return
      }
      if (test.phase === 5) {
        if (now - test.sampleStarted < 400) return
        if (!test.require(!test.surface || !test.find(test.surface.contentItem, "round-video-player"), "closed decoder released") ||
            !test.require(test.route !== "main" || (inlineLoader.item.playback && inlineLoader.item.playback.muted), "background resumes muted")) return
        test.viewer = null; test.surface = null; test.trial++
        if (test.trial < 3) { test.phase = 0; test.deadline = now + 20000; return }
        console.log("ROUND_LIFECYCLE_PASS", test.route, "3 opens; inspect all PNGs separately")
        test.phase = -1
        Qt.quit()
      }
    }
  }
}
