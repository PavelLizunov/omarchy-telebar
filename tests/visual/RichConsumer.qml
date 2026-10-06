import QtQuick
import qs.Commons
import "../../app" as App
import "../../shell" as Shell

Rectangle {
  id: fixture
  width: 760
  height: 650
  color: model.background
  property bool compact: false
  property int themeRadius: 0
  property bool partial: false
  property alias serviceModel: model
  property var richContent: ({ kind: "rich", text: "Before the image\n\nAfter the image", entities: [], full: !partial, rtl: false,
    blocks: [
      { kind: "text", text: "Before the image", entities: [{ type: "bold", offset: 0, length: 6 }] },
      { kind: "photo", text: "", entities: [], media: { width: 960, height: 640, file: { id: 50, size: 1500, path: model.imagePath, active: false } } },
      { kind: "text", text: "After the image. A safe link", entities: [{ type: "textUrl", offset: 17, length: 11, url: "https://example.org" }] }
    ] })
  property var sample: ({ id: 27, chatId: 101, date: 1790726000, senderName: "Synthetic post", sender: { type: "user", id: 101 },
                         outgoing: false, content: richContent, reactions: [] })
  FixtureApp { id: model; history: [fixture.sample] }
  property var fullReply: null
  property string richError: ""
  property alias consumer: scene.item
  property bool ready: scene.status === Loader.Ready && (!compact || scene.item.historyChatId === 101)
  Component.onCompleted: { Style.cornerRadius = themeRadius; Style.fontBaseSize = 12 }
  Loader {
    id: scene
    anchors.fill: parent
    sourceComponent: fixture.compact ? quickScene : rowScene
  }
  Component {
    id: rowScene
    App.MessageRow {
      width: fixture.width
      mid: 27
      index: 0
      messages: [fixture.sample]
      app: model
      view: rowView
    }
  }
  QtObject {
    id: rowView
    property var chat: model.chats[0]
    property var revealed: ({})
    property var selection: ({})
    property int cursor: 0
    property bool messagesFocused: false
    property real nowMs: model.nowMs
    property real confirmDeleteId: 0
    property bool threadOpen: false
    property real topicId: 0
    property bool selecting: false
    property var translations: ({})
    property var pollChoices: ({})
    function openLink(link) { model.lastRequest = { cmd: "link.open", args: { url: link } } }
    function reveal(id) { var next = {}; next[id] = true; revealed = next }
  }
  Component {
    id: quickScene
    Shell.QuickView {
      service: model
      opened: true
      compact: true
      Component.onCompleted: reset(101, 0)
    }
  }
}
