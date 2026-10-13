import QtQuick
import QtQuick.Layouts
import qs.Commons
import "Model.js" as Model
import "Keymap.js" as Keymap

Rectangle {
  required property var root
  required property var app
  required property var composer
  Layout.fillWidth: true
  Layout.preferredHeight: visible ? suggestionList.height + Style.space(9) : 0
  visible: root.suggestions.length > 0 && !root.showTopics
  color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.03)

  Rectangle { width: parent.width; height: 1; color: app.border; opacity: 0.35 }

  ListView {
    id: suggestionList
    x: Style.space(10)
    y: Style.space(5)
    width: parent.width - Style.space(20)
    height: Math.min(6, count) * Style.space(34)
    clip: true
    interactive: count > 6
    boundsBehavior: Flickable.StopAtBounds
    model: root.suggestions
    currentIndex: root.suggestionCursor
    onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)

    delegate: Rectangle {
      id: suggestion
      required property var modelData
      required property int index
      width: suggestionList.width
      height: Style.space(34)
      radius: Style.cornerRadius
      color: suggestion.index === root.suggestionCursor ? app.selected
           : (suggestionArea.containsMouse ? Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.05) : "transparent")

      Text {
        id: suggestionLabel
        anchors.left: parent.left
        anchors.leftMargin: Style.space(10)
        anchors.verticalCenter: parent.verticalCenter
        text: suggestion.modelData.label
        textFormat: Text.PlainText
        color: app.foreground
        font.family: app.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
      }
      Text {
        anchors.left: suggestionLabel.right
        anchors.leftMargin: Style.space(12)
        anchors.right: parent.right
        anchors.rightMargin: Style.space(10)
        anchors.verticalCenter: parent.verticalCenter
        text: suggestion.modelData.detail
        textFormat: Text.PlainText
        elide: Text.ElideRight
        color: app.muted
        font.family: app.fontFamily
        font.pixelSize: Style.font.caption
      }
      MouseArea {
        id: suggestionArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: {
          root.pickSuggestion(suggestion.index, false)
          composer.forceActiveFocus()
        }
      }
    }
  }
}
