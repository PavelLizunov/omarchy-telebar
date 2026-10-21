import QtQuick
import QtQuick.Window
import QtTest
import QtMultimedia
import "../../app" as App

TestCase {
  id: tests
  name: "RoundSurfaceStartup"
  width: 900
  height: 760
  visible: true
  when: windowShown
  Component {
    id: coldWindow
    Window {
      id: host
      width: 900
      height: 760
      visible: false
      property alias viewer: viewer
      QtObject {
        id: model
        property string fontFamily: "Sans Serif"
        property real playbackRate: 1
        function fileState(file) { return file }
        function download(id, priority, callback) {}
      }
      App.RoundVideoViewer {
        id: viewer
        anchors.fill: parent
        app: model
        playbackMuted: true
        message: ({ id: 71, content: { kind: "videoNote", media: { file: {
          id: 71, path: decodeURIComponent(Qt.resolvedUrl("round-480.mp4").toString().replace("file://", ""))
        } } } })
      }
    }
  }
  function test_cold_overlay_waits_for_render_surface() {
    var host = createTemporaryObject(coldWindow, tests)
    verify(host !== null)
    wait(100)
    compare(host.viewer.player, null, "Do not start audio/decoder before the overlay's first render")
    host.visible = true
    tryVerify(function () { return host.viewer.player && host.viewer.player.position > 150 }, 8000)
    tryCompare(host.viewer, "pictureReady", true, 5000)
    host.viewer.player.pause()
    var image = grabImage(host.contentItem), colored = 0
    for (var y = 200; y < 500; y += 20) for (var x = 320; x < 580; x += 20) {
      var c = image.pixel(x, y)
      if (Math.max(c.r, c.g, c.b) - Math.min(c.r, c.g, c.b) > 0.15) colored++
    }
    verify(colored > 20, "Cold overlay must paint its decoded video")
    compare(host.viewer.player.error, MediaPlayer.NoError)
    host.visible = false
  }
}
