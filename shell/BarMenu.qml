import QtQuick
import qs.Commons

// Actual popup content; window placement and focus remain owned by PopupCard.
Column {
  id: menu
  property var entries: []
  property string fontFamily: Style.font.family
  signal picked(string action)
  spacing: Style.space(2)
  Repeater {
    model: menu.entries
    delegate: Rectangle {
      id: entry
      required property var modelData
      objectName: "bar-menu-" + modelData.action
      width: menu.width
      height: Style.space(30)
      radius: Style.cornerRadius
      color: entryHover.hovered || activeFocus ? Style.hoverFillFor(Color.popups.text, Color.accent) : "transparent"
      activeFocusOnTab: true
      Accessible.role: Accessible.Button
      Accessible.name: modelData.label
      Keys.onReturnPressed: menu.picked(modelData.action)
      Keys.onSpacePressed: menu.picked(modelData.action)
      HoverHandler { id: entryHover }
      Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Style.space(10)
        anchors.rightMargin: Style.space(10)
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: entry.modelData.label
        elide: Text.ElideRight
        color: entry.modelData.action === "quit" ? Color.urgent : Color.popups.text
        font.family: menu.fontFamily
        font.pixelSize: Style.font.body
      }
      MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: menu.picked(entry.modelData.action) }
    }
  }
}
