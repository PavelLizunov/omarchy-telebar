import QtQuick
import QtQuick.Layouts
import qs.Commons
import "../../app" as App

Rectangle {
  id: preview
  objectName: "review-root"
  width: 552
  height: 1020
  color: model.background
  property string state: "normal"
  property int themeRadius: 8
  property bool ready: false
  FixtureApp {
    id: model
    fontFamily: "monospace"
    background: "#2e3440"
    foreground: "#d8dee9"
    muted: "#8e9aab"
    accent: "#81a1c1"
    accentText: "#88c0d0"
  }
  Component.onCompleted: {
    Style.fontBaseSize = 15
    Style.cornerRadius = preview.themeRadius
    Qt.callLater(function () {
      view.setComposerText(preview.state === "draft" ? "Проверяем длинный черновик при изменении размера окна." : "")
      if (preview.state === "info") view.openInfo()
      else if (preview.state === "selection") view.selection = { "1": true }
      else if (preview.state === "prompt") view.prompt = { text: "Delete the selected message for everyone?", action: "Delete" }
      else if (preview.state === "attachments") view.addAttachments([model.imagePath], true)
      else if (preview.state === "recording") model.recording = { state: "voice", chatId: 101, startedAt: Date.now() }
      else if (preview.state === "reply") view.startReply(view.history[0])
      else if (preview.state === "edit") view.startEdit(view.history[1])
      else if (preview.state === "emoji") view.openEmoji()
      else if (preview.state === "stickers") view.toggleStickers()
      else if (preview.state === "date") view.openDateBar()
      else if (preview.state === "location") view.openLocationBar()
      else if (preview.state === "scheduled") view.openScheduled()
      else if (preview.state === "scheduled-loading") { view.scheduledOpen = true; view.scheduledLoading = true }
      else if (preview.state === "scheduled-populated") { view.scheduledOpen = true; view.scheduledMessages = view.history }
      else if (preview.state === "pinned") view.pinnedMessage = view.history[0]
      else if (preview.state === "link-preview") { view.linkPreview = { title: "Synthetic link preview", siteName: "Example", description: "Long description for checking layout", url: "https://example.invalid/" }; view.setComposerText("https://example.invalid/") }
      view.focusComposer()
      if (preview.state === "menu") view.composerAction("overflow", null)
      else if (preview.state === "send-menu") { view.setComposerText("Synthetic draft"); view.openSendMenu(null) }
      else if (preview.state === "more-menu") view.openMoreMenu(null)
      else if (preview.state === "dice-menu") view.morePicked("dice")
      else if (preview.state === "contact-picker") view.morePicked("contact")
      preview.ready = true
    })
  }
  RowLayout {
    anchors.fill: parent
    spacing: 0
    App.ChatList { id: sidebar; Layout.preferredWidth: 90; Layout.minimumWidth: 90; Layout.maximumWidth: 90; Layout.fillHeight: true; app: model; menuHost: preview; chats: model.chats; allChats: model.chats; openChatId: 101; nowMs: model.nowMs }
    App.SidebarHandle { Layout.preferredWidth: 1; Layout.minimumWidth: 1; Layout.maximumWidth: 1; Layout.fillHeight: true; z: 20; app: model; compact: sidebar.compact; currentWidth: sidebar.width; maximumWidth: preview.width * 0.45 }
    App.ChatView {
      id: view
      objectName: "review-content"
      Layout.fillWidth: true
      Layout.fillHeight: true
      app: model
      client: model
      nowMs: model.nowMs
      chat: ({ id: 101, title: "Длинное название группы · комментарии и обсуждения", kind: "group", myStatus: "member", memberCount: 842, unread: 0, mentions: 0, draft: "", hasScheduled: true })
      history: [
        { id: 1, chatId: 101, date: 1790726000, outgoing: false, senderName: "Очень длинное имя участника обсуждения", sender: { type: "user", id: 101 }, content: { kind: "text", text: "В узком окне текст должен занимать доступную ширину, а действия не должны вытеснять поле ввода.", entities: [] }, reactions: [{ emoji: "👍", count: 1 }] },
        { id: 2, chatId: 101, date: 1790726100, outgoing: true, senderName: "You", sender: { type: "user", id: 900 }, content: { kind: "text", text: "Черновик сохраняется при изменении ширины.", entities: [] }, reactions: [] }
      ]
    }
  }
}
