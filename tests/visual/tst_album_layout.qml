import QtQuick
import QtTest
import qs.Commons
import "../../app" as App

TestCase {
  id: tests
  name: "AlbumCreationHeight"
  when: windowShown
  visible: true
  width: 700
  height: 800
  FixtureApp { id: model }
  App.ChatView { id: chatView; width: 640; height: 760; app: model; client: model; chat: model.chats[0] }
  Component { id: scene; App.MessageRow { width: 600; mid: 1; index: 0; app: model; view: chatView } }
  function init() { failOnWarning(/.*/) }
  function cleanup() { Style.cornerRadius = 0 }
  function test_final_height_at_creation_data() {
    return [{ tag: "square", radius: 0 }, { tag: "rounded", radius: 8 }]
  }
  function test_final_height_at_creation(data) {
    Style.cornerRadius = data.radius
    var messages = []
    for (var i = 0; i < 10; i++) messages.push({
      id: i + 1, chatId: 101, date: 1790726000 + i, outgoing: false,
      albumId: "synthetic-album", reactions: [],
      content: { kind: "photo", text: "", entities: [], media: {
        file: { id: i + 1, path: "", size: 100, active: false }, width: 720, height: 1280
      } }
    })
    var row = createTemporaryObject(scene, tests, { messages: messages })
    verify(row !== null)
    var initialHeight = row.height
    wait(100)
    compare(initialHeight, row.height,
      "An album must have its settled height before ListView measures its delegate")
  }
}
