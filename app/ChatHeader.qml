import QtQuick
import QtQuick.Layouts
import qs.Commons
import "Model.js" as Model
import "Keymap.js" as Keymap

Rectangle {
  required property var root
  required property var app
  readonly property var buttons: headerButtons
  Layout.fillWidth: true
  Layout.preferredHeight: Style.space(56)
  color: "transparent"

  // md-arrow-left U+F004D: from a topic back to the forum's topics, from scheduled messages to the chat
  Item {
    id: backButton
    readonly property bool shown: root.topicId > 0 || root.scheduledOpen
    anchors.left: parent.left
    anchors.leftMargin: Style.space(8)
    anchors.verticalCenter: parent.verticalCenter
    width: backButton.shown ? Style.space(32) : 0
    height: Style.space(32)
    visible: backButton.shown

    Icon {
      anchors.centerIn: parent
      name: "back"
      color: backArea.containsMouse ? app.foreground : app.muted
      size: Style.font.title
    }
    MouseArea {
      id: backArea
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: {
        if (!root.scheduledOpen) { app.closeTopic(); return }
        root.closeScheduled()
        root.focusComposer()
      }
      onContainsMouseChanged: if (containsMouse) root.flash(root.scheduledOpen ? "Back to the chat   " + Keymap.label(Keymap.keysFor(app.shortcuts, "messages.toComposer")[0] || "")
                                                            : (root.threadOpen ? "Back to the message   " : "Back to the topics   ")
                                                              + Keymap.label(Keymap.keysFor(app.shortcuts, "window.topicList")[0] || ""))
    }
  }

  Avatar {
    id: headerAvatar
    anchors.left: backButton.right
    anchors.leftMargin: backButton.shown ? Style.space(4) : Style.space(10)
    anchors.verticalCenter: parent.verticalCenter
    app: root.app
    chat: root.chat
    size: Style.space(38)
  }

  Column {
    anchors.left: headerAvatar.right
    anchors.leftMargin: Style.space(12)
    anchors.right: headerButtons.left
    anchors.rightMargin: Style.space(8)
    anchors.verticalCenter: parent.verticalCenter
    spacing: Style.space(2)

    RowLayout {
      width: parent.width
      spacing: Style.space(8)
      Text {
        objectName: "chat-title"
        Layout.preferredWidth: implicitWidth
        Layout.minimumWidth: 0
        Layout.maximumWidth: implicitWidth
        elide: Text.ElideRight
        text: root.scheduledOpen ? "Scheduled messages" : (root.topicId ? app.openTopic.name : (root.chat ? Model.chatTitle(root.chat, app.meId) : ""))
        textFormat: Text.PlainText
        color: app.foreground
        font.family: app.fontFamily
        font.pixelSize: Style.font.title
        font.bold: true
      }
      Text {
        objectName: "chat-online-status"
        visible: !root.scheduledOpen && !root.topicId && root.chat && ["private", "secret"].indexOf(root.chat.kind) >= 0 && peerSubtitle.peerStatus === "online"
        text: "online"
        color: app.accentText
        font.family: app.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
      }
      Item { Layout.fillWidth: true }
    }
    Text {
      id: peerSubtitle
      objectName: "chat-peer-status"
      visible: text !== "" && (text !== "online" || root.scheduledOpen || root.topicId)
      readonly property string peerStatus: root.chat && root.chat.userId !== app.meId && !root.chat.bot
          ? Model.statusText(app.userStatuses[root.chat.userId] || root.chat.status, root.nowMs) : ""
      readonly property string activity: !root.chat || root.scheduledOpen ? ""
          : Model.actionText(Model.activeActions(app.chatActions, root.chat.id, app.clockMs), root.chat.kind === "private" || root.chat.kind === "secret")
      width: parent.width
      elide: Text.ElideRight
      text: !root.chat ? "" : (activity
          || (root.scheduledOpen ? Model.chatTitle(root.chat, app.meId)
              : (root.topicId ? (root.threadOpen ? (app.openTopic.subtitle || Model.chatTitle(root.chat, app.meId))
                                                 : "Topic in " + Model.chatTitle(root.chat, app.meId))
                 : (root.chat.kind === "private"
                    ? (root.chat.userId === app.meId ? "" : (root.chat.bot ? "bot" : peerStatus))
                    : (root.chat.kind === "secret" ? "Secret chat: " + Model.secretStateText(root.chat) + (peerStatus ? " · " + peerStatus : "")
                       : (root.forum ? "Topics" : Model.memberCountText(root.chat.memberCount, root.chat.kind === "channel")))))))
      textFormat: Text.PlainText
      color: activity || peerStatus === "online" ? app.accentText : app.muted
      font.family: app.fontFamily
      font.pixelSize: Style.font.bodySmall
    }
  }

  // Search in this chat; its notifications.
  Row {
    id: headerButtons
    anchors.right: parent.right
    anchors.rightMargin: Style.space(10)
    anchors.verticalCenter: parent.verticalCenter

    Repeater {
      // md-magnify U+F0349; md-bell-outline U+F009C, md-bell-off U+F009B; md-information-outline U+F02FD
      // md-calendar-clock U+F00F0: the chat has scheduled messages
      model: (root.chat && root.chat.hasScheduled && !root.scheduledOpen && !root.compactControls ? [
        { glyph: String.fromCodePoint(0xF00F0), action: "scheduled", hint: "Scheduled messages" }
      ] : []).concat([
        { glyph: String.fromCodePoint(0xF0349), action: "search",
          hint: "Search in this chat   " + Keymap.label(Keymap.keysFor(app.shortcuts, "window.searchInChat")[0] || "") },
        { glyph: String.fromCodePoint(0xF02FD), action: "info",
          hint: "The chat's info   " + Keymap.label(Keymap.keysFor(app.shortcuts, "window.chatInfo")[0] || "") },
        { glyph: String.fromCodePoint(root.chat && root.chat.muted ? 0xF009B : 0xF009C), action: "mute",
          hint: (root.chat && root.chat.muted ? "Muted" : "Notifications are on") + "   " + Keymap.label(Keymap.keysFor(app.shortcuts, "window.mute")[0] || "") }
      ]).filter(function (entry) {
        return !root.compactControls || entry.action === "search" || (entry.action === "info" && !root.minimalControls)
      }).concat(root.compactControls ? [{ glyph: String.fromCodePoint(0xF01D8), action: "overflow", hint: "Chat actions" }] : [])
      delegate: Item {
        id: headerButton
        required property var modelData
        objectName: "header-action-" + modelData.action
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: modelData.hint
        width: Style.space(36)
        height: Style.space(36)
        function activate() {
          var action = modelData.action
          if (action === "search") root.searchInChatRequested()
          else if (action === "info") root.toggleInfo()
          else if (action === "scheduled") root.openScheduled()
          else if (action === "overflow") {
            var at = mapToItem(root, 0, height)
            headerMenu.open(at.x, at.y)
          }
          else root.openMuteMenu(headerButton)
        }
        Keys.onReturnPressed: activate()
        Keys.onSpacePressed: activate()
        Rectangle { anchors.fill: parent; radius: Style.cornerRadius; color: "transparent"; border.width: parent.activeFocus ? 1 : 0; border.color: app.accent }

        Icon {
          anchors.centerIn: parent
          glyph: headerButton.modelData.glyph
          color: headerArea.containsMouse ? app.foreground : app.muted
          size: Style.font.title
        }
        MouseArea {
          id: headerArea
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: headerButton.activate()
          onContainsMouseChanged: if (containsMouse) root.flash(headerButton.modelData.hint)
        }
      }
    }
  }

  Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: app.border; opacity: 0.35 }
}
