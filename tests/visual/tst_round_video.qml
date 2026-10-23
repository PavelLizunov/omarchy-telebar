import QtQuick
import QtTest
import QtMultimedia
import "../../app" as App

TestCase {
  id: tests
  name: "ActualRoundVideoZoom"
  width: 900
  height: 760
  visible: true
  when: windowShown
  Component { id: scene; RoundVideoConsumer {} }
  Component { id: inlineScene; App.MediaView { width: implicitWidth; height: implicitHeight; animationsEnabled: false } }
  FixtureApp { id: inlineModel; signal roundVideoOpening(); property real viewerMessageId: 0; property var openedNote: null; function openPhoto(message) { openedNote = message } }
  SignalSpy { id: closedSpy; signalName: "closed" }

  function test_original_video_zoom_pan_pause_and_keyboard_close() {
    var item = createTemporaryObject(scene, tests)
    verify(item !== null)
    tryCompare(item, "ready", true, 5000)
    var viewer = item.viewer
    var player = viewer.player
    compare(player.videoSink.videoSize.width, 480)
    compare(player.error, MediaPlayer.NoError)
    verify(viewer.diameter > 500, "Original video is shown larger than its 200-unit inline bubble")
    var position = player.position
    var zoomIn = findChild(viewer, "round-video-zoom-in")
    mouseClick(zoomIn, zoomIn.width / 2, zoomIn.height / 2)
    compare(viewer.zoom, 1.25)
    compare(viewer.player, player, "Zoom retains the same native decoder")
    compare(player.position, position, "Zoom does not replay the video")
    var pan = findChild(viewer, "round-video-pan")
    mouseWheel(pan, pan.width / 2, pan.height / 2, 0, 120)
    compare(viewer.zoom, 1.5625)
    mousePress(pan, pan.width / 2, pan.height / 2)
    mouseMove(pan, pan.width / 2 + 45, pan.height / 2 + 30, 30)
    mouseRelease(pan, pan.width / 2 + 45, pan.height / 2 + 30)
    verify(viewer.panX > 0 && viewer.panY > 0)
    verify(Math.abs(viewer.panX) <= viewer.panLimit)
    viewer.forceActiveFocus()
    keyClick(Qt.Key_0)
    compare(viewer.zoom, 1)
    compare(viewer.panX, 0)
    keyClick(Qt.Key_Plus)
    compare(viewer.zoom, 1.25)
    var reset = findChild(viewer, "round-video-zoom-reset")
    reset.forceActiveFocus()
    keyClick(Qt.Key_Return)
    compare(viewer.zoom, 1)
    viewer.setZoom(99)
    compare(viewer.zoom, 4)
    viewer.setZoom(-1)
    compare(viewer.zoom, 1)
    reset.focus = false
    viewer.forceActiveFocus()
    keyClick(Qt.Key_Space)
    tryCompare(player, "playbackState", MediaPlayer.PlayingState)
    keyClick(Qt.Key_Space)
    tryCompare(player, "playbackState", MediaPlayer.PausedState)
    closedSpy.target = viewer
    keyClick(Qt.Key_Escape)
    compare(closedSpy.count, 1)
    closedSpy.target = null
  }

  function test_inline_pointer_and_keyboard_enlarge_without_replacing_play() {
    var item = createTemporaryObject(scene, tests)
    tryCompare(item, "ready", true, 5000)
    var media = createTemporaryObject(inlineScene, tests, { app: inlineModel, message: item.sample })
    verify(media !== null)
    var button = findChild(media, "round-video-enlarge")
    verify(button !== null && button.visible)
    inlineModel.openedNote = null
    mouseClick(button, button.width / 2, button.height / 2)
    compare(inlineModel.openedNote, item.sample)
    inlineModel.openedNote = null
    button.forceActiveFocus()
    keyClick(Qt.Key_Return)
    compare(inlineModel.openedNote, item.sample)
    media.spoiler = true
    verify(!button.visible)
    inlineModel.openedNote = null
    media.enlarge()
    compare(inlineModel.openedNote, null, "Covered video cannot open until revealed")
    media.revealed = true
    media.togglePlay()
    compare(inlineModel.openedNote, null, "Inline playback remains a separate action")
  }

  function test_other_inline_media_releases_before_round_open() {
    var item = createTemporaryObject(scene, tests)
    tryCompare(item, "ready", true, 5000)
    for (var kind of ["video", "videoNote"]) {
      var message = Object.assign({}, item.sample, { content: Object.assign({}, item.sample.content, { kind: kind }) })
      var media = createTemporaryObject(inlineScene, tests, { app: inlineModel, message: message, animationsEnabled: true })
      if (kind === "videoNote") tryVerify(function() { return media.playback !== null }, 1500)
      wait(0)
      media.togglePlay()
      tryVerify(function() { return media.playback !== null }, 2000)
      var player = media.playback
      tryCompare(player, "playbackState", MediaPlayer.PlayingState)
      inlineModel.roundVideoOpening()
      tryVerify(function() { return media.playback === null }, 2000,
        "Expansion must destroy the inline decoder, not retain its paused hardware buffers")
      media.togglePlay()
      tryVerify(function() { return media.playback !== null }, 2000)
      tryCompare(media.playback, "playbackState", MediaPlayer.PlayingState)
      media.releasePlayback()
    }
  }

  function videoOutputs(item) {
    var count = typeof item.videoSink !== "undefined" ? 1 : 0
    for (var child of item.children || []) count += videoOutputs(child)
    return count
  }

  function test_overlay_suspends_background_decoders_and_close_resumes() {
    var item = createTemporaryObject(scene, tests)
    tryCompare(item, "ready", true, 5000)
    for (var kind of ["videoNote", "gif"]) {
      var message = Object.assign({}, item.sample, { content: Object.assign({}, item.sample.content, { kind: kind }) })
      var media = createTemporaryObject(inlineScene, tests, { app: inlineModel, message: message, animationsEnabled: true })
      tryVerify(function() { return videoOutputs(media) > 0 }, 2000)
      inlineModel.viewerMessageId = 701
      tryCompare(media, "playbackSuspended", true)
      tryVerify(function() { return videoOutputs(media) === 0 }, 2000)
      inlineModel.viewerMessageId = 0
      tryCompare(media, "playbackSuspended", false)
      tryVerify(function() { return videoOutputs(media) > 0 }, 2000)
      media.releasePlayback()
    }
  }

  function test_photo_dismissal_does_not_restart_audible_video() {
    var item = createTemporaryObject(scene, tests)
    tryCompare(item, "ready", true, 5000)
    for (var paused of [false, true]) {
      var message = Object.assign({}, item.sample, { content: Object.assign({}, item.sample.content, { kind: "video" }) })
      var media = createTemporaryObject(inlineScene, tests, { app: inlineModel, message: message })
      media.togglePlay()
      tryVerify(function() { return media.playback !== null }, 2000)
      tryCompare(media.playback, "playbackState", MediaPlayer.PlayingState)
      if (paused) { media.togglePlay(); tryCompare(media.playback, "playbackState", MediaPlayer.PausedState) }
      inlineModel.viewerMessageId = 900
      tryVerify(function() { return media.playback === null }, 2000)
      inlineModel.viewerMessageId = 0
      wait(100)
      compare(media.playback, null, "Closing a photo must not autoplay a previously playing or paused video")
      media.togglePlay()
      tryVerify(function() { return media.playback !== null }, 2000)
      tryCompare(media.playback, "playbackState", MediaPlayer.PlayingState)
      media.releasePlayback()
    }
  }

  function test_refused_download_has_error_and_no_decoder() {
    var item = createTemporaryObject(scene, tests, { downloadRefused: true, files: { "71": { id: 71, path: "", active: false } } })
    tryVerify(function() { return item.downloads > 0 })
    compare(item.viewer.downloadError, "Could not download this round video")
    compare(item.viewer.player, null)
    verify(findChild(item.viewer, "round-video-status").text.indexOf("Could not download") >= 0)
  }

  function test_missing_file_waits_for_original_download() {
    var item = createTemporaryObject(scene, tests, { files: { "71": { id: 71, path: "", active: false } } })
    verify(item !== null)
    tryVerify(function() { return item.downloads > 0 })
    compare(item.viewer.url, "")
    compare(item.viewer.player, null)
    item.files = ({})
    tryCompare(item, "ready", true, 5000)
    compare(item.viewer.player.videoSink.videoSize.width, 480)
  }
}
