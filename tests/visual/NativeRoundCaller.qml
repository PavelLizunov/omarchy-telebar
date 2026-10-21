import QtQuick
import QtMultimedia
import Quickshell
import "../../app" as App

// Native VM-only scenario: actual Main owns its real layer-shell overlay.
// Blank transport, synthetic local media, no account or Telegram actions.
ShellRoot {
  id: test
  property int trial: 0
  property int phase: 0
  property real started: Date.now()
  property var viewer: null
  property bool capturing: false
  property bool failed: false
  readonly property string clip: decodeURIComponent(Qt.resolvedUrl("round-480.mp4").toString().replace("file://", ""))
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
    animationsEnabled: false
  }
  function require(ok, message) {
    if (!ok) {
      test.failed = true
      console.error("ROUND_CALLER_FAIL", message)
      Qt.quit()
      throw new Error(message)
    }
  }
  function find(item) {
    if (item.objectName === "round-video-viewer") return item
    for (var child of item.children || []) { var result = find(child); if (result) return result }
    return null
  }
  Timer {
    interval: 100; repeat: true; running: !test.failed
    onTriggered: {
      if (test.capturing) return
      test.require(Date.now() - test.started < 15000, "Bounded caller readiness")
      if (test.phase === 0) {
        test.require(main.socketPath === "" && !main.autoStart, "Inert transport")
        inlineVideo.enlarge()
        test.phase = 1
        return
      }
      if (test.phase === 1) {
        test.require(main.viewerMessageId === 701 && main.viewerFileId === 71, "Main owns original identity")
        test.viewer = test.find(main.mediaSurface.contentItem)
        var v = test.viewer, p = v ? v.player : null
        if (!v || !p || p.position < 300 || !v.pictureReady) return
        test.require(main.mediaSurface.visible && main.mediaSurface.width > 500, "Native overlay mapped")
        test.require(p.error === MediaPlayer.NoError, "Decoder has no error")
        var record = { trial: test.trial, position: p.position, width: p.videoSink.videoSize.width,
          api: v.GraphicsInfo.api, elapsedMs: Date.now() - test.started, overlayVisible: main.mediaSurface.visible }
        test.capturing = true
        v.grabToImage(function(result) {
          var saved = result.saveToFile("caller-" + test.trial + ".png")
          test.require(saved, "Actual overlay capture saved")
          console.log("ROUND_CALLER_SAMPLE", JSON.stringify(record))
          test.viewer.closed()
          test.require(main.viewerMessageId === 0, "Actual close handler cleared Main state")
          test.trial++
          test.phase = 2
          test.started = Date.now()
          test.capturing = false
        })
        return
      }
      if (Date.now() - test.started < 250) return
      if (test.trial === 2) {
        console.log("ROUND_CALLER_PASS", "cold first open and reopen; inspect/assert both saved PNGs")
        Qt.quit()
      } else { test.phase = 0 }
    }
  }
}
