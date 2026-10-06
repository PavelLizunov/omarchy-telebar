import QtQuick
import qs.Commons
import "../../app" as App
import "Readiness.js" as Readiness

Rectangle {
  id: preview
  objectName: "mention-contract-root"
  width: 700
  height: 650
  color: model.background
  property int themeRadius: 0
  property bool ready: false
  FixtureApp { id: model }
  App.ChatView {
    id: view
    objectName: "mention-contract-chat"
    anchors.fill: parent
    app: model
    client: model
    chat: model.chats[1]
    history: [{ id: 77, chatId: -102, date: 1790726000, outgoing: false,
      senderName: "Synthetic sender", sender: { type: "user", id: 101 }, reactions: [],
      content: { kind: "text", text: "Synthetic new-topic history. Old mention navigation must be cancelled.", entities: [] } }]
  }
  Component.onCompleted: {
    Style.cornerRadius = preview.themeRadius
    Style.fontBaseSize = 12
    model.openTopic = model.topicData[0]
    view.pendingMention = 77
    view.mentionBusy = true
    model.openTopic = model.topicData[1]
    view.setComposerText("Synthetic new-topic draft")
    settle.start()
  }
  Timer {
    id: settle
    interval: 20
    repeat: true
    property int frames: 0
    onTriggered: {
      if (Readiness.pending(preview)) { frames = 0; return }
      if (++frames < 4) return
      preview.ready = view.pendingMention === 0 && !view.mentionBusy && view.topicId === 2
        && Readiness.matchesRadius(view, "composer-box", preview.themeRadius)
      stop()
    }
  }
}
