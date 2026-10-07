import QtQuick
import qs.Commons
import "../../app" as App
import "Readiness.js" as Readiness

Rectangle {
  id: preview
  objectName: "send-contract-root"
  width: 700
  height: 650
  color: model.background
  property int themeRadius: 0
  property string scenario: "rejected"
  property bool files: false
  property bool editing: false
  property bool voice: false
  property bool auxiliary: false
  property string pickerKind: ""
  property bool video: false
  property string retainedState: ""
  property bool ready: false
  FixtureApp { id: model }
  App.ChatView {
    id: view
    objectName: "send-contract-chat"
    anchors.fill: parent
    app: model
    client: model
    chat: model.chats[0]
    history: [model.photoMessage]
    onNoticeChanged: if (preview.scenario === "rejected" && notice.indexOf("Synthetic rejection") >= 0) settle.start()
  }
  Component.onCompleted: {
    Style.cornerRadius = preview.themeRadius
    Style.fontBaseSize = 12
    view.setComposerText("Synthetic composition retained until sending succeeds")
    view.replyToId = model.photoMessage.id
    if (preview.editing) {
      view.startEdit({ id: model.photoMessage.id, chatId: model.photoMessage.chatId, outgoing: true,
                       content: { kind: "text", text: "Original synthetic message", entities: [] } }, true)
      view.setComposerText("Synthetic edit retained until Telegram accepts it")
    }
    if (preview.files) view.addAttachments([model.imagePath], false)
    if (preview.retainedState) {
      model.recordedSend = { token: "synthetic-token", state: preview.retainedState, kind: "video",
                             chatId: view.chat.id, account: "default" }
      settle.start()
      return
    }
    model.holdSend = preview.scenario === "pending"
    model.sendError = preview.scenario === "rejected" ? "Synthetic rejection" : ""
    if (preview.voice) {
      model.recording = { state: "voice", chatId: view.chat.id, startedAt: Date.now() }
      view.stopVoice(true)
    } else if (preview.video) {
      var recorder = Readiness.named(view, "chat-video-recorder")
      recorder.visible = false
      view.openVideo()
      var context = recorder.sendContext
      recorder.phase = "closed"
      recorder.recorded(context.chatId, "/synthetic/recordings/video.mp4", context)
    } else if (preview.pickerKind === "sticker") {
      Readiness.named(view, "chat-sticker-picker").picked({ file: { id: 456 }, width: 128, height: 128, emoji: "" })
    } else if (preview.pickerKind === "saved-gif") {
      view.sendGif({ gif: { file: { id: 789 }, width: 320, height: 240, duration: 2 } })
    } else if (preview.pickerKind === "inline-gif") {
      view.sendGif({ gif: {}, queryId: "12345", resultId: "synthetic-result" })
    } else if (preview.auxiliary) view.sendExtra("message.sendDice", { emoji: "🎲" }, "the dice")
    else view.send()
    if (preview.scenario === "pending") settle.start()
  }
  Timer {
    id: settle
    interval: 20
    repeat: true
    property int frames: 0
    onTriggered: {
      if (Readiness.pending(preview)) { frames = 0; return }
      if (++frames < 4) return
      preview.ready = Readiness.matchesRadius(view, preview.voice ? "voice-recording-bar" : "composer-box", preview.themeRadius)
      stop()
    }
  }
}
