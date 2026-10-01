import QtQuick
import QtQuick.Layouts
import qs.Commons
import "../../app" as App
import "../../app/Icons.js" as Icons
import "../../shell" as Shell
import "Readiness.js" as Readiness

Rectangle {
  id: preview
  objectName: "review-root"
  width: 1100
  height: 760
  color: model.background
  property string scene: "chat"
  property int themeRadius: 0
  property int fontSize: 12
  property bool ready: false
  property string variant: ""
  FixtureApp { id: model }
  property var sampleMessages: [
    { id: 1, chatId: 101, date: 1790726000, outgoing: false, senderName: "Alex Demo", sender: { type: "user", id: 101 },
      content: { kind: "text", text: "The icons are vectors now. Try the panel button at the top.", entities: [] }, reactions: [] },
    { id: 2, chatId: 101, date: 1790726100, outgoing: true, senderName: "You", sender: { type: "user", id: 900 },
      content: { kind: "text", text: "Looks good. How does it fit in a narrow window?", entities: [{ type: "bold", offset: 0, length: 11 }] }, reactions: [] },
    { id: 3, chatId: 101, date: 1790726200, outgoing: false, senderName: "Alex Demo", sender: { type: "user", id: 101 },
      content: { kind: "text", text: "Long names, forum topics, account switching and error states should stay readable.", entities: [] }, reactions: [{ emoji: "👍", count: 2, chosen: false }] }
  ]
  Component.onCompleted: {
    Style.cornerRadius = preview.themeRadius
    Style.fontBaseSize = preview.fontSize
    model.history = preview.sampleMessages
    settle.restart()
    if (preview.scene === "long-chat") {
      var list = []
      for (var i = 0; i < 9; i++) list.push({ id: i+1, chatId: 101, date: 1790726000+i*30,
        outgoing: i % 3 === 2, senderName: "Alex Demo", sender: { type: "user", id: 101 }, reactions: [],
        content: { kind: "text", entities: [], text: i % 2 ? "Separate messages should remain easy to distinguish, even in a busy conversation." : "A longer paragraph wraps over several lines. Its line spacing must remain comfortable.\nThe next line still belongs to the same message." } })
      preview.sampleMessages = list
    }
  }
  Timer {
    id: settle
    interval: 20
    repeat: true
    property int settled: 0
    onTriggered: {
      if (!sceneLoader.item) return
      if (preview.scene === "quick-reply" && (sceneLoader.item.historyChatId !== 101 || !sceneLoader.item.history.length)) return
      if (Readiness.pending(preview)) { settled = 0; return }
      if (++settled < 4) return
      preview.ready = true
      stop()
    }
  }

  Text {
    z: 100
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.margins: 8
    text: "QML fixture · synthetic data · " + preview.scene
    color: model.muted
    font.pixelSize: 10
  }

  Loader {
    id: sceneLoader
    anchors.fill: parent
    onLoaded: if (item) item.objectName = "review-content"
    sourceComponent: preview.scene === "icons" ? iconSheet : preview.scene === "settings" ? settingsScreen
      : preview.scene === "login" || preview.scene === "password" ? loginScreen
      : preview.scene === "setup" ? setupScreen : preview.scene === "topics" || preview.scene === "topics-error" ? topicsScreen
      : preview.scene === "settings-connection" || preview.scene === "settings-shortcuts" ? settingsScreen
      : preview.scene === "new-chat" ? newChatScreen : preview.scene === "forward" ? forwardScreen
      : preview.scene === "poll" ? pollScreen : preview.scene === "info" ? infoScreen
      : preview.scene === "quick" || preview.scene === "quick-reply" ? quickScreen : preview.scene === "photo" ? photoScreen
      : preview.scene === "story" ? storyScreen : preview.scene === "media" ? mediaScreen
      : preview.scene === "emoji" ? emojiScreen : preview.scene === "stickers" ? stickersScreen
      : preview.scene === "menu" ? menuScreen : preview.scene === "people" ? peopleScreen
      : preview.scene === "call" ? callScreen : chatScreen
  }
  Component {
    id: iconSheet
    GridLayout {
      columns: 8
      anchors.fill: parent
      anchors.margins: 30
      rowSpacing: 12
      columnSpacing: 12
      Repeater {
        model: Object.keys(Icons.paths)
        Rectangle {
          Layout.fillWidth: true
          Layout.fillHeight: true
          color: "#192025"
          radius: 6
          App.Icon { anchors.horizontalCenter: parent.horizontalCenter; y: 10; name: modelData; color: "#a9d5d9"; size: 30; animated: false }
          Text { anchors.bottom: parent.bottom; anchors.bottomMargin: 10; anchors.horizontalCenter: parent.horizontalCenter; text: modelData; color: "#cacccc"; font.pixelSize: 11 }
        }
      }
    }
  }
  Component {
    id: chatScreen
    RowLayout {
      anchors.fill: parent
      spacing: 1
      App.ChatList { Layout.preferredWidth: preview.scene === "compact" ? 72 : 300; Layout.minimumWidth: Layout.preferredWidth; Layout.maximumWidth: Layout.preferredWidth; Layout.fillHeight: true; app: model; menuHost: preview; chats: model.chats; allChats: model.chats; tabs: [{ key: "main", title: "All" }, { key: "archive", title: "Archive" }]; openChatId: 101; nowMs: model.nowMs }
      App.ChatView { Layout.fillWidth: true; Layout.fillHeight: true; app: model; client: model; chat: model.chats[0]; history: preview.sampleMessages; nowMs: model.nowMs }
    }
  }
  Component {
    id: settingsScreen
    App.SettingsView {
      app: model
      visible: true
      Component.onCompleted: Qt.callLater(function () {
        var kind = preview.scene === "settings-connection" ? "bridge" : preview.scene === "settings-shortcuts" ? "action" : "profilePhoto"
        var index = rows.findIndex(function (r) { return r.kind === kind })
        if (index > 0) move(index - cursor)
      })
    }
  }
  Component { id: setupScreen; App.SetupView { app: model; client: model } }
  Component { id: loginScreen; App.LoginView { app: model; client: model; Component.onCompleted: {
    model.auth = preview.scene === "password" ? { state: "password", hint: "Fictional hint" }
      : preview.variant === "code" ? { state: "code", via: "TelegramMessage", phone: "+10000000000" }
      : preview.variant === "qr" || preview.variant === "qr-pending" ? { state: "qr", image: "", imagePending: preview.variant === "qr-pending" } : { state: "phone" }
    Qt.callLater(function () { if (preview.variant === "proxy") toggleProxy(); if (preview.variant === "error") error = "Synthetic sign-in failure" })
  } } }
  Component { id: topicsScreen; App.TopicList { app: model; client: model; chat: model.chats[1]; nowMs: model.nowMs; Component.onCompleted: if (preview.scene === "topics-error") Qt.callLater(function () { list = []; error = "Telegram temporarily unavailable"; loading = false }) } }
  Component { id: newChatScreen; App.NewChat { app: model; Component.onCompleted: {
    open()
    Qt.callLater(function () { if (preview.variant) showMode(preview.variant) })
  } } }
  Component { id: forwardScreen; App.ForwardPicker { app: model; chats: model.chats; multiple: true; Component.onCompleted: { open(101,[1,2]); chosen = [900] } } }
  Component { id: pollScreen; App.PollComposer { app: model; Component.onCompleted: { open("Community",false); quiz = preview.variant === "quiz" } } }
  Component { id: infoScreen; App.ChatInfo { app: model; client: model; chat: model.chats[0]; visible: true } }
  Component { id: quickScreen; Shell.QuickView { service: quickService; compact: true; opened: true; Component.onCompleted: if (preview.scene === "quick-reply") reset(101,0) } }
  QtObject {
    id: quickService
    property bool ready: true
    property bool connected: true
    property var shortcuts: ({})
    property var chats: model.chats
    property var recording: ({ state: "idle" })
    property var playing: ({ fileId: 0 })
    property var files: ({})
    property string activeAccount: "default"
    property real quickChatId: 0
    property string quickDraft: ""
    property real quickClosedAt: 0
    signal messageEvent(string name, var event)
    signal accountChanging()
    function request(cmd,args,callback) { model.request(cmd,args,callback) }
    function download() {}
  }
  Component { id: photoScreen; App.PhotoViewer { app: model; messages: [model.photoMessage]; messageId: 50 } }
  Component { id: storyScreen; App.StoryViewer { app: model; client: model; chatId: 101; storyId: 1; error: "Could not download the story" } }
  Component {
    id: mediaScreen
    Column {
      anchors.centerIn: parent
      width: 360
      height: implicitHeight
      spacing: 20
      App.MediaView { app: model; maxWidth: 360; message: model.photoMessage }
      App.MediaView { app: model; message: ({ id: 12, chatId: 101, content: { kind: "voice", media: { duration: 24, waveform: [3,12,20,31,12,7], file: { id: 12, size: 2345, path: "", active: false } } } }) }
      App.MediaView { app: model; message: ({ id: 13, chatId: 101, content: { kind: "file", media: { fileName: "preview-document.pdf", file: { id: 13, size: 100000, path: "", active: false } } } }) }
    }
  }
  Component { id: emojiScreen; App.EmojiPanel { app: model; visible: true; anchors.fill: parent; Component.onCompleted: {
    load("emoji", JSON.stringify({ items: [{ e: "😀", n: ["Smile"] }, { e: "👍", n: ["Thumbs up"] }, { e: "🎉", n: ["Party"] }, { e: "🐱", n: ["Cat"] }, { e: "🔥", n: ["Fire"] }, { e: "❤️", n: ["Heart"] }, { e: "🌿", n: ["Leaf"] }] })); open()
  } } }
  Component { id: stickersScreen; App.StickerPicker { app: model; visible: true; anchors.fill: parent } }
  Component { id: menuScreen; App.ContextMenu { app: model; anchors.fill: parent; items: [{ id: "reply", label: "Reply" }, { id: "forward", label: "Forward" }, { id: "delete", label: "Delete message", danger: true }]; reactions: ["👍","❤","🔥"]; Component.onCompleted: Qt.callLater(function () { open(300,200) }) } }
  Component { id: peopleScreen; App.PeopleList { app: model; anchors.fill: parent; Component.onCompleted: { open("People who reacted"); show([{ type: "user", id: 101, name: "Alex Demo", detail: "👍" }, { type: "user", id: 102, name: "Morgan Example", detail: "❤️" }]) } } }
  Component { id: callScreen; Item { App.CallBar { anchors.top: parent.top; width: parent.width; height: implicitHeight; app: model; call: ({ id: 1, name: "Long example caller name", video: true }) } } }
}
