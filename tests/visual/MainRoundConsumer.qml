import QtQuick
import "../../app" as App

Rectangle {
  id: fixture
  width: 900
  height: 760
  property alias app: app
  property var surface: null
  property var inlineMedia: null
  Component { id: inlineScene; App.MediaView { width: implicitWidth; height: implicitHeight; animationsEnabled: true } }
  function prepareInline(message) { inlineMedia = inlineScene.createObject(fixture, { app: app, message: message, x: 350, y: 240 }) }
  function openFromInline() { inlineMedia.enlarge() }
  property bool settled: false
  readonly property bool ready: settled
  App.Main { id: app; autoStart: false; surfaceVisible: false; socketPath: "" }
  Component.onCompleted: {
    fixture.surface = app.mediaSurface
    if (!surface) throw new Error("Actual Main media surface missing")
    // Render only the actual content; never map the layer-shell/window surface.
    surface.visible = false
    surface.contentItem.parent = fixture
    surface.contentItem.z = 10
    surface.contentItem.width = Qt.binding(function() { return fixture.width })
    surface.contentItem.height = Qt.binding(function() { return fixture.height })
    fixture.settled = true
  }
}
