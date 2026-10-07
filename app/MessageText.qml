import QtQuick
import qs.Commons
import "Model.js" as Model

// Native fragment selection and clipboard copying, without an editable message.
TextEdit {
  id: body
  readOnly: true
  selectByMouse: true
  persistentSelection: true
  textFormat: TextEdit.RichText
  wrapMode: TextEdit.Wrap
  property color surfaceColor: Color.menu.background
  selectionColor: { var c = Model.mixColors(surfaceColor, color, 0.22); return Qt.rgba(c.r, c.g, c.b, 1) }
  selectedTextColor: { var c = Model.readableColor(color, selectionColor, color, 4.5); return Qt.rgba(c.r, c.g, c.b, 1) }
  property bool contextEnabled: false
  signal contextRequested(real x, real y)
  signal messageSelectionRequested()

  Keys.onPressed: function (event) {
    if (event.matches(StandardKey.Copy) && body.selectedText !== "") {
      body.copy()
      event.accepted = true
    }
  }
  HoverHandler { cursorShape: body.hoveredLink ? Qt.PointingHandCursor : Qt.IBeamCursor }
  MouseArea {
    anchors.fill: parent
    acceptedButtons: Qt.LeftButton | (body.contextEnabled ? Qt.RightButton : Qt.NoButton)
    onPressed: function (mouse) {
      if (mouse.button === Qt.LeftButton && (!(mouse.modifiers & Qt.ControlModifier) || !body.contextEnabled)) mouse.accepted = false
    }
    onClicked: function (mouse) {
      if (mouse.button === Qt.RightButton) body.contextRequested(mouse.x, mouse.y)
      else body.messageSelectionRequested()
    }
  }
}
