import QtQuick
import QtQuick.Layouts
import qs.Commons
import "../../app" as App

Rectangle {
  id: preview
  width: 900
  height: 650
  color: model.background
  property int themeRadius: 0
  property int fontSize: 12
  property real sidebarWidth: 300
  property bool focusHandle: false
  property bool ready: false
  FixtureApp {
    id: model
    // Same translucent hover role as the native host.
    selected: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.08)
  }
  Component.onCompleted: {
    Style.fontBaseSize = fontSize
    Style.cornerRadius = themeRadius
    Qt.callLater(function () {
      if (focusHandle) buttonFocus.start()
      else ready = true
    })
  }
  Timer {
    id: buttonFocus
    interval: 1
    onTriggered: { split.forceButtonFocus(); preview.ready = true }
  }
  RowLayout {
    anchors.fill: parent
    spacing: 0
    App.ChatList {
      id: sidebar
      Layout.minimumWidth: Math.min(preview.sidebarWidth, preview.width * 0.45)
      Layout.preferredWidth: Layout.minimumWidth
      Layout.maximumWidth: Layout.minimumWidth
      Layout.fillHeight: true
      app: model
      menuHost: preview
      chats: model.chats
      allChats: model.chats
      tabs: [{ key: "main", title: "All" }]
      openChatId: 101
      nowMs: model.nowMs
    }
    App.SidebarHandle {
      id: split
      z: 20
      Layout.minimumWidth: 1
      Layout.preferredWidth: 1
      Layout.maximumWidth: 1
      Layout.fillHeight: true
      app: model
      compact: sidebar.compact
      currentWidth: sidebar.width
      maximumWidth: preview.width * 0.45
      onWidthEdited: function (value) { preview.sidebarWidth = value }
      onWidthCommitted: if (preview.sidebarWidth < 180) preview.sidebarWidth = 72
      onToggleRequested: preview.sidebarWidth = sidebar.compact ? 300 : 72
    }
    App.ChatView {
      Layout.fillWidth: true
      Layout.fillHeight: true
      app: model
      client: model
      chat: model.chats[0]
      history: [{ id: 1, chatId: 101, date: 1790726000, outgoing: false, senderName: "Alex Demo", sender: { type: "user", id: 101 },
        content: { kind: "text", text: "Click the handle to toggle the list. Drag the same handle to set its width.", entities: [] }, reactions: [] }]
    }
  }
}
