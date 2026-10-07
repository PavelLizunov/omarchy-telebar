import QtQuick
import qs.Commons
import "../../shell" as Shell
import "Readiness.js" as Readiness

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
  property bool installedPalette: false
  property var peerStatus: null
  property bool compactMode: true
  property bool showMessageStart: false
  property bool longMessageReady: false
  property bool ready: state === "long-message" ? longMessageReady : quick.history.length >= 4
  property alias serviceModel: model
  property alias consumer: quick
  FixtureApp {
    id: model
    background: preview.installedPalette ? "#111c18" : preview.lightTheme ? "#f2f1ed" : "#101315"
    foreground: preview.installedPalette ? "#C1C497" : preview.lightTheme ? "#22282b" : "#cacccc"
    muted: preview.lightTheme ? "#59656a" : "#8a969f"
    accent: preview.installedPalette ? "#509475" : preview.lightTheme ? "#35676c" : "#8ab4b8"
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
    // The second text candidate is the host palette, not an unrelated dark fixture.
    Color.foreground = model.foreground
    if (preview.peerStatus) model.chats = model.chats.map(function (chat) { return chat.id === 101 ? Object.assign({}, chat, { status: preview.peerStatus }) : chat })
    quick.reset(101, 0)
    if (state === "attachments") quick.addAttachments([model.imagePath], true)
    else if (state === "recording") model.recording = { state: "voice", chatId: 101, startedAt: Date.now() }
    else if (state === "video-recording") model.recording = { state: "video", chatId: 101, startedAt: Date.now() }
    else if (state === "retained-rejected" || state === "retained-unknown") model.recordedSend = {
      token: "synthetic-token", kind: "video", chatId: 101, account: "default",
      state: state === "retained-rejected" ? "rejected" : "unknown" }
    else if (state === "long-message") Qt.callLater(function () {
      model.chats = model.chats.map(function (chat) { return chat.id === 101 ? Object.assign({}, chat, { status: { state: "online" } }) : chat })
      model.history = [{ id: 80, chatId: 101, date: 1790726000, outgoing: false, senderName: "Alex Demo",
        content: { kind: "text", text: "Select fragment from this message.\n" + Array(24).fill("A long message remains readable here.").join("\n") + "\nEND OF THE MESSAGE", entities: [] } }]
      quick.reset(101, 0)
    })
    else if (state === "long-draft") Qt.callLater(function () { quick.focusItem.text = Array(20).fill("Long draft line").join("\n"); quick.focusItem.cursorPosition = quick.focusItem.text.length })
    else if (state === "picker") Qt.callLater(function () {
      quick.attach()
      for (var object of quick.data) if (object.objectName === "quick-file-picker") object.navigate(Qt.resolvedUrl("dialog-files/"))
    })
  }
  Timer {
    interval: 20
    repeat: true
    running: preview.state === "long-message" && quick.history.length === 1 && !preview.longMessageReady
    property int frames: 0
    onTriggered: {
      if (++frames < 4) return
      if (preview.showMessageStart) {
        var list = Readiness.named(preview, "quick-message-list")
        var text = Readiness.named(preview, "quick-message-text-80")
        if (!list || !text) return
        list.positionViewAtBeginning()
        text.select(7, 15)
      }
      preview.longMessageReady = true
    }
  }
  Shell.QuickView { id: quick; objectName: "review-content"; anchors.fill: parent; anchors.margins: 12; compact: preview.compactMode; opened: true; service: model; background: model.background; foreground: model.foreground; accent: model.accent; urgent: model.urgent }
}
