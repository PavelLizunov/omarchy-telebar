import QtQuick
import Quickshell
import Quickshell.Wayland
import "../app"

// The quick view's photo or video over the whole screen (MediaViewer.qml), in a window of its own: above
// everything, with the keyboard. The quick view loads this file only while something is open.
PanelWindow {
  id: window

  property var host: null   // the quick view

  visible: true
  screen: window.host && window.host.QsWindow.window ? window.host.QsWindow.window.screen : null
  anchors { top: true; bottom: true; left: true; right: true }
  color: "transparent"
  exclusionMode: ExclusionMode.Ignore
  WlrLayershell.namespace: "omagram-quick-media"
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

  Loader {
    anchors.fill: parent
    active: !!window.host && !!window.host.viewingBlock && window.host.viewingBlock.content.kind === "videoNote"
    sourceComponent: RoundVideoViewer {
      app: window.host
      message: window.host.viewingBlock
      onClosed: window.host.closeMedia()
    }
  }
  MediaViewer {
    visible: !window.host || !window.host.viewingBlock || window.host.viewingBlock.content.kind !== "videoNote"
    anchors.fill: parent
    host: window.host
    items: visible && window.host ? window.host.mediaItems : []
    messageId: visible && window.host ? window.host.viewingId : 0
    fileId: window.host ? window.host.viewingFileId : 0
    Component.onCompleted: if (visible) forceActiveFocus()
    onClosed: window.host.closeMedia()
    onPlayed: {
      window.host.closeMedia()
      window.host.dismissRequested()
    }
    onOpenInWindowRequested: function (chatId) {
      window.host.closeMedia()
      window.host.openInWindowRequested(chatId)
    }
  }
}
