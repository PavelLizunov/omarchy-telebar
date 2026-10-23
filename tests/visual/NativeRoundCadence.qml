import QtQuick
import QtQuick.Window
import QtMultimedia
import Quickshell
import "../../app" as App

// VM-only: actual inline playback -> Main-owned fullscreen, cold and reopen.
// Supply a >=12s, 30fps synthetic audio/video clip beside this file.
// Sink frames are diagnostic only. Window swaps are a necessary, not sufficient,
// presentation check; pair this with changing painted pixels/compositor capture.
ShellRoot {
  id: test
  property int phase: 0
  property int trial: 0
  property real deadline: Date.now() + 45000
  property real started: 0
  property real sampled: 0
  property var viewer: null
  property int frames: 0
  property int swaps: 0
  property int lastFrames: 0
  property int lastSwaps: 0
  property int samples: 0
  property bool failed: false
  property bool lowCadence: false
  property int backgroundCount: 2
  readonly property string clip: decodeURIComponent(String(Qt.resolvedUrl("round-cadence.mp4")).replace("file://", ""))
  readonly property var note: ({ id: 701, chatId: 101, content: { kind: "videoNote", media: { duration: 12,
    file: { id: 71, path: test.clip, active: false } } } })
  App.Main { id: main; autoStart: false; socketPath: ""; surfaceVisible: true }
  App.MediaView {
    id: inlineVideo
    parent: main.contentSurface.contentItem
    x: 20; y: 20
    width: implicitWidth; height: implicitHeight
    app: main
    message: test.note
    animationsEnabled: true
  }
  Item {
    parent: main.contentSurface.contentItem
    Repeater {
      model: test.backgroundCount
      App.MediaView {
        required property int index
        x: 250 + index * 50; y: 20
        width: implicitWidth; height: implicitHeight
        app: main
        animationsEnabled: true
        message: ({ id: 702 + index, chatId: 101, content: { kind: "gif", media: { width: 384, height: 384,
          file: { id: 72 + index, path: test.clip, active: false } } } })
      }
    }
  }
  function find(item) {
    if (item.objectName === "round-video-viewer") return item
    for (var child of item.children || []) { var found = find(child); if (found) return found }
    return null
  }
  function fail(reason) {
    failed = true
    console.error("CADENCE_FAIL", reason)
    Qt.quit()
  }
  Connections {
    target: test.viewer && test.viewer.player ? test.viewer.player.videoSink : null
    function onVideoFrameChanged() { test.frames++ }
  }
  Connections {
    target: test.viewer ? test.viewer.Window.window : null
    function onFrameSwapped() { test.swaps++ }
  }
  Timer {
    interval: 100; repeat: true; running: !test.failed
    onTriggered: {
      var now = Date.now()
      if (now > test.deadline) { test.fail("Timed out in phase " + test.phase); return }
      if (test.phase === 0) {
        if (!inlineVideo.playback || inlineVideo.playback.position < 300) return
        inlineVideo.togglePlay()
        test.phase = 1
        return
      }
      if (test.phase === 1) {
        if (!inlineVideo.playback || inlineVideo.playback.position < 300) return
        inlineVideo.enlarge()
        test.phase = 2
        test.started = now
        test.deadline = now + 25000
        test.frames = 0; test.swaps = 0
        return
      }
      test.viewer = test.find(main.mediaSurface.contentItem)
      var v = test.viewer, p = v ? v.player : null
      if (test.phase === 2) {
        if (!p || p.position < 400 || test.frames < 3) return
        if (inlineVideo.playback) { test.fail("Inline decoder retained under fullscreen"); return }
        console.log("CADENCE_HANDOFF", JSON.stringify({ trial: test.trial, firstMs: now - test.started,
          api: v.GraphicsInfo.api, error: p.error, position: p.position }))
        test.sampled = now; test.lastFrames = test.frames; test.lastSwaps = test.swaps
        test.samples = 0; test.phase = 3
        return
      }
      if (test.phase !== 3 || now - test.sampled < 1000) return
      var elapsed = now - test.sampled
      var presented = test.swaps - test.lastSwaps
      var sample = { trial: test.trial, frames: test.frames - test.lastFrames, swaps: presented,
        elapsedMs: elapsed, position: p ? p.position : 0 }
      console.log("CADENCE_SAMPLE", JSON.stringify(sample))
      if (presented * 1000 / elapsed < 10) test.lowCadence = true
      test.lastFrames = test.frames; test.lastSwaps = test.swaps; test.sampled = now
      if (++test.samples < 3) return
      v.closed()
      if (main.viewerMessageId !== 0) { test.fail("Main close did not clear viewer"); return }
      test.trial++; test.viewer = null
      if (test.trial < 2) { test.phase = 0; test.deadline = now + 45000; return }
      if (test.lowCadence) { test.fail("Presentation below 10 FPS; decoded frames do not establish playback"); return }
      console.log("CADENCE_SCHEDULING_PASS", "Window cadence only; verify continuing painted changes separately")
      Qt.quit()
    }
  }
}
