import QtQuick
import qs.Commons
import "../../app" as App

Rectangle {
  id: fixture
  objectName: "emoji-chat-root"
  width: 520
  height: 400
  color: model.background
  property int fontSize: 12
  property int themeRadius: 0
  property bool openPicker: true
  property bool longDraft: false
  property alias chat: chat
  property alias model: model
  property bool ready: false
  FixtureApp { id: model; function saveEmojiState(state) { emojiState = state } }
  App.ChatView {
    id: chat
    objectName: "emoji-chat"
    anchors.fill: parent
    app: model
    client: model
    chat: model.chats[0]
    history: [
      { id: 1, chatId: 101, date: 1790726000, senderName: "Alex Demo", outgoing: false,
        content: { kind: "text", text: "This chat must stay reachable while emoji are open.", entities: [] }, reactions: [] },
      { id: 2, chatId: 101, date: 1790726100, outgoing: true,
        content: { kind: "text", text: "A small window should not require full screen.", entities: [] }, reactions: [] }
    ]
  }
  Component.onCompleted: {
    Style.fontBaseSize = fontSize
    Style.cornerRadius = themeRadius
    chat.setComposerText(longDraft ? Array(12).fill("Synthetic draft line").join("\n") : "Synthetic unsent draft")
    chat.emoji.objectName = "chat-emoji-panel"
    chat.emoji.load("emoji", JSON.stringify({ items: [{ e: "😀", n: ["Smile"] }, { e: "👍", n: ["Thumbs up"] }, { e: "🎉", n: ["Party"] }, { e: "🐱", n: ["Cat"] }, { e: "🔥", n: ["Fire"] }, { e: "❤️", n: ["Heart"] }, { e: "🌿", n: ["Leaf"] }] }))
    if (openPicker) chat.openEmoji()
  }
  Timer { interval: 20; running: true; repeat: true; property int ticks: 0; onTriggered: { if (++ticks < 5) return; fixture.ready = true; stop() } }
}
