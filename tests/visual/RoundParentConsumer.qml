import QtQuick
import QtMultimedia
import qs.Commons
import "../../app" as App
import "../../shell" as Shell

Rectangle {
  id: fixture
  width: 900
  height: 760
  property string mode: "main"
  property int themeRadius: 0
  property bool settled: false
  property string failure: ""
  property var consumer: null
  property var surface: null
  property var viewer: null
  property int frames: 0
  readonly property bool ready: settled && failure === ""
  readonly property string videoPath: decodeURIComponent(String(Qt.resolvedUrl("round-480.mp4")).replace("file://", ""))
  readonly property var note: ({ id: 701, chatId: 101, outgoing: true, content: { kind: "videoNote", media: { duration: 4, width: 480, height: 480, file: { id: 71, path: videoPath, active: false } } } })
  function require(condition, reason) { if (!condition) { fixture.failure = reason; throw new Error(reason) } }
  function find(item, name) {
    if (!item) return null
    if (item.objectName === name) return item
    for (var child of item.children || []) { var value = find(child, name); if (value) return value }
    return null
  }
  FixtureApp { id: model }
  Component { id: mainScene; MainRoundConsumer {} }
  Component { id: quickScene; Shell.TelebarQuickView { service: model; opened: true } }
  function run() {
    Style.cornerRadius = themeRadius
    consumer = (mode === "main" ? mainScene : quickScene).createObject(fixture, { width: 900, height: 760 })
    require(!!consumer, "Actual parent must load")
    if (mode === "main") {
      require(!consumer.app.autoStart && consumer.app.socketPath === "", "No production I/O")
      consumer.prepareInline(note)
      surface = consumer.surface
    } else {
      model.history = [note]
      consumer.replyChatId = 101
      consumer.historyChatId = 101
      consumer.history = [note]
      consumer.historyComplete = true
      // The native test driver clicks the actual delegate, not openMedia().
    }
    settle.start()
  }
  Timer {
    id: settle
    interval: 20
    repeat: true
    property int ticks: 0
    onTriggered: {
      if (++ticks > 240) { stop(); fixture.require(false, "Bounded original parent readiness"); return }
      if (!fixture.surface && fixture.mode === "quick") {
        if (consumer.mediaSurface) {
          surface = consumer.mediaSurface
          surface.visible = false
          surface.contentItem.parent = fixture
          surface.contentItem.width = Qt.binding(function() { return fixture.width })
          surface.contentItem.height = Qt.binding(function() { return fixture.height })
        }
      }
      if (!surface) return
      if (fixture.mode === "main") {
        require(consumer.app.viewerMessageId === 701 && consumer.app.viewerFileId === 71, "Main preserves original message/file identity")
        require(!consumer.inlineMedia.playback, "Main destroys the inline decoder before fullscreen")
      }
      if (!viewer) viewer = fixture.find(surface.contentItem, "round-video-viewer")
      if (!viewer || !viewer.player) return
      viewer.playbackMuted = true
      if (viewer.player.error !== MediaPlayer.NoError) { stop(); fixture.require(false, "Native parent decoder error"); return }
      if (viewer.player.videoSink.videoSize.width !== 480 || viewer.player.position < 100) return
      require(!surface.visible, "No native surface is mapped by fixture")
      viewer.player.pause()
      fixture.settled = true
      stop()
    }
  }
  function finish() {
    require(mode === "main" ? consumer.app.viewerMessageId === 0 : consumer.viewingId === 0, "Actual close handler clears parent state")
  }
}
