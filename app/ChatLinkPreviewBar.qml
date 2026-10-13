import QtQuick
import QtQuick.Layouts
import qs.Commons
import "Model.js" as Model
import "Keymap.js" as Keymap

Rectangle {
  required property var root
  required property var app
  Layout.fillWidth: true
  Layout.preferredHeight: visible ? linkPreviewRow.implicitHeight + Style.space(14) : 0
  visible: !!root.chat && root.composerBlock === "" && !root.editingId
           && (!!root.linkPreview || (root.linkPreviewMode === "none" && root.linkPreviewLink !== ""))
  color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.03)

  Rectangle { width: Style.space(3); height: parent.height; color: root.linkPreviewMode === "none" ? app.muted : app.accent }

  RowLayout {
    id: linkPreviewRow
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    anchors.leftMargin: Style.space(18)
    anchors.rightMargin: Style.space(14)
    spacing: Style.space(12)

    Column {
      Layout.fillWidth: true
      spacing: Style.space(1)

      Text {
        width: parent.width
        elide: Text.ElideRight
        textFormat: Text.PlainText
        text: root.linkPreviewMode === "none" ? "No link preview"
            : (root.linkPreview ? (root.linkPreview.siteName || root.linkPreview.displayUrl || "Link preview") : "")
        color: root.linkPreviewMode === "none" ? app.muted : app.accentText
        font.family: app.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }
      Text {
        visible: root.linkPreviewMode !== "none" && text !== ""
        width: parent.width
        elide: Text.ElideRight
        textFormat: Text.PlainText
        text: root.linkPreview ? (root.linkPreview.title || root.linkPreview.description || "") : ""
        color: app.foreground
        font.family: app.fontFamily
        font.pixelSize: Style.font.bodySmall
      }
    }

    Text {
      readonly property string key: Keymap.label(Keymap.keysFor(app.shortcuts, "composer.linkPreview")[0] || "")
      text: ({ below: "Under the text", above: "Above the text", none: "Left out" })[root.linkPreviewMode] + (root.compactControls ? "" : "   " + key + " changes it")
      textFormat: Text.PlainText
      color: app.muted
      font.family: app.fontFamily
      font.pixelSize: Style.font.caption

      MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.cycleLinkPreview() }
    }
  }
}
