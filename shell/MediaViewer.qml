import QtQuick
import QtQuick.Layouts
import qs.Commons
import "../app" as App
import "../app/Model.js" as Model
import "../app/Keymap.js" as Keymap

// A photo, video or GIF from the quick view, over the whole screen. ←/→ or h/l step through the ones the quick
// view has loaded; Enter plays a video or GIF in your own video player, since the shell loads no media player;
// o opens the chat in the window; Esc or q closes, and so does a click beside the picture.
FocusScope {
  id: viewer

  property var host: null             // the quick view this belongs to: its service, files, keys and words
  property var items: []              // the quick view's messages with a photo, video or GIF, oldest first
  property real messageId: 0
  property real fileId: 0
  property real waitingFileId: 0      // a video fetched to play the moment it is here
  property string error: ""
  property alias actions: actions

  signal closed()
  signal played()                     // a video went to the video player: the quick view gets out of its way
  signal openInWindowRequested(real chatId)

  readonly property int index: {
    for (var i = 0; i < viewer.items.length; i++) if (viewer.items[i].id === viewer.messageId && (!viewer.fileId || viewer.items[i].content.media.file.id === viewer.fileId)) return i
    return -1
  }
  readonly property var current: viewer.index >= 0 ? viewer.items[viewer.index] : null
  readonly property string kind: viewer.current ? viewer.current.content.kind : ""
  readonly property var media: viewer.current ? viewer.current.content.media : null
  readonly property bool video: viewer.kind === "video" || viewer.kind === "gif"
  readonly property var file: viewer.media && viewer.host ? viewer.host.fileOf(viewer.media.file) : null
  readonly property var keys: viewer.host ? viewer.host.keys : ({})
  readonly property string fontFamily: viewer.host ? viewer.host.fontFamily : Style.font.family

  property string fetchedFor: ""         // the message whose picture was last asked for

  // Asked for the moment a message is chosen, and again when the list it is in first arrives.
  onMessageIdChanged: {
    viewer.error = ""
    viewer.fetch()
  }
  onFileIdChanged: viewer.fetch()
  onItemsChanged: viewer.fetch()
  Component.onCompleted: viewer.fetch()
  onFileChanged: {
    if (viewer.waitingFileId && viewer.file && viewer.file.id === viewer.waitingFileId && viewer.file.path) viewer.openInPlayer()
  }
  onVisibleChanged: if (visible) viewer.forceActiveFocus()

  // A photo comes whole as soon as it shows; a video only when it is to be played, its still picture meanwhile.
  // Found from the list itself: inside a change handler, `current` can still hold the message before.
  function fetch() {
    var message = null
    for (var i = 0; i < viewer.items.length; i++) if (viewer.items[i].id === viewer.messageId && (!viewer.fileId || viewer.items[i].content.media.file.id === viewer.fileId)) message = viewer.items[i]
    if (!message || !viewer.host) return
    var key = message.id + ":" + message.content.media.file.id
    if (viewer.fetchedFor === key) return
    viewer.fetchedFor = key
    var media = message.content.media
    if (message.content.kind === "photo") viewer.host.fetchNow(media.file)
    else viewer.host.fetch(viewer.host.stillThumb(media))
  }

  function step(delta) {
    var next = viewer.index + delta
    if (viewer.index < 0 || next < 0 || next >= viewer.items.length) return
    viewer.waitingFileId = 0
    viewer.fileId = viewer.items[next].content.media.file.id
    viewer.messageId = viewer.items[next].id
    viewer.fetch()
  }

  function play() {
    if (!viewer.video || !viewer.file || !viewer.host) return
    viewer.error = ""
    viewer.waitingFileId = viewer.file.id
    if (viewer.file.path) viewer.openInPlayer()
    else viewer.host.fetchNow(viewer.media.file)
  }

  function openInPlayer() {
    var fileId = viewer.waitingFileId
    viewer.waitingFileId = 0
    viewer.host.service.request("file.open", { fileId: fileId }, function (answer) {
      if (answer.ok) viewer.played()
      else viewer.error = answer.error || "It could not be played"
    })
  }

  Keys.onPressed: function (event) {
    function is(id) { return Keymap.matches(viewer.keys, id, event) }
    if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && event.modifiers === Qt.ShiftModifier)) actions.openMenu(viewer.width / 2, viewer.height / 2)
    else if (is("quickMedia.close")) viewer.closed()
    else if (is("quickMedia.previous")) viewer.step(-1)
    else if (is("quickMedia.next")) viewer.step(1)
    else if (is("quickMedia.play")) viewer.play()
    else if (is("quickMedia.openInWindow")) {
      if (viewer.current) viewer.openInWindowRequested(viewer.current.chatId)
    }
    else return
    event.accepted = true
  }

  Rectangle {
    anchors.fill: parent
    color: Qt.rgba(0, 0, 0, 0.95)
  }

  MouseArea {
    anchors.fill: parent
    onClicked: viewer.closed()
  }

  Item {
    anchors.fill: parent
    anchors.leftMargin: Style.space(48)
    anchors.rightMargin: Style.space(48)
    anchors.topMargin: Style.space(60)
    anchors.bottomMargin: Style.space(96)

    // Its tiny blurred copy, then the small preview, until the whole picture is here.
    Image {
      anchors.fill: parent
      visible: preview.status !== Image.Ready && picture.status !== Image.Ready
      source: viewer.media ? Model.miniUrl(viewer.media.mini) : ""
      fillMode: Image.PreserveAspectFit
      smooth: true
    }
    Image {
      id: preview
      anchors.fill: parent
      visible: picture.status !== Image.Ready
      asynchronous: true
      fillMode: Image.PreserveAspectFit
      source: viewer.media && viewer.host && viewer.kind === "photo" && viewer.media.preview ? viewer.host.urlOf(viewer.media.preview.file) : ""
    }
    Image {
      id: picture
      anchors.fill: parent
      asynchronous: true
      cache: false
      sourceSize.width: Math.min(4096, Math.ceil(viewer.width * 1.5 / 256) * 256)
      sourceSize.height: Math.min(4096, Math.ceil(viewer.height * 1.5 / 256) * 256)
      autoTransform: true
      fillMode: Image.PreserveAspectFit
      source: !viewer.media || !viewer.host ? ""
            : (viewer.kind === "photo" ? viewer.host.urlOf(viewer.media.file) : viewer.host.urlOf(viewer.host.stillThumb(viewer.media)))
    }

    // A video or GIF: played in your video player.
    Rectangle {
      visible: viewer.video
      anchors.centerIn: parent
      width: Style.space(88)
      height: width
      radius: width / 2
      color: Qt.rgba(0, 0, 0, playArea.containsMouse ? 0.75 : 0.55)
      border.width: Math.max(1, Style.space(2))
      border.color: Qt.rgba(1, 1, 1, 0.85)

      // md-play U+F040A
      App.Icon {
        anchors.centerIn: parent
        anchors.horizontalCenterOffset: Style.space(3)
        name: "play"
        color: "white"
        size: Style.space(44)
      }
      MouseArea {
        id: playArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: viewer.play()
      }
    }
  }

  RowLayout {
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.leftMargin: Style.space(24)
    anchors.rightMargin: Style.space(24)
    anchors.topMargin: Style.space(18)
    spacing: Style.space(12)

    Text {
      Layout.fillWidth: true
      text: !viewer.current ? ""
          : (viewer.current.outgoing ? "You" : (viewer.current.senderName || (viewer.host && viewer.host.shownChat ? viewer.host.shownChat.title : "")))
            + "  ·  " + Model.clock(viewer.current.date)
      textFormat: Text.PlainText
      elide: Text.ElideRight
      color: Qt.rgba(1, 1, 1, 0.85)
      font.family: viewer.fontFamily
      font.pixelSize: Style.font.body
    }
    // md-open-in-new U+F03CC: the chat, in the window
    App.Icon {
      name: "window"
      color: openArea.containsMouse ? "white" : Qt.rgba(1, 1, 1, 0.65)
      size: Style.font.title

      MouseArea {
        id: openArea
        anchors.fill: parent
        anchors.margins: -Style.space(6)
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: if (viewer.current) viewer.openInWindowRequested(viewer.current.chatId)
      }
    }
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
      font.family: viewer.fontFamily
      font.pixelSize: Style.font.body
    }
    Text {
      width: parent.width
      horizontalAlignment: Text.AlignHCenter
      wrapMode: Text.Wrap
      textFormat: Text.PlainText
      text: {
        if (!viewer.current || !viewer.host) return ""
        var parts = [(viewer.index + 1) + " of " + viewer.items.length]
        if (viewer.video) parts.push((viewer.kind === "gif" ? "GIF" : "Video") + " " + viewer.host.duration(viewer.media.duration))
        if (actions.status) parts.push(actions.status)
        if (viewer.error) parts.push(viewer.error)
        else if (viewer.waitingFileId && viewer.file) parts.push("getting it ready " + Math.round(Model.progress(viewer.file) * 100) + "%")
        else if (viewer.kind === "photo" && viewer.file && viewer.file.active) parts.push("loading " + Math.round(Model.progress(viewer.file) * 100) + "%")
        var keys = viewer.host.hints(["quickMedia.previous", "back", "quickMedia.next", "next"]
                                     .concat(viewer.video ? ["quickMedia.play", "plays"] : [])
                                     .concat(["quickMedia.openInWindow", "opens the chat", "quickMedia.close", "closes"]))
        return parts.join("  ·  ") + (keys ? "     " + keys : "")
      }
      color: Qt.rgba(1, 1, 1, 0.6)
      font.family: viewer.fontFamily
      font.pixelSize: Style.font.caption
    }
  }

  // ContextMenu expects the window's theme/shortcut contract, adapted to the quick host.
  QtObject {
    id: menuTheme
    readonly property var shortcuts: viewer.keys
    readonly property string fontFamily: viewer.fontFamily
    readonly property color background: viewer.host ? viewer.host.background : Color.menu.background
    readonly property color foreground: viewer.host ? viewer.host.foreground : Color.menu.text
    readonly property color selected: viewer.host ? viewer.host.selected : Color.menu.selectedBackground
    readonly property color accent: viewer.host ? viewer.host.accent : Color.accent
    readonly property color urgent: viewer.host ? viewer.host.urgent : Color.urgent
  }
  App.PhotoActions {
    id: actions
    anchors.fill: parent
    visible: viewer.kind === "photo"
    app: menuTheme
    client: viewer.host ? viewer.host.service : null
    message: viewer.current
    file: viewer.file
    onDownloadRequested: function (fileId) { viewer.host.fetchNow(viewer.media.file) }
  }
}
