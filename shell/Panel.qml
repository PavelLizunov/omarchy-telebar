import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons
import qs.Ui

// The bar's panel: the quick view (QuickView.qml), compact, under Omagram's bar icon. The chats first; choosing
// one puts the chat in their place, and Esc or the arrow goes back to them.
Panel {
  id: root
  moduleName: "io.github.pavellizunov.telebar"
  // Summon from a key binding: omarchy-shell io.github.pavellizunov.telebar.panel toggle
  // (its own target, apart from the overlay, which the shell toggles by plugin id).
  ipcTarget: "io.github.pavellizunov.telebar.panel"
  manageIpc: true

  property var anchorItem: null
  property var hostWidget: null
  property var omagram: null
  readonly property var barIdentity: hostWidget || root
  readonly property int viewHeight: Style.space(560)
  readonly property int actionRevision: 20261015

  function openInWindow(chatId) {
    if (!root.omagram) return false
    if (chatId) root.omagram.openChat(chatId)
    else root.omagram.openWindow()
    // Closing puts away text, but attachments and in-flight work must stay owned.
    if (!view.attachments.length && !view.sending && !view.recordingHere) root.close()
    return true
  }

  function actionStatus() {
    return JSON.stringify({ revision: root.actionRevision,
      iconRevision: root.hostWidget ? root.hostWidget.iconRevision : "",
      serviceReady: !!root.omagram && root.omagram.ready,
      serviceConnected: !!root.omagram && root.omagram.connected,
      roundVideoRevision: view.roundVideoRevision,
      searchRevision: view.searchRevision, searchOpen: view.searchOpen,
      serviceRevision: root.omagram ? root.omagram.implementationRevision : 0,
      launcherRuntime: root.omagram && root.omagram.launcherRuntime ? root.omagram.launcherRuntime : "unknown",
      serviceDraftPresent: !!root.omagram && !!root.omagram.quickDraft,
      opened: root.opened, draftPresent: view.draftPresent,
      attachments: view.attachments.length, sending: view.sending, recording: view.recordingHere })
  }

  IpcHandler {
    target: root.moduleName + ".action"
    function status(): string { return root.actionStatus() }
    function openWindow(): string { return root.openInWindow(0) ? "requested" : "unavailable" }
  }

  onOpenedChanged: {
    if (opened) view.reset(0)
    else view.leave()
  }

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    centerOnBar: true
    focusTarget: view.focusItem
    popoutSwitching: root.popoutSwitching
    popoutSwitchClosing: root.popoutSwitchClosing
    contentWidth: panel.fittedContentWidth(Style.space(460))
    contentHeight: panel.fittedContentHeight(root.viewHeight)

    TelebarQuickView {
      id: view
      width: parent.width
      height: Math.max(Style.space(240), panel.contentHeight - panel.verticalContentInset)
      compact: true
      service: root.omagram
      opened: root.opened
      background: Commons.Color.popups.background
      foreground: root.barForeground
      selected: Style.hoverFillFor(root.barForeground, Commons.Color.accent)
      onDismissRequested: root.close()
      onTabRequested: function (direction) { root.switchPanel(direction) }
      onFocusReturned: panel.beginFocusPrime()   // the full-screen view had the keyboard: take it back
      onOpenInWindowRequested: function (chatId) { root.openInWindow(chatId) }
    }
  }
}
