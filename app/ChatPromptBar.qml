import QtQuick
import QtQuick.Layouts
import qs.Commons
import "Model.js" as Model
import "Keymap.js" as Keymap

Rectangle {
  required property var root
  required property var app
  Layout.fillWidth: true
  Layout.preferredHeight: visible ? (root.prompt && root.compactControls ? promptText.implicitHeight + promptButtons.height + Style.space(26) : Style.space(40)) : 0
  visible: !!root.prompt || root.notice !== "" || (root.composerBlock === "" && (!!root.replyTo || !!root.editing))
  color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.04)

  Rectangle {
    width: Style.space(3)
    height: parent.height
    color: root.prompt ? app.urgent : (root.notice !== "" && !root.replyTo && !root.editing ? app.muted : app.accent)
  }

  Text {
    id: promptText
    readonly property string cancelKey: Keymap.label(Keymap.keysFor(app.shortcuts, "composer.cancel")[0] || "")
    anchors.left: parent.left
    anchors.leftMargin: Style.space(18)
    anchors.right: promptButtons.visible && !root.compactControls ? promptButtons.left : parent.right
    anchors.rightMargin: Style.space(18)
    anchors.verticalCenter: root.prompt && root.compactControls ? undefined : parent.verticalCenter
    anchors.top: root.prompt && root.compactControls ? parent.top : undefined
    anchors.topMargin: Style.space(10)
    elide: root.prompt && root.compactControls ? Text.ElideNone : Text.ElideRight
    wrapMode: root.prompt && root.compactControls ? Text.Wrap : Text.NoWrap
    textFormat: Text.PlainText
    color: app.foreground
    font.family: app.fontFamily
    font.pixelSize: Style.font.bodySmall
    text: {
      if (root.prompt) return root.prompt.text
      if (root.notice !== "") return root.notice
      if (root.editing) return (root.editingCaption ? "Editing the caption" : "Editing") + "   " + cancelKey + " to cancel"
      if (root.replyTo) return "Replying to " + (root.replyTo.outgoing ? "yourself" : (root.replyTo.senderName || "message")) + ": "
                               + Model.previewOf(root.replyTo) + "   " + cancelKey + " to cancel"
      return ""
    }
  }

  // A question's answers, with their keys.
  Row {
    id: promptButtons
    visible: !!root.prompt
    anchors.right: parent.right
    anchors.rightMargin: Style.space(10)
    height: implicitHeight
    y: root.compactControls ? parent.height - height - Style.space(6) : (parent.height - height) / 2
    spacing: Style.space(4)

    Repeater {
      model: root.prompt ? [
        { accept: true, label: root.prompt.action + (root.compactControls ? "" : "   " + Keymap.label(Keymap.keysFor(app.shortcuts, "prompt.accept")[0] || "")) },
        { accept: false, label: "Cancel" + (root.compactControls ? "" : "   " + Keymap.label(Keymap.keysFor(app.shortcuts, "prompt.cancel")[0] || "")) }
      ] : []
      delegate: Rectangle {
        id: promptAnswer
        required property var modelData
        width: promptLabel.implicitWidth + Style.space(18)
        height: Style.space(28)
        radius: Style.cornerRadius
        color: promptArea.containsMouse ? Qt.rgba(app.accent.r, app.accent.g, app.accent.b, 0.3)
             : (promptAnswer.modelData.accept ? Qt.rgba(app.accent.r, app.accent.g, app.accent.b, 0.14) : "transparent")
        Text {
          id: promptLabel
          anchors.centerIn: parent
          text: promptAnswer.modelData.label
          textFormat: Text.PlainText
          color: app.foreground
          font.family: app.fontFamily
          font.pixelSize: Style.font.caption
          font.bold: promptAnswer.modelData.accept
        }
        MouseArea {
          id: promptArea
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.runPrompt(promptAnswer.modelData.accept)
        }
      }
    }
  }
}
