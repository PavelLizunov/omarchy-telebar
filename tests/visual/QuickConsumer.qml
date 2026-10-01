import QtQuick
import qs.Commons
import "../../shell" as Shell

Rectangle {
  id: preview
  objectName: "review-root"
  width: 500
  height: 650
  color: model.background
  property int themeRadius: 0
  property int fontSize: 15
  property string state: "receipts"
  property bool lightTheme: false
  property bool compactMode: true
  property bool ready: quick.history.length >= 4
  property alias serviceModel: model
  property alias consumer: quick
  FixtureApp {
    id: model
    background: preview.lightTheme ? "#f2f1ed" : "#101315"
    foreground: preview.lightTheme ? "#22282b" : "#cacccc"
    muted: preview.lightTheme ? "#59656a" : "#8a969f"
    accent: preview.lightTheme ? "#35676c" : "#8ab4b8"
    urgent: preview.lightTheme ? "#943c3c" : "#dd8e8e"
    history: [
      { id: 1, chatId: 101, date: 1790726000, outgoing: true, content: { kind: "text", text: "This message was read.", entities: [] }, reactions: [] },
      { id: 20, chatId: 101, date: 1790726100, outgoing: true, content: { kind: "text", text: "Sent, waiting to be read.", entities: [] }, reactions: [] },
      { id: 21, chatId: 101, date: 1790726200, outgoing: true, sending: "pending", content: { kind: "text", text: "Sending a message.", entities: [] }, reactions: [] },
      { id: 22, chatId: 101, date: 1790726300, outgoing: true, sending: "failed", content: { kind: "text", text: "Could not send this message.", entities: [] }, reactions: [] },
      { id: 23, chatId: 101, date: 1790726350, outgoing: false, senderName: "Alex Demo", content: { kind: "text", text: "Reply here; Enter keeps the conversation open.", entities: [] }, reactions: [] }
    ]
  }
  Component.onCompleted: {
    Style.cornerRadius = themeRadius
    Style.fontBaseSize = fontSize
    quick.reset(101, 0)
    if (state === "attachments") quick.addAttachments([model.imagePath], true)
    else if (state === "recording") model.recording = { state: "voice", chatId: 101, startedAt: Date.now() }
    else if (state === "video-recording") model.recording = { state: "video", chatId: 101, startedAt: Date.now() }
    else if (state === "long-draft") Qt.callLater(function () { quick.focusItem.text = Array(20).fill("Long draft line").join("\n"); quick.focusItem.cursorPosition = quick.focusItem.text.length })
    else if (state === "picker") Qt.callLater(function () {
      quick.attach()
      for (var object of quick.data) if (object.objectName === "quick-file-picker") object.navigate(Qt.resolvedUrl("dialog-files/"))
    })
  }
  Shell.QuickView { id: quick; objectName: "review-content"; anchors.fill: parent; anchors.margins: 12; compact: preview.compactMode; opened: true; service: model; background: model.background; foreground: model.foreground; accent: model.accent; urgent: model.urgent }
}
