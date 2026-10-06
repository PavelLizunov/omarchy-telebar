import QtQuick
import qs.Commons
import "Model.js" as Model

// Shared reading-order consumer. QuickView supplies still images and inert playback routing;
// only the separate application window loads MediaView/QtMultimedia.
Column {
  id: rich
  property var message
  property var client
  property var app
  property bool compact: false
  property bool animationsEnabled: false
  property color foreground: Color.foreground
  property color muted: Color.foreground
  property color accent: Color.accent
  property string fontFamily: Style.font.family
  property string linkHex: Model.hexOf(accent)
  property string codeBackground: "transparent"
  property bool revealed: false
  property var fetched: null
  property string error: ""
  property int serial: 0
  readonly property var source: message && message.content ? message.content : ({})
  readonly property var content: fetched || source
  readonly property var blocks: content.blocks || []
  readonly property bool loading: source.kind === "rich" && !source.full && !fetched && error === ""
  signal linkActivated(string link)
  signal mediaActivated(var message)
  signal revealRequested()
  spacing: Style.space(6)

  function load() {
    var token = ++serial
    fetched = null
    error = ""
    if (source.kind !== "rich" || source.full || !client || !message) return
    var account = client.activeAccount
    client.request("message.rich", { chatId: message.chatId, messageId: message.id }, function (answer) {
      if (!rich || token !== rich.serial || !rich.client || account !== rich.client.activeAccount) return
      if (answer.ok && answer.result && answer.result.full) rich.fetched = answer.result
      else rich.error = answer.error || "Could not load the full post"
    })
  }
  onSourceChanged: load()
  onClientChanged: load()
  Component.onDestruction: serial++
  Connections {
    target: rich.client
    function onActiveAccountChanged() { rich.serial++; rich.fetched = null; rich.error = "" }
  }

  Repeater {
    model: rich.blocks
    delegate: Column {
      id: block
      required property var modelData
      required property int index
      objectName: "rich-block-" + index
      width: rich.width
      spacing: Style.space(4)
      readonly property bool covered: !!modelData.spoiler && !rich.revealed
      readonly property var blockMessage: Model.blockMessage(rich.message, modelData)
      readonly property var media: modelData.media || null
      readonly property var file: media && rich.compact ? rich.client.fileOf(rich.client.stillOf(modelData.kind, media)) : null
      function fetch() {
        if (rich.compact && media) rich.client.fetchPicture(modelData.kind, media)
      }
      Component.onCompleted: fetch()
      onMediaChanged: fetch()

      Text {
        objectName: "rich-text-" + block.index
        width: parent.width
        visible: (block.modelData.text || "") !== ""
        text: Model.richText(block.modelData.text || "", block.modelData.entities, rich.revealed,
                             rich.codeBackground, !rich.compact && rich.app ? rich.app.customEmojiImages(Model.customEmojiIds(block.modelData.entities)) : null,
                             rich.linkHex)
        textFormat: Text.RichText
        wrapMode: Text.Wrap
        horizontalAlignment: rich.content.rtl ? Text.AlignRight : Text.AlignLeft
        color: block.modelData.kind === "unsupported" ? rich.muted : rich.foreground
        font.family: block.modelData.style === "pre" ? "monospace" : rich.fontFamily
        font.pixelSize: block.modelData.style === "heading" ? Style.font.title : rich.compact ? Style.font.bodySmall : Style.font.body
        font.bold: block.modelData.style === "heading"
        lineHeight: 1.22
        lineHeightMode: Text.ProportionalHeight
        onLinkActivated: function (link) { rich.linkActivated(link) }
        Component.onCompleted: if (!rich.compact && rich.app) rich.app.requestCustomEmoji(Model.customEmojiIds(block.modelData.entities))
        HoverHandler { cursorShape: parent.hoveredLink ? Qt.PointingHandCursor : Qt.ArrowCursor }
      }
      Loader {
        width: parent.width
        active: !!block.media && !rich.compact
        activeFocusOnTab: active
        Accessible.role: Accessible.Button
        Accessible.name: Model.contentLabel(block.modelData)
        Keys.onReturnPressed: if (item) item.activate()
        Keys.onSpacePressed: if (item) item.activate()
        Rectangle { anchors.fill: parent; color: "transparent"; radius: Style.cornerRadius; border.width: parent.activeFocus ? 1 : 0; border.color: rich.accent }
        function loadMedia() {
          if (!active || !rich.app) return
          setSource(Qt.resolvedUrl("MediaView.qml"), {
            app: rich.app, message: Qt.binding(function () { return block.blockMessage }),
            maxWidth: Qt.binding(function () { return block.width }),
            spoiler: Qt.binding(function () { return !!block.modelData.spoiler }),
            revealed: Qt.binding(function () { return rich.revealed }),
            animationsEnabled: Qt.binding(function () { return rich.animationsEnabled })
          })
        }
        Component.onCompleted: loadMedia()
        onActiveChanged: loadMedia()
        onLoaded: item.revealRequested.connect(function () { rich.revealRequested() })
      }
      Item {
        id: picture
        objectName: "rich-media-" + block.index
        visible: rich.compact && !!block.media
        readonly property bool visual: ["photo", "video", "gif"].indexOf(block.modelData.kind) >= 0
        readonly property var box: visual && block.media ? Model.fitSize(block.media.width || 320, block.media.height || 240,
                                        Math.min(block.width, Style.space(220)), Style.space(128)) : ({ width: block.width, height: Style.space(40) })
        width: box.width
        height: visible ? box.height : 0
        Image {
          anchors.fill: parent
          visible: picture.visual && !block.covered
          source: block.file && block.file.path ? Model.fileUrl(block.file.path) : Model.miniUrl(block.media ? block.media.mini : null)
          asynchronous: true
          fillMode: Image.PreserveAspectFit
          sourceSize.width: Math.round(width * 2)
        }
        Text {
          anchors.centerIn: parent
          visible: block.covered || !picture.visual || !block.file || !block.file.path || block.modelData.kind !== "photo"
          text: block.covered ? "Spoiler · click to show" : Model.contentLabel(block.modelData)
          textFormat: Text.PlainText
          color: rich.foreground
          font.family: rich.fontFamily
          font.pixelSize: Style.font.bodySmall
        }
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: block.covered ? "Show spoiler" : Model.contentLabel(block.modelData)
        function activate() { if (block.covered) rich.revealRequested(); else rich.mediaActivated(block.blockMessage) }
        Keys.onReturnPressed: activate()
        Keys.onSpacePressed: activate()
        Rectangle { anchors.fill: parent; color: "transparent"; radius: Style.cornerRadius; border.width: parent.activeFocus ? 1 : 0; border.color: rich.accent }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: picture.activate() }
      }
      Rectangle {
        visible: block.modelData.kind === "divider"
        width: parent.width
        height: visible ? 1 : 0
        color: rich.muted
        opacity: 0.4
      }
    }
  }
  Text {
    width: parent.width
    visible: rich.loading || rich.error !== "" || !!rich.content.truncated
    text: rich.error || (rich.loading ? "Loading full post…" : "Post exceeds display limits")
    textFormat: Text.PlainText
    wrapMode: Text.Wrap
    color: rich.muted
    font.family: rich.fontFamily
    font.pixelSize: Style.font.caption
  }
}
