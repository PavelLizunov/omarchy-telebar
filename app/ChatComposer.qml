import QtQuick
import QtQuick.Layouts
import qs.Commons
import "Model.js" as Model
import "Keymap.js" as Keymap

Rectangle {
  required property var root
  required property var app
  required property var client
  required property var videoNote
  readonly property var input: composer
  readonly property var buttons: composerButtons
  Layout.fillWidth: true
  // With scheduled messages listed, only while one of them is being edited.
  visible: !root.showTopics && (!root.scheduledOpen || root.editingId > 0) && root.composerBlock === ""
  // 20 of outer margin and 16 of inner padding around the text, plus room for the caret.
  Layout.preferredHeight: Math.min(Style.space(180), composer.implicitHeight + Style.space(40))
  color: "transparent"

  Rectangle { width: parent.width; height: 1; color: app.border; opacity: 0.35 }

  Rectangle {
    objectName: "composer-box"
    visible: !root.recordingVoice && !root.secretBlocked
    anchors.left: parent.left
    anchors.right: composerButtons.left
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.margins: Style.space(10)
    radius: Style.cornerRadius
    readonly property bool silent: !!root.chat && root.chat.silent === true
    color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, silent ? 0.02 : 0.05)
    border.width: Math.max(1, Style.space(1.5))
    // Silent sending draws the box as an outline, focused or not: a message without sound is never a surprise.
    border.color: silent ? Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, composer.activeFocus ? 0.55 : 0.3)
                         : (composer.activeFocus ? app.accent : "transparent")

    Rectangle {
      id: silentChip
      visible: parent.silent
      anchors.left: parent.left
      anchors.top: parent.top
      anchors.leftMargin: Style.space(8)
      anchors.topMargin: Style.space(7)
      width: silentChipRow.implicitWidth + Style.space(14)
      height: silentChipRow.implicitHeight + Style.space(6)
      radius: Style.cornerRadius
      color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.08)

      Row {
        id: silentChipRow
        anchors.centerIn: parent
        spacing: Style.space(5)

        Icon {
          anchors.verticalCenter: parent.verticalCenter
          name: "bellSleep"
          color: app.foreground
          size: Style.font.bodySmall
        }
        Text {
          visible: !root.minimalControls
          anchors.verticalCenter: parent.verticalCenter
          text: "Silent"
          textFormat: Text.PlainText
          color: app.foreground
          font.family: app.fontFamily
          font.pixelSize: Style.font.caption
          font.bold: true
        }
      }

      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggleSilent()
      }
    }

    Flickable {
      id: composerFlick
      anchors.fill: parent
      anchors.leftMargin: silentChip.visible ? silentChip.width + Style.space(16) : Style.space(12)
      anchors.rightMargin: Style.space(12)
      anchors.topMargin: Style.space(8)
      anchors.bottomMargin: Style.space(8)
      contentHeight: composer.implicitHeight
      clip: true
      boundsBehavior: Flickable.StopAtBounds

      TextEdit {
        id: composer
        objectName: "composer-text"
        width: composerFlick.width
        wrapMode: TextEdit.Wrap
        textFormat: TextEdit.PlainText
        color: app.foreground
        selectionColor: app.accent
        font.family: app.fontFamily
        font.pixelSize: Style.font.body
        onCursorRectangleChanged: {
          if (cursorRectangle.y < composerFlick.contentY) composerFlick.contentY = cursorRectangle.y
          else if (cursorRectangle.y + cursorRectangle.height > composerFlick.contentY + composerFlick.height)
            composerFlick.contentY = cursorRectangle.y + cursorRectangle.height - composerFlick.height
        }

        onTextChanged: root.composerEdited()
        onCursorPositionChanged: suggestLater.restart()

        Keys.onPressed: function (event) {
          var keys = root.app.shortcuts
          function is(id) { return Keymap.matchesInText(keys, id, event) }
          // Suggestions, while they show, take the keys that move through them.
          if (root.suggestions.length && root.suggestionKey(event)) {
            event.accepted = true
            return
          }
          // A text box takes Enter before any shortcut can, so a question is answered here.
          if (root.prompt && is("prompt.accept")) root.runPrompt(true)
          else if (root.prompt && is("prompt.cancel")) root.runPrompt(false)
          // Copied files or a copied picture wait to be sent; with neither on the clipboard the text is pasted.
          else if (event.matches(StandardKey.Paste)) root.pasteFromClipboard(root.attachments.length ? root.attachAsMedia : true)
          else if (is("composer.pasteFiles")) root.pasteFromClipboard(false)
          else if (is("composer.sendSilent")) root.send({ silent: true })
          else if (is("composer.later")) root.openSendMenu(null)
          else if (is("composer.bold")) root.wrapSelection("**", "**")
          else if (is("composer.italic")) root.wrapSelection("__", "__")
          else if (is("composer.strikethrough")) root.wrapSelection("~~", "~~")
          else if (is("composer.code")) root.wrapSelection("`", "`")
          else if (is("composer.spoiler")) root.wrapSelection("||", "||")
          else if (is("composer.link")) root.wrapLink()
          else if (is("composer.linkPreview")) root.cycleLinkPreview()
          else if (is("composer.send")) root.send()
          else if (is("composer.newLine")) composer.insert(composer.cursorPosition, "\n")
          else if (is("composer.cancel")) {
            if (root.editingId) {
              root.finishEdit()
              if (root.scheduledOpen) root.focusMessages()
            }
            else if (root.replyToId) root.replyToId = 0
            else if (root.attachments.length) root.attachments = []
            else if (root.selecting) root.clearSelection()
            else root.focusMessages()
          }
          else if (is("composer.editLast") && composer.text === "") root.startEdit(Model.lastOwnEditable(root.messages))
          else if (is("composer.toMessages")) root.focusMessages()
          else return
          event.accepted = true
        }

        Text {
          width: parent.width
          elide: Text.ElideRight
          visible: composer.text === ""
          text: root.editing ? (root.editingCaption ? "Caption" : "Edit message")
              : root.attachments.length && root.compactControls ? "Caption"
              : root.attachments.length ? "A caption for the files, if you like   "
                                          + Keymap.label(Keymap.keysFor(app.shortcuts, "composer.send")[0] || "") + " to send"
              : (root.threadOpen ? (app.openTopic.name === "Comments" ? "Comment" : "Reply") : "Message") + (root.compactControls ? "" : "   "
                + Keymap.label(Keymap.keysFor(app.shortcuts, "composer.send")[0] || "")
                + (root.chat && root.chat.silent ? " to send without sound, " : " to send, ")
                 + Keymap.label(Keymap.keysFor(app.shortcuts, "composer.newLine")[0] || "") + " for a new line")
          color: app.muted
          opacity: 0.7
          font: composer.font
        }
      }
    }
  }

  // Attach, stickers, a video message, a voice message.
  Row {
    id: composerButtons
    objectName: "composer-actions"
    visible: !root.recordingVoice && !root.secretBlocked
    anchors.right: parent.right
    anchors.rightMargin: Style.space(8)
    anchors.verticalCenter: parent.verticalCenter
    spacing: 0

    Repeater {
      // md-bell-sleep U+F00A0 (its outline U+F0A93), md-send-clock-outline U+F1164, md-emoticon-outline U+F01F2, md-paperclip U+F03E2,
      // md-plus-circle-outline U+F0419, md-sticker-emoji U+F0785, md-video U+F0567, md-microphone U+F036C
      model: [
        { glyph: String.fromCodePoint(root.chat && root.chat.silent ? 0xF00A0 : 0xF0A93), action: "silent",
          hint: (root.chat && root.chat.silent ? "Silent sending is on: messages here go without sound" : "Silent sending in this chat")
                + "   " + Keymap.label(Keymap.keysFor(app.shortcuts, "window.silent")[0] || "") },
        { glyph: String.fromCodePoint(0xF1164), action: "later",
          hint: "Send later or without sound   " + Keymap.label(Keymap.keysFor(app.shortcuts, "composer.later")[0] || "") },
        { glyph: String.fromCodePoint(0xF01F2), action: "emoji", hint: "Emoji   " + Keymap.label(Keymap.keysFor(app.shortcuts, "window.emoji")[0] || "") },
        { glyph: String.fromCodePoint(0xF03E2), action: "attach", hint: "Attach photos or files   " + Keymap.label(Keymap.keysFor(app.shortcuts, "window.attach")[0] || "") },
        { glyph: String.fromCodePoint(0xF0419), action: "more", hint: "A poll, dice, a contact card or a location   " + Keymap.label(Keymap.keysFor(app.shortcuts, "window.more")[0] || "") },
        { glyph: String.fromCodePoint(0xF0785), action: "stickers", hint: "Stickers   " + Keymap.label(Keymap.keysFor(app.shortcuts, "window.stickers")[0] || "") },
        { glyph: String.fromCodePoint(0xF0567), action: "video", hint: "Video message   " + Keymap.label(Keymap.keysFor(app.shortcuts, "window.videoNote")[0] || "") },
        { glyph: String.fromCodePoint(0xF036C), action: "voice", hint: "Voice message   " + Keymap.label(Keymap.keysFor(app.shortcuts, "window.voice")[0] || "") }
      ].filter(function (entry) {
        return !root.compactControls || entry.action === "voice" || (!root.minimalControls && (entry.action === "emoji" || entry.action === "attach"))
      }).map(function (entry) {
        if (entry.action === "voice" && (composer.text.trim() !== "" || root.attachments.length || root.editingId))
          return { glyph: String.fromCodePoint(0xF048A), action: "send", hint: root.editingId ? "Save edit" : "Send message" }
        return entry
      }).concat(root.compactControls ? [{ glyph: String.fromCodePoint(0xF01D8), action: "overflow", hint: "Message actions" }] : [])
      delegate: Item {
        id: composerButton
        required property var modelData
        objectName: "composer-action-" + modelData.action
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: modelData.hint
        width: Style.space(38)
        height: Style.space(38)
        Keys.onReturnPressed: root.composerAction(modelData.action, composerButton)
        Keys.onSpacePressed: root.composerAction(modelData.action, composerButton)
        Rectangle { anchors.fill: parent; radius: Style.cornerRadius; color: "transparent"; border.width: parent.activeFocus ? 1 : 0; border.color: app.accent }

        Icon {
          anchors.centerIn: parent
          glyph: composerButton.modelData.glyph
          color: buttonArea.containsMouse || (composerButton.modelData.action === "stickers" && root.stickersOpen)
                 || (composerButton.modelData.action === "silent" && !!root.chat && root.chat.silent) ? app.foreground : app.muted
          size: Style.font.title
        }
        MouseArea {
          id: buttonArea
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            root.composerAction(composerButton.modelData.action, composerButton)
          }
          onContainsMouseChanged: if (containsMouse) root.flash(composerButton.modelData.hint)
        }
      }
    }
  }

  // A secret chat not set up yet, or ended: why nothing can be written.
  Row {
    visible: root.secretBlocked
    anchors.centerIn: parent
    spacing: Style.space(8)

    // md-lock-outline U+F0341
    Icon {
      anchors.verticalCenter: parent.verticalCenter
      name: "lock"
      color: app.muted
      size: Style.font.body
    }
    Text {
      readonly property string stateText: Model.secretStateText(root.chat)
      anchors.verticalCenter: parent.verticalCenter
      text: stateText.charAt(0).toUpperCase() + stateText.slice(1)
      textFormat: Text.PlainText
      color: app.muted
      font.family: app.fontFamily
      font.pixelSize: Style.font.body
    }
  }

  // Recording a voice message: replaces the composer until it is sent or cancelled.
  Rectangle {
    objectName: "voice-recording-bar"
    visible: root.recordingVoice
    anchors.fill: parent
    anchors.margins: Style.space(10)
    radius: Style.cornerRadius
    color: Qt.rgba(app.urgent.r, app.urgent.g, app.urgent.b, 0.1)
    border.width: Math.max(1, Style.space(1.5))
    border.color: app.urgent

    Row {
      anchors.left: parent.left
      anchors.leftMargin: Style.space(14)
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(10)

      Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        width: Style.space(10)
        height: width
        radius: width / 2
        color: app.urgent
        SequentialAnimation on opacity {
          running: root.recordingVoice
          loops: Animation.Infinite
          NumberAnimation { to: 0.25; duration: 600 }
          NumberAnimation { to: 1; duration: 600 }
        }
      }
      Text {
        anchors.verticalCenter: parent.verticalCenter
        objectName: "voice-recording-label"
        text: "Recording  " + Model.formatDuration(root.recordingSeconds)
        color: app.foreground
        font.family: app.fontFamily
        font.pixelSize: Style.font.body
      }
      Text {
        visible: !root.compactControls
        anchors.verticalCenter: parent.verticalCenter
        text: Keymap.label(Keymap.keysFor(app.shortcuts, "voice.send")[0] || "") + " sends  ·  "
              + Keymap.label(Keymap.keysFor(app.shortcuts, "voice.cancel")[0] || "") + " cancels"
        color: app.muted
        font.family: app.fontFamily
        font.pixelSize: Style.font.caption
      }
    }

    Row {
      objectName: "voice-recording-actions"
      anchors.right: parent.right
      anchors.rightMargin: Style.space(6)
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(4)

      Repeater {
        // md-close U+F0156, md-send U+F048A
        model: [
          { glyph: String.fromCodePoint(0xF0156), send: false },
          { glyph: String.fromCodePoint(0xF048A), send: true }
        ]
        delegate: Item {
          id: recordButton
          required property var modelData
          width: Style.space(38)
          height: Style.space(38)
          Icon {
            anchors.centerIn: parent
            glyph: recordButton.modelData.glyph
            color: recordButton.modelData.send ? app.accentText : (recordArea.containsMouse ? app.urgent : app.muted)
            size: Style.font.title
          }
          MouseArea {
            id: recordArea
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.stopVoice(recordButton.modelData.send)
          }
        }
      }
    }
  }
}
