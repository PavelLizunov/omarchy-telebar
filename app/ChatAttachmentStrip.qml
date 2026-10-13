import QtQuick
import QtQuick.Layouts
import qs.Commons
import "Model.js" as Model
import "Keymap.js" as Keymap

Rectangle {
  required property var root
  required property var app
  Layout.fillWidth: true
  Layout.preferredHeight: visible ? Style.space(82) : 0
  visible: root.attachments.length > 0 && !root.showTopics
  color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.03)

  Rectangle { width: parent.width; height: 1; color: app.border; opacity: 0.35 }

  ListView {
    id: attachmentList
    anchors.left: parent.left
    anchors.right: attachmentMode.left
    anchors.leftMargin: Style.space(10)
    anchors.rightMargin: Style.space(10)
    anchors.verticalCenter: parent.verticalCenter
    height: Style.space(64)
    orientation: ListView.Horizontal
    spacing: Style.space(8)
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    model: root.attachments

    delegate: Rectangle {
      id: attachment
      required property var modelData
      required property int index
      width: Style.space(64)
      height: Style.space(64)
      radius: Style.cornerRadius
      color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.06)
      clip: true

      Image {
        id: thumbnail
        anchors.fill: parent
        visible: status === Image.Ready
        source: attachment.modelData.kind === "photo" ? "file://" + attachment.modelData.path : ""
        sourceSize.width: 128
        sourceSize.height: 128
        asynchronous: true
        fillMode: Image.PreserveAspectCrop
      }
      // What has no picture shows what it is: md-video U+F0567, md-music-note U+F0387, md-file-outline U+F0224
      Icon {
        anchors.horizontalCenter: parent.horizontalCenter
        y: Style.space(10)
        visible: !thumbnail.visible
        name: attachment.modelData.kind === "video" ? "video" : (attachment.modelData.kind === "audio" ? "music" : "file")
        color: app.foreground
        size: Style.font.title
      }
      Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: Style.space(4)
        visible: !thumbnail.visible
        horizontalAlignment: Text.AlignHCenter
        elide: Text.ElideMiddle
        text: attachment.modelData.name
        textFormat: Text.PlainText
        color: app.muted
        font.family: app.fontFamily
        font.pixelSize: Style.font.caption
      }
      // md-close U+F0156: leave this one out
      Rectangle {
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: Style.space(3)
        width: Style.space(18)
        height: width
        radius: width / 2
        color: Qt.rgba(0, 0, 0, removeArea.containsMouse ? 0.8 : 0.55)
        Icon {
          anchors.centerIn: parent
          name: "close"
          color: "white"
          size: Style.font.caption
        }
        MouseArea {
          id: removeArea
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.removeAttachment(attachment.index)
        }
      }
    }
  }

  Column {
    id: attachmentMode
    anchors.right: parent.right
    anchors.rightMargin: Style.space(14)
    anchors.verticalCenter: parent.verticalCenter
    width: Math.min(Style.space(250), parent.width * 0.45)
    spacing: Style.space(4)

    // Photos and videos as themselves, or everything as files.
    Rectangle {
      width: parent.width
      height: Style.space(28)
      radius: Style.cornerRadius
      color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, modeArea.containsMouse ? 0.16 : 0.08)
      Text {
        anchors.centerIn: parent
        width: parent.width - Style.space(12)
        horizontalAlignment: Text.AlignHCenter
        elide: Text.ElideRight
        text: root.attachAsMedia ? "As photos and videos" : "As files"
        textFormat: Text.PlainText
        color: app.foreground
        font.family: app.fontFamily
        font.pixelSize: Style.font.caption
      }
      MouseArea {
        id: modeArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.attachAsMedia = !root.attachAsMedia
        onContainsMouseChanged: if (containsMouse) root.flash(root.attachAsMedia ? "Click to send them as files, as they are"
                                                                               : "Click to send photos and videos as themselves")
      }
    }
    Text {
      width: parent.width
      horizontalAlignment: Text.AlignHCenter
      elide: Text.ElideRight
      text: (root.attachments.length === 1 ? "1 file" : root.attachments.length + " files") + "   "
            + Keymap.label(Keymap.keysFor(app.shortcuts, "composer.cancel")[0] || "") + " takes them away"
      textFormat: Text.PlainText
      color: app.muted
      font.family: app.fontFamily
      font.pixelSize: Style.font.caption
    }
  }
}
