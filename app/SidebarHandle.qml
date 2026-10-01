import QtQuick
import qs.Commons

// Shared by the window and inert previews: click toggles, drag resizes.
Item {
  id: handle
  property var app
  property bool compact: false
  property real currentWidth: 300
  property real maximumWidth: 1200
  signal widthEdited(real value)
  signal widthCommitted()
  signal toggleRequested()
  readonly property bool dragging: (rail.pressed && rail.dragged) || (grip.pressed && grip.dragged)
  readonly property bool hovered: rail.containsMouse || grip.containsMouse
  implicitWidth: 1
  function forceButtonFocus() { button.forceActiveFocus() }

  Rectangle {
    width: 1
    height: button.y
    color: handle.hovered || handle.dragging ? app.accent : app.border
    opacity: handle.hovered || handle.dragging ? 0.7 : 0.35
  }
  Rectangle {
    y: button.y + button.height
    width: 1
    height: Math.max(0, handle.height - y)
    color: handle.hovered || handle.dragging ? app.accent : app.border
    opacity: handle.hovered || handle.dragging ? 0.7 : 0.35
  }

  component ResizeArea: MouseArea {
    property bool dragged: false
    property real pressX: 0
    property real startWidth: 0
    hoverEnabled: true
    preventStealing: true
    cursorShape: Qt.SplitHCursor
    onPressed: function (mouse) {
      dragged = false
      pressX = mapToItem(null, mouse.x, mouse.y).x
      startWidth = handle.currentWidth
      hintDelay.stop()
    }
    onPositionChanged: function (mouse) {
      if (!pressed) return
      var delta = mapToItem(null, mouse.x, mouse.y).x - pressX
      if (Math.abs(delta) >= drag.threshold) dragged = true
      if (dragged) handle.widthEdited(Math.round(Math.max(72, Math.min(1200, handle.maximumWidth, startWidth + delta))))
    }
    onReleased: if (dragged) handle.widthCommitted()
    onCanceled: {
      if (dragged) handle.widthEdited(startWidth)
      dragged = false
    }
  }

  ResizeArea {
    id: rail
    objectName: "sidebar-rail-hit"
    width: Style.space(12)
    height: parent.height
    anchors.horizontalCenter: parent.horizontalCenter
    onDoubleClicked: if (!dragged) handle.toggleRequested()
  }

  Rectangle {
    id: button
    objectName: "sidebar-button"
    anchors.horizontalCenter: parent.horizontalCenter
    y: (Style.space(56) - height) / 2
    width: Style.space(28)
    height: width
    radius: Style.cornerRadius
    // A solid base is required: selectedBackground is normally translucent.
    color: Qt.rgba(app.background.r, app.background.g, app.background.b, 1)
    border.width: 1
    border.color: activeFocus ? app.accent : app.border
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: handle.compact ? "Expand chat list" : "Collapse chat list"
    Accessible.description: "Drag to resize. Left and Right adjust the width."
    Accessible.onPressAction: handle.toggleRequested()
    Keys.onReturnPressed: handle.toggleRequested()
    Keys.onEnterPressed: handle.toggleRequested()
    Keys.onSpacePressed: handle.toggleRequested()
    Keys.onLeftPressed: { handle.widthEdited(Math.max(72, handle.currentWidth - Style.space(16))); handle.widthCommitted() }
    Keys.onRightPressed: { handle.widthEdited(Math.min(handle.maximumWidth, Math.max(180, handle.currentWidth + Style.space(16)))); handle.widthCommitted() }
    Rectangle {
      anchors.fill: parent
      anchors.margins: 1
      radius: Math.max(0, parent.radius - 1)
      color: app.selected
      visible: grip.containsMouse || button.activeFocus || grip.pressed
    }
    Icon {
      objectName: "sidebar-icon"
      anchors.centerIn: parent
      size: Style.space(16)
      name: handle.compact ? "panelOpen" : "panelClose"
      color: app.foreground
    }
    ResizeArea {
      id: grip
      objectName: "sidebar-grip-hit"
      anchors.centerIn: parent
      width: Style.space(32)
      height: width
      onClicked: if (!dragged) handle.toggleRequested()
      onDoubleClicked: if (!dragged) handle.toggleRequested()
      onContainsMouseChanged: {
        if (containsMouse) hintDelay.restart()
        else hintDelay.stop()
      }
    }
    Timer { id: hintDelay; interval: 650 }
    Rectangle {
      visible: grip.containsMouse && !grip.pressed && !hintDelay.running && !handle.dragging
      anchors.top: parent.bottom
      anchors.topMargin: Style.space(6)
      x: handle.compact ? 0 : (parent.width - width) / 2
      width: hint.implicitWidth + Style.space(16)
      height: hint.implicitHeight + Style.space(10)
      radius: Style.cornerRadius
      color: app.background
      border.width: 1
      border.color: app.border
      Text {
        id: hint
        anchors.centerIn: parent
        text: "Click to toggle · drag to resize"
        color: app.foreground
        font.family: app.fontFamily
        font.pixelSize: Style.font.caption
      }
    }
  }
}
