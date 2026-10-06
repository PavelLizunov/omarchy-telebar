import QtQuick
import qs.Commons
import "Model.js" as Model
import "Keymap.js" as Keymap

// A photo at full size over the chat. Keyboard: ←/→ or h/l step through the chat's photos,
// Esc or q closes; a left click closes, a right click opens the photo's menu. The photo is the file the service already
// vouched for, downloaded on demand when you step to one that is not on disk yet.
FocusScope {
  id: viewer

  property var app
  property var messages: []
  property real messageId: 0
  property real fileId: 0
  property var extraMessage: null

  signal closed()

  readonly property var photos: {
    var items = Model.mediaMessages(viewer.messages).filter(function (m) { return m.content.kind === "photo" })
    if (viewer.extraMessage && viewer.extraMessage.id === viewer.messageId && !items.some(function (m) { return m.id === viewer.messageId && m.content.media.file.id === viewer.fileId }))
      items.push(viewer.extraMessage)
    return items
  }
  readonly property int index: {
    for (var i = 0; i < viewer.photos.length; i++) if (viewer.photos[i].id === viewer.messageId && (!viewer.fileId || viewer.photos[i].content.media.file.id === viewer.fileId)) return i
    return -1
  }
  readonly property var current: viewer.index >= 0 ? viewer.photos[viewer.index] : null
  readonly property var file: viewer.current ? app.fileState(viewer.current.content.media.file) : null
  readonly property string url: viewer.file ? Model.fileUrl(viewer.file.path) : ""

  visible: viewer.messageId > 0
  onVisibleChanged: if (visible) forceActiveFocus()
  onFileChanged: if (viewer.file && !viewer.url && !viewer.file.active) app.download(viewer.file.id, 32)

  property alias actions: actions

  function step(delta) {
    var next = viewer.index + delta
    if (viewer.index >= 0 && next >= 0 && next < viewer.photos.length) app.openPhoto(viewer.photos[next])
  }

  Keys.onPressed: function (event) {
    var keys = app.shortcuts
    if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && event.modifiers === Qt.ShiftModifier)) actions.openMenu(viewer.width / 2, viewer.height / 2)
    else if (Keymap.matches(keys, "photo.close", event)) viewer.closed()
    else if (Keymap.matches(keys, "photo.previous", event)) viewer.step(-1)
    else if (Keymap.matches(keys, "photo.next", event)) viewer.step(1)
    else return
    event.accepted = true
  }

  Rectangle {
    anchors.fill: parent
    color: Qt.rgba(0, 0, 0, 0.97)   // over the whole screen now: whatever is behind should not show through
  }

  MouseArea {
    anchors.fill: parent
    onClicked: viewer.closed()
  }

  Image {
    anchors.fill: parent
    anchors.margins: Style.space(48)
    anchors.bottomMargin: Style.space(80)
    visible: photo.status !== Image.Ready
    source: viewer.current ? Model.miniUrl(viewer.current.content.media.mini) : ""
    fillMode: Image.PreserveAspectFit
    smooth: true
  }

  Image {
    id: photo
    anchors.fill: parent
    anchors.margins: Style.space(48)
    anchors.bottomMargin: Style.space(80)
    source: viewer.url
    asynchronous: true
    cache: false
    sourceSize.width: Math.min(4096, Math.ceil(viewer.width * 1.5 / 256) * 256)
    sourceSize.height: Math.min(4096, Math.ceil(viewer.height * 1.5 / 256) * 256)
    autoTransform: true
    fillMode: Image.PreserveAspectFit
  }

  Column {
    anchors.horizontalCenter: parent.horizontalCenter
    anchors.bottom: parent.bottom
    anchors.bottomMargin: Style.space(20)
    width: Math.min(parent.width - Style.space(96), Style.space(900))
    spacing: Style.space(6)

    Text {
      width: parent.width
      visible: text !== ""
      horizontalAlignment: Text.AlignHCenter
      wrapMode: Text.Wrap
      maximumLineCount: 2
      elide: Text.ElideRight
      textFormat: Text.PlainText
      text: viewer.current ? (viewer.current.content.text || "") : ""
      color: "white"
      font.family: app.fontFamily
      font.pixelSize: Style.font.body
    }
    Text {
      width: parent.width
      horizontalAlignment: Text.AlignHCenter
      wrapMode: Text.Wrap
      textFormat: Text.PlainText
      text: (viewer.index + 1) + " of " + viewer.photos.length
        + (actions.status ? "  ·  " + actions.status : "")
        + (viewer.file && viewer.file.active ? "  ·  loading " + Math.round(Model.progress(viewer.file) * 100) + "%" : "")
        + "   " + Keymap.label(Keymap.keysFor(app.shortcuts, "photo.previous")[0] || "") + " "
        + Keymap.label(Keymap.keysFor(app.shortcuts, "photo.next")[0] || "") + " to step, "
        + Keymap.label(Keymap.keysFor(app.shortcuts, "photo.close")[0] || "") + " to close"
      color: Qt.rgba(1, 1, 1, 0.6)
      font.family: app.fontFamily
      font.pixelSize: Style.font.caption
    }
  }

  PhotoActions {
    id: actions
    anchors.fill: parent
    app: viewer.app
    client: viewer.app
    message: viewer.current
    file: viewer.file
    onDownloadRequested: function (fileId) { viewer.app.download(fileId, 32) }
  }
}
