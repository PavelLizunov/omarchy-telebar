import QtQuick
import QtQuick.Layouts
import qs.Commons
import "Model.js" as Model
import "Keymap.js" as Keymap
import "SettingsRows.js" as SettingsRows
import "SettingsProfile.js" as SettingsProfile
import "SettingsSecurity.js" as SettingsSecurity
import "SettingsFolders.js" as SettingsFolders
import "SettingsProxies.js" as SettingsProxies
import "SettingsActions.js" as SettingsActions

// Settings: your profile (name, username, bio, photo), your account (what Omagram keeps on this
// computer, the devices signed in, signing out), every keyboard shortcut in Omagram, and the
// shortcuts that work anywhere.
//
// ↑/↓ or j/k choose. Enter changes a field of your profile (Enter saves it, Esc leaves it as it
// was; Backspace on the photo removes it). Enter opens an account row -- anything that changes the account asks first,
// answered with Enter or Esc -- or records new keys for an action (the next combination you press
// replaces its keys); A adds a key, Backspace removes its last key, R resets it, Esc closes. While
// recording, Esc cancels. These keys are fixed on purpose: whatever you do to the other shortcuts,
// this screen always works, and Ctrl+, cannot be lost for good (the gear in the chat list opens it
// too).
FocusScope {
  id: settings

  property var app
  property int cursor: 1
  property string recording: ""      // the action being recorded, "" when none
  property bool recordingAdds: false
  property string error: ""
  property var storage: null         // what storage.stats last said
  property var sessions: []
  property bool sessionsOpen: false
  property var confirm: null         // { text, run }: a question waiting for Enter or Esc
  property real nowMs: Date.now()
  property var profile: null         // what profile.get last said
  property var editing: null         // { field, label }: a field of your profile being changed
  property bool photoBusy: false
  readonly property bool narrow: width < Style.space(600)
  readonly property var profileChat: Model.profileChat(settings.profile)

  readonly property var overrides: settings.app ? settings.app.shortcuts : ({})
  readonly property var globals: settings.app ? settings.app.globalShortcuts : ({})
  readonly property var globalStatus: settings.app ? settings.app.globalStatus : ({})
  readonly property var rows: settings.buildRows()
  readonly property var current: settings.rows[settings.cursor] || null
  readonly property var accountKinds: ["profilePhoto", "profileField", "profilePhone", "privacy", "blocked", "blockedSender", "password",
                                       "passwordOff", "passwordCode", "accountTtl", "autoDelete", "proxy", "proxyAdd", "reactionsSeen", "sound", "soundHear", "scope", "previews", "download", "folder", "newFolder",
                                       "folderName", "folderFlag", "folderChat", "folderAdd", "folderSave", "folderDelete",
                                       "storage", "sessions", "session", "otherSessions", "logout", "bridge", "bridgeTest", "showStories"]

  signal closed()

  function loadVisibleSettings() {
    return SettingsRows.loadVisibleSettings({ settings: settings })
  }
  onVisibleChanged: settings.loadVisibleSettings()
  Component.onCompleted: settings.loadVisibleSettings()

  function loadProfile() {
    return SettingsRows.loadProfile({ app: app, settings: settings })
  }

  function loadStorage() {
    return SettingsRows.loadStorage({ app: app, settings: settings })
  }

  function loadSessions() {
    return SettingsRows.loadSessions({ app: app, settings: settings })
  }

  function buildRows() {
    return SettingsRows.buildRows({ app: app, settings: settings })
  }

  function move(delta) {
    return SettingsRows.move({ ListView: ListView, list: list, settings: settings }, delta)
  }

  function editable(row) {
    return SettingsRows.editable({  }, row)
  }

  function keysOf(row) {
    return SettingsRows.keysOf({ settings: settings }, row)
  }

  function changed(row) {
    return SettingsRows.changed({ settings: settings }, row)
  }

  function startRecording(adds) {
    return SettingsRows.startRecording({ settings: settings }, adds)
  }

  function send(overrides, globals) {
    return SettingsRows.send({ app: app, settings: settings }, overrides, globals)
  }

  function withGlobal(id, combo) {
    return SettingsRows.withGlobal({ settings: settings }, id, combo)
  }

  function capture(event) {
    return SettingsRows.capture({ settings: settings }, event)
  }

  function removeLast() {
    return SettingsRows.removeLast({ settings: settings })
  }

  function reset() {
    return SettingsRows.reset({ settings: settings })
  }

  function statusText(id) {
    return SettingsRows.statusText({ settings: settings }, id)
  }

  // ---------------------------------------------------------------- your profile

  function startEditing(row) {
    return SettingsProfile.startEditing({ editor: editor, settings: settings }, row)
  }

  function cancelEditing() {
    return SettingsProfile.cancelEditing({ editor: editor, list: list, settings: settings })
  }

  function saveEditing() {
    return SettingsProfile.saveEditing({ app: app, editor: editor, settings: settings })
  }

  function urlToPath(url) {
    return SettingsProfile.urlToPath({  }, url)
  }

  // Telegram takes a square JPEG: the service cuts one from the middle of the picture.
  function setPhoto(path) {
    return SettingsProfile.setPhoto({ app: app, settings: settings }, path)
  }

  function askRemovePhoto() {
    return SettingsProfile.askRemovePhoto({ app: app, settings: settings })
  }

  // ---------------------------------------------------------------- privacy and security

  property var privacy: ({})         // setting id -> { base, allowed, restricted }, or null when Telegram did not say
  property var blocked: null         // { total, senders }
  property bool blockedOpen: false
  property var password: null        // what password.get last said
  property int accountTtl: 0
  property int defaultAutoDelete: -1 // seconds; -1 until Telegram has said
  property var flow: null            // two-step verification being changed, or a proxy added: { kind, steps, step, values }
  property string editorError: ""
  property int flowSerial: 0
  property int folderSerial: 0

  function loadPrivacy() {
    return SettingsSecurity.loadPrivacy({ app: app, settings: settings })
  }

  function loadBlocked() {
    return SettingsSecurity.loadBlocked({ app: app, settings: settings })
  }

  function changePrivacy(row) {
    return SettingsSecurity.changePrivacy({ app: app, settings: settings }, row)
  }

  function askUnblock(sender) {
    return SettingsSecurity.askUnblock({ app: app, settings: settings }, sender)
  }

  function changeAccountTtl() {
    return SettingsSecurity.changeAccountTtl({ app: app, settings: settings })
  }

  function changeDefaultAutoDelete() {
    return SettingsSecurity.changeDefaultAutoDelete({ app: app, settings: settings })
  }

  // Two-step verification is changed one field at a time in the bar above the list; what is typed
  // is kept only until it is sent, or the change is left with Esc.
  function startPasswordFlow(kind) {
    return SettingsSecurity.startPasswordFlow({ settings: settings }, kind)
  }

  function showFlowStep() {
    return SettingsSecurity.showFlowStep({ editor: editor, settings: settings })
  }

  function flowNext() {
    return SettingsSecurity.flowNext({ app: app, editor: editor, settings: settings })
  }

  function passwordAnswered(answer, serial) {
    return SettingsSecurity.passwordAnswered({ settings: settings }, answer, serial)
  }

  // ---------------------------------------------------------------- notifications

  property var scopes: ({})          // "private", "groups", "channels" -> { muted, preview }, or null

  function loadNotifications() {
    return SettingsFolders.loadNotifications({ app: app, settings: settings })
  }

  function changeScope(scope, change) {
    return SettingsFolders.changeScope({ app: app, settings: settings }, scope, change)
  }

  // One row for message text in the notifications of every type of chat: shown everywhere, unless it already is.
  function togglePreviews() {
    return SettingsFolders.togglePreviews({ settings: settings })
  }

  // ---------------------------------------------------------------- chat folders

  property var folders: []           // what folders.get last said, in Telegram's order
  property var folderOpen: null      // the folder being edited: a copy, with id 0 for a new one
  property string pickerFor: ""      // "included" or "excluded": where the chat picked goes
  readonly property bool pickerOpen: chatPicker.visible

  function loadFolders() {
    return SettingsFolders.loadFolders({ app: app, settings: settings })
  }

  // A folder's own page: its name, the kinds of chats it takes, what it leaves out, the chats always and never
  // in it, then saving or deleting it.
  function folderRows() {
    return SettingsFolders.folderRows({ list: list, settings: settings })
  }

  function openFolder(folder) {
    return SettingsFolders.openFolder({ list: list, settings: settings }, folder)
  }

  function closeFolder() {
    return SettingsFolders.closeFolder({ settings: settings })
  }

  // The folder being edited, changed: a new object, so the rows see it.
  function changeFolder(change) {
    return SettingsFolders.changeFolder({ settings: settings }, change)
  }

  function startEditingFolderName() {
    return SettingsFolders.startEditingFolderName({ editor: editor, settings: settings })
  }

  function startAddingChat(listName) {
    return SettingsFolders.startAddingChat({ chatPicker: chatPicker, settings: settings }, listName)
  }

  // A chat goes on one list and off the other: a chat cannot be both always and never in a folder.
  function addFolderChat(chatId) {
    return SettingsFolders.addFolderChat({ settings: settings }, chatId)
  }

  function removeFolderChat(listName, chatId) {
    return SettingsFolders.removeFolderChat({ settings: settings }, listName, chatId)
  }

  function saveFolder() {
    return SettingsFolders.saveFolder({ app: app, settings: settings })
  }

  function askDeleteFolder() {
    return SettingsFolders.askDeleteFolder({ app: app, settings: settings })
  }

  function moveFolder(folderId, delta) {
    return SettingsFolders.moveFolder({ app: app, settings: settings }, folderId, delta)
  }

  FilePicker {
    id: photoDialog
    objectName: "profile-photo-file-dialog"
    app: settings.app
    title: "Your new profile photo"
    nameFilters: ["*.jpg", "*.jpeg", "*.png", "*.webp", "*.gif", "*.bmp"]
    onAccepted: settings.setPhoto(settings.urlToPath(selectedFile))
    onRejected: settings.forceActiveFocus()
  }

  // Choosing a chat for a folder: the forward picker, with a title of its own.
  ForwardPicker {
    id: chatPicker
    anchors.fill: parent
    app: settings.app
    chats: settings.app && settings.app.chats ? settings.app.chats : []
    title: settings.pickerFor === "excluded" ? "A chat never in the folder" : "A chat always in the folder"
    onPicked: function (chatId, title) { settings.addFolderChat(chatId) }
    onDismissed: settings.forceActiveFocus()
  }

  // ---------------------------------------------------------------- proxies

  property var proxies: []           // what proxies.list said: never a password or secret
  property var bridge: ({ running: false, enabled: false })
  property bool bridgeBusy: false
  property string bridgeTestResult: ""
  property var proxyPings: ({})      // proxy id -> { seconds } or { error: true }

  function loadProxies() {
    return SettingsProxies.loadProxies({ app: app, settings: settings })
  }

  function toggleBridge() {
    return SettingsProxies.toggleBridge({ app: app, settings: settings })
  }

  function pingProxy(id) {
    return SettingsProxies.pingProxy({ app: app, settings: settings }, id)
  }

  // Enter on a proxy: use it, or stop using it.
  function toggleProxy(proxy) {
    return SettingsProxies.toggleProxy({ app: app, settings: settings }, proxy)
  }

  function askRemoveProxy(proxy) {
    return SettingsProxies.askRemoveProxy({ app: app, settings: settings }, proxy)
  }

  // A proxy is added one field at a time in the bar above the list, as two-step verification is changed.
  function startProxyFlow(kind) {
    return SettingsProxies.startProxyFlow({ settings: settings }, kind)
  }

  function addProxy(kind, values) {
    return SettingsProxies.addProxy({ app: app, settings: settings }, kind, values)
  }

  // ---------------------------------------------------------------- the account

  function ask(text, run) {
    return SettingsActions.ask({ list: list, settings: settings }, text, run)
  }

  function answer(yes) {
    return SettingsActions.answer({ settings: settings }, yes)
  }

  function activate(row) {
    return SettingsActions.activate({ app: app, list: list, photoDialog: photoDialog, settings: settings }, row)
  }

  Keys.onPressed: function (event) {
    if (photoDialog.visible) return
    if (settings.recording !== "") {
      settings.capture(event)
      event.accepted = true
      return
    }
    var key = event.key
    if (settings.editing) {   // the field being changed has the keys; nothing else moves meanwhile
      if (key === Qt.Key_Escape) settings.cancelEditing()
      event.accepted = true
      return
    }
    if (settings.confirm) {
      if (key === Qt.Key_Return || key === Qt.Key_Enter) settings.answer(true)
      else if (key === Qt.Key_Escape) settings.answer(false)
      event.accepted = true   // nothing else happens while a question waits
      return
    }
    if (key === Qt.Key_Escape) { if (settings.folderOpen) settings.closeFolder(); else settings.closed() }
    else if (key === Qt.Key_Down || key === Qt.Key_J) settings.move(1)
    else if (key === Qt.Key_Up || key === Qt.Key_K) settings.move(-1)
    else if (key === Qt.Key_PageDown) settings.move(8)
    else if (key === Qt.Key_PageUp) settings.move(-8)
    else if (key === Qt.Key_Return || key === Qt.Key_Enter) settings.activate(settings.current)
    else if (key === Qt.Key_A) settings.startRecording(true)
    else if (key === Qt.Key_Backspace || key === Qt.Key_Delete) settings.removeLast()
    else if (key === Qt.Key_R) settings.reset()
    else if ((key === Qt.Key_BracketLeft || key === Qt.Key_BracketRight) && settings.current && settings.current.kind === "folder")
      settings.moveFolder(settings.current.folder.id, key === Qt.Key_BracketLeft ? -1 : 1)
    else return
    event.accepted = true
  }

  Rectangle {
    anchors.fill: parent
    color: settings.app.background
  }

  MouseArea { anchors.fill: parent }

  ColumnLayout {
    anchors.fill: parent
    anchors.margins: Style.space(24)
    spacing: Style.space(10)

    RowLayout {
      Layout.fillWidth: true

      Column {
        Layout.fillWidth: true
        spacing: Style.space(4)
        Text {
          text: "Settings"
          color: settings.app.foreground
          font.family: settings.app.fontFamily
          font.pixelSize: Style.font.displayLarge
          font.bold: true
        }
        Text {
          width: parent.width
          wrapMode: Text.Wrap
          text: "↑↓ choose  ·  Enter open or change  ·  A add a key  ·  Backspace remove  ·  R reset  ·  Esc close"
          color: settings.app.muted
          font.family: settings.app.fontFamily
          font.pixelSize: Style.font.caption
        }
      }

      // md-close (U+F0156)
      Icon {
        name: "close"
        color: closeArea.containsMouse ? settings.app.foreground : settings.app.muted
        size: Style.font.title
        MouseArea {
          id: closeArea
          anchors.fill: parent
          anchors.margins: -Style.space(6)
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: settings.closed()
        }
      }
    }

    // A question about the account, before anything is changed.
    Rectangle {
      Layout.fillWidth: true
      visible: !!settings.confirm
      Layout.preferredHeight: visible ? Math.max(Style.space(46), confirmText.implicitHeight + (settings.narrow ? confirmButtons.height + Style.space(30) : Style.space(20))) : 0
      radius: Style.cornerRadius
      color: Qt.rgba(settings.app.urgent.r, settings.app.urgent.g, settings.app.urgent.b, 0.1)
      border.width: 1
      border.color: settings.app.urgent

      Text {
        id: confirmText
        objectName: "settings-confirmation-text"
        anchors.left: parent.left
        anchors.right: settings.narrow ? parent.right : confirmButtons.left
        anchors.leftMargin: Style.space(14)
        anchors.rightMargin: Style.space(10)
        anchors.verticalCenter: settings.narrow ? undefined : parent.verticalCenter
        anchors.top: settings.narrow ? parent.top : undefined
        anchors.topMargin: Style.space(10)
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
        text: settings.confirm ? settings.confirm.text : ""
        color: settings.app.foreground
        font.family: settings.app.fontFamily
        font.pixelSize: Style.font.bodySmall
      }

      Row {
        id: confirmButtons
        anchors.right: parent.right
        anchors.rightMargin: Style.space(10)
        anchors.verticalCenter: settings.narrow ? undefined : parent.verticalCenter
        anchors.bottom: settings.narrow ? parent.bottom : undefined
        anchors.bottomMargin: Style.space(6)
        spacing: Style.space(4)

        Repeater {
          model: [{ yes: true, label: "Yes   Enter" }, { yes: false, label: "No   Esc" }]
          delegate: Rectangle {
            id: choice
            required property var modelData
            width: choiceLabel.implicitWidth + Style.space(18)
            height: Style.space(28)
            radius: Style.cornerRadius
            color: choiceArea.containsMouse ? Qt.rgba(settings.app.foreground.r, settings.app.foreground.g, settings.app.foreground.b, 0.14)
                 : (choice.modelData.yes ? Qt.rgba(settings.app.urgent.r, settings.app.urgent.g, settings.app.urgent.b, 0.2) : "transparent")
            Text {
              id: choiceLabel
              anchors.centerIn: parent
              text: choice.modelData.label
              color: settings.app.foreground
              font.family: settings.app.fontFamily
              font.pixelSize: Style.font.caption
              font.bold: choice.modelData.yes
            }
            MouseArea {
              id: choiceArea
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: settings.answer(choice.modelData.yes)
            }
          }
        }
      }
    }

    // A field of your profile being changed: Enter saves it, Esc leaves it as it was.
    Rectangle {
      Layout.fillWidth: true
      visible: !!settings.editing
      Layout.preferredHeight: visible ? editor.implicitHeight + Style.space(24) : 0
      radius: Style.cornerRadius
      color: Qt.rgba(settings.app.accent.r, settings.app.accent.g, settings.app.accent.b, 0.06)
      border.width: 1
      border.color: settings.app.accent

      Field {
        id: editor
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: Style.space(12)
        app: settings.app
        label: settings.editing ? settings.editing.label + "   ·   Enter saves   ·   Esc cancels" : ""
        placeholder: settings.editing && settings.editing.placeholder ? settings.editing.placeholder
                   : (settings.editing && settings.editing.field === "username" ? "a name people can find you by"
                      : (settings.editing && settings.editing.field === "bio" ? "a few words about you" : ""))
        secret: !!settings.editing && settings.editing.secret === true
        maximumLength: settings.flow ? (settings.flow.kind === "proxy" ? 2048 : 256) : (settings.editing && settings.editing.field === "bio" ? 140 : 64)
        error: !settings.editing ? "" : (settings.flow ? settings.editorError : Model.profileProblem(settings.editing.field, editor.text))
        onAccepted: settings.saveEditing()
        Keys.onEscapePressed: settings.cancelEditing()
      }
    }

    Text {
      Layout.fillWidth: true
      visible: settings.error !== ""
      text: settings.error
      textFormat: Text.PlainText
      wrapMode: Text.Wrap
      color: settings.app.urgent
      font.family: settings.app.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    ListView {
      id: list
      Layout.fillWidth: true
      Layout.fillHeight: true
      clip: true
      model: settings.rows
      boundsBehavior: Flickable.StopAtBounds

      WheelScroll { view: list }

      delegate: Rectangle {
        id: row
        required property var modelData
        required property int index
        readonly property bool header: modelData.kind === "header"
        readonly property bool account: settings.accountKinds.indexOf(modelData.kind) >= 0
        readonly property bool isCursor: index === settings.cursor
        readonly property bool isRecording: settings.editable(modelData) && settings.recording === modelData.id
        readonly property var keys: settings.keysOf(modelData)
        readonly property var clashes: modelData.kind === "action" ? Keymap.conflictsFor(settings.overrides, modelData.id) : []

        width: list.width
        height: header ? Math.max(Style.space(modelData.note ? 58 : 44), sectionHeading.implicitHeight + Style.space(16))
              : (account ? Style.space(modelData.kind === "profilePhoto" ? 66
                                       : (["session", "storage", "profileField", "profilePhone", "privacy", "blocked", "password", "accountTtl", "autoDelete", "proxy", "reactionsSeen", "sound",
                                           "scope", "previews", "download", "folder", "folderName", "folderFlag"]
                                            .indexOf(modelData.kind) >= 0 ? 58 : 44))
                         : Style.space(clashes.length || modelData.kind === "global" ? 58 : 42))
        radius: Style.cornerRadius
        color: row.isCursor && !row.header ? settings.app.selected
             : (rowArea.containsMouse && !row.header ? Qt.rgba(settings.app.foreground.r, settings.app.foreground.g, settings.app.foreground.b, 0.04) : "transparent")

        Column {
          id: sectionHeading
          visible: row.header
          anchors.left: parent.left
          anchors.bottom: parent.bottom
          anchors.leftMargin: Style.space(4)
          anchors.right: parent.right
          anchors.rightMargin: Style.space(4)
          anchors.bottomMargin: Style.space(6)
          spacing: Style.space(2)
          Text {
            width: parent.width
            wrapMode: Text.Wrap
            text: row.modelData.title || ""
            color: settings.app.foreground
            font.family: settings.app.fontFamily
            font.pixelSize: Style.font.bodySmall
            font.bold: true
          }
          Text {
            width: parent.width
            wrapMode: Text.Wrap
            visible: !!row.modelData.note
            text: row.modelData.note || ""
            color: settings.app.muted
            font.family: settings.app.fontFamily
            font.pixelSize: Style.font.caption
          }
        }

        // ---------------------------------------------- account rows
        ColumnLayout {
          visible: row.account
          anchors.fill: parent
          anchors.leftMargin: Style.space(["session", "blockedSender", "folderChat"].indexOf(row.modelData.kind) >= 0 ? 30 : 14)
          anchors.rightMargin: Style.space(14)
          anchors.topMargin: Style.space(6)
          anchors.bottomMargin: Style.space(6)
          spacing: Style.space(2)

          RowLayout {
            Layout.fillWidth: true
            spacing: Style.space(8)

            Avatar {
              visible: row.modelData.kind === "profilePhoto"
              app: settings.app
              chat: settings.profileChat
              size: Style.space(40)
              Layout.preferredWidth: size
              Layout.preferredHeight: size
            }

            Text {
              Layout.fillWidth: true
              elide: Text.ElideRight
              textFormat: Text.PlainText
              text: row.modelData.kind === "session" ? Model.sessionTitle(row.modelData.session)
                  : row.modelData.kind === "blockedSender" ? row.modelData.sender.name
                  : row.modelData.kind === "folderChat" ? (Model.chatTitle(Model.findChat(settings.app.chats || [], row.modelData.chatId), settings.app.meId)
                                                           || "A chat that is in none of your lists")
                  : row.modelData.label + (row.modelData.kind === "sessions" && settings.sessions.length ? "  (" + settings.sessions.length + ")" : "")
              color: ["logout", "otherSessions", "passwordOff", "folderDelete"].indexOf(row.modelData.kind) >= 0 ? settings.app.urgent : settings.app.foreground
              font.family: settings.app.fontFamily
              font.pixelSize: ["session", "blockedSender", "folderChat"].indexOf(row.modelData.kind) >= 0 ? Style.font.bodySmall : Style.font.body
            }
            Text {
              visible: !settings.narrow || row.isCursor
              Layout.maximumWidth: row.width * 0.4
              elide: Text.ElideRight
              textFormat: Text.PlainText
              text: ({ profileField: "Enter changes it", privacy: "Enter changes it", accountTtl: "Enter changes it", autoDelete: "Enter changes it", proxy: "Enter uses it or stops", proxyAdd: "Enter", reactionsSeen: "Enter changes it", sound: "Enter changes it", soundHear: "Enter plays it",
                        bridge: "Enter enables or restores the previous connection", scope: "Enter changes it", previews: "Enter changes it", download: "Enter changes it",
                       folder: "Enter opens it", newFolder: "Enter", folderName: "Enter changes it", folderFlag: "Enter changes it",
                       folderChat: "Enter takes it off", folderAdd: "Enter", folderSave: "Enter", folderDelete: "Enter",
                       blocked: settings.blockedOpen ? "Enter hides them" : "Enter shows them", blockedSender: "Enter unblocks",
                       password: settings.password && settings.password.hasPassword ? "Enter changes the password" : "Enter turns it on",
                       passwordOff: "Enter", passwordCode: "Enter",
                       profilePhoto: settings.profile && (settings.profile.photo || settings.profile.photoId)
                                     ? "Enter changes it  ·  Backspace removes it" : "Enter sets one",
                       storage: "Enter clears the cache", sessions: settings.sessionsOpen ? "Enter hides them" : "Enter shows them",
                       session: row.modelData.session && row.modelData.session.current ? "this computer" : "Enter signs it out",
                       otherSessions: "Enter", logout: "Enter" })[row.modelData.kind] || ""
              color: settings.app.muted
              font.family: settings.app.fontFamily
              font.pixelSize: Style.font.caption
            }
          }

          Text {
            Layout.fillWidth: true
            visible: text !== ""
            elide: Text.ElideRight
            textFormat: Text.PlainText
            text: row.modelData.kind === "storage" ? Model.storageText(settings.storage)
                : row.modelData.kind === "session" ? Model.sessionDetail(row.modelData.session, settings.nowMs)
                : row.modelData.kind === "profileField" ? Model.profileValue(settings.profile, row.modelData.field)
                : row.modelData.kind === "profilePhone" ? Model.profileValue(settings.profile, "phone")
                : row.modelData.kind === "privacy" ? Model.privacyText(settings.privacy[row.modelData.id])
                : row.modelData.kind === "blocked" ? Model.blockedText(settings.blocked)
                : row.modelData.kind === "password" ? Model.passwordText(settings.password)
                : row.modelData.kind === "accountTtl" ? Model.ttlText(settings.accountTtl)
                : row.modelData.kind === "autoDelete" ? (settings.defaultAutoDelete < 0 ? "Loading…" : Model.autoDeleteText(settings.defaultAutoDelete))
                 : row.modelData.kind === "proxy" ? Model.proxyText(row.modelData.proxy, settings.proxyPings[row.modelData.proxy.id], settings.app.connection)
                 : row.modelData.kind === "bridge" ? (settings.bridgeBusy ? "Changing connection…" : (settings.bridge.enabled ? (settings.bridge.running ? "Enabled for this account · localhost:" + settings.bridge.port : "Enabled, but process stopped · toggle off/on to retry") : "Off · Telegram WebSocket hosts only · no external server list"))
                 : row.modelData.kind === "bridgeTest" ? (settings.bridgeTestResult || "Checks Telegram connectivity; leaves your current connection unchanged")
                : row.modelData.kind === "reactionsSeen" ? (settings.app.reactionsSeen ? "Seen when you open the chat" : "Kept until you scroll to them")
                : row.modelData.kind === "showStories" ? (settings.app.showStories ? "Shown" : "Hidden")
                : row.modelData.kind === "sound" ? Model.soundStyleText(settings.app.soundStyle)
                : row.modelData.kind === "scope" ? Model.scopeText(settings.scopes[row.modelData.id])
                : row.modelData.kind === "previews" ? Model.previewsText(settings.scopes)
                : row.modelData.kind === "download" ? Model.downloadText(settings.app.autoDownloadRules, row.modelData.id)
                : row.modelData.kind === "folder" ? Model.folderSummary(row.modelData.folder)
                : row.modelData.kind === "folderName" ? (settings.folderOpen && settings.folderOpen.name ? settings.folderOpen.name : "None yet")
                : row.modelData.kind === "folderFlag" ? (settings.folderOpen && settings.folderOpen[row.modelData.key] ? "Yes" : "No")
                : row.modelData.kind === "profilePhoto" ? (settings.photoBusy ? "Setting your new photo…" : Model.profileValue(settings.profile, "photo"))
                : ""
            color: settings.app.muted
            font.family: settings.app.fontFamily
            font.pixelSize: Style.font.caption
          }
        }

        // ---------------------------------------------- shortcuts
        ColumnLayout {
          visible: !row.header && !row.account
          anchors.fill: parent
          anchors.leftMargin: Style.space(14)
          anchors.rightMargin: Style.space(14)
          anchors.topMargin: Style.space(6)
          anchors.bottomMargin: Style.space(6)
          spacing: Style.space(2)

          RowLayout {
            Layout.fillWidth: true
            spacing: Style.space(8)

            Text {
              Layout.fillWidth: true
              text: row.modelData.label || ""
              elide: Text.ElideRight
              color: settings.app.foreground
              font.family: settings.app.fontFamily
              font.pixelSize: Style.font.body
            }

            Text {
              visible: settings.changed(row.modelData) && !row.isRecording
              text: row.modelData.kind === "global" ? "" : "changed"
              color: settings.app.muted
              font.family: settings.app.fontFamily
              font.pixelSize: Style.font.caption
            }

            Rectangle {
              visible: row.isRecording
              implicitWidth: recordingText.implicitWidth + Style.space(16)
              implicitHeight: Style.space(26)
              radius: Style.cornerRadius
              color: Qt.rgba(settings.app.urgent.r, settings.app.urgent.g, settings.app.urgent.b, 0.15)
              border.width: 1
              border.color: settings.app.urgent
              Text {
                id: recordingText
                anchors.centerIn: parent
                text: settings.recordingAdds ? "Press the key to add  ·  Esc cancels" : "Press the new keys  ·  Esc cancels"
                color: settings.app.foreground
                font.family: settings.app.fontFamily
                font.pixelSize: Style.font.caption
              }
            }

            Repeater {
              model: row.isRecording ? [] : row.keys
              delegate: Rectangle {
                required property var modelData
                implicitWidth: chip.implicitWidth + Style.space(14)
                implicitHeight: Style.space(26)
                radius: Style.cornerRadius
                color: Qt.rgba(settings.app.foreground.r, settings.app.foreground.g, settings.app.foreground.b, 0.08)
                border.width: 1
                border.color: Qt.rgba(settings.app.foreground.r, settings.app.foreground.g, settings.app.foreground.b, 0.18)
                Text {
                  id: chip
                  anchors.centerIn: parent
                  text: row.modelData.kind === "global" ? modelData : Keymap.label(modelData)
                  textFormat: Text.PlainText
                  color: settings.app.foreground
                  font.family: settings.app.fontFamily
                  font.pixelSize: Style.font.caption
                }
              }
            }

            Text {
              visible: !row.isRecording && row.keys.length === 0
              text: row.modelData.kind === "global" ? "Not set" : "Off"
              color: settings.app.muted
              font.family: settings.app.fontFamily
              font.pixelSize: Style.font.caption
              font.italic: true
            }
          }

          Text {
            visible: row.modelData.kind === "global" && !!settings.globals[row.modelData.id]
            text: settings.statusText(row.modelData.id)
            color: (settings.globalStatus[row.modelData.id] || "") === "active" ? settings.app.muted : settings.app.urgent
            font.family: settings.app.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            Layout.fillWidth: true
            visible: row.clashes.length > 0
            elide: Text.ElideRight
            color: settings.app.urgent
            font.family: settings.app.fontFamily
            font.pixelSize: Style.font.caption
            text: {
              var names = []
              for (var i = 0; i < row.clashes.length; i++) {
                for (var j = 0; j < row.clashes[i].ids.length; j++) {
                  var other = row.clashes[i].ids[j]
                  if (other === row.modelData.id) continue
                  var action = Keymap.actionById(other)
                  var section = Keymap.sectionOf(other)
                  names.push(Keymap.label(row.clashes[i].sequence) + " is also " + (action ? action.label : other)
                             + (section ? " (" + section.title + ")" : ""))
                }
              }
              return names.join("  ·  ")
            }
          }
        }

        MouseArea {
          id: rowArea
          anchors.fill: parent
          enabled: !row.header && !settings.confirm && !settings.editing && settings.recording === ""
          hoverEnabled: true
          acceptedButtons: Qt.LeftButton | Qt.RightButton
          cursorShape: Qt.PointingHandCursor
          onClicked: function (mouse) {
            settings.cursor = row.index
            settings.forceActiveFocus()
            if (row.account) settings.activate(row.modelData)
            else if (mouse.button === Qt.RightButton) settings.reset()
            else settings.startRecording(false)
          }
        }
      }
    }
  }
}
