import QtQuick
import QtQuick.Layouts
import QtCore
import Qt.labs.folderlistmodel
import qs.Commons

// Read-only Qt directory model; no native GTK chooser or helper process.
FocusScope {
  id: picker
  property var app
  property string title: "Choose files"
  property bool multiple: false
  property var nameFilters: ["*"]
  property url currentFolder: ""
  property var selectedFiles: []
  property int cursor: 0
  property string error: ""
  readonly property url selectedFile: selectedFiles.length ? selectedFiles[0] : ""
  readonly property bool ready: visible && files.status === FolderListModel.Ready
  signal accepted()
  signal rejected()
  visible: false
  z: 65
  anchors.fill: parent

  function open() {
    selectedFiles = []
    error = ""
    if (!String(currentFolder)) navigate(StandardPaths.writableLocation(StandardPaths.HomeLocation))
    cursor = 0
    visible = true
    list.forceActiveFocus()
  }
  function reject() { visible = false; selectedFiles = []; rejected() }
  function confirmCursor() {
    if (files.count && files.isFolder(cursor)) activate(cursor)
    else if (selectedFiles.length) accept()
    else { toggle(cursor); if (!multiple) accept() }
  }
  function navigate(url) {
    if (String(url).indexOf("file:///") !== 0) { error = "Enter an absolute local folder path"; return }
    error = ""
    currentFolder = url
    cursor = 0
    list.positionViewAtBeginning()
  }
  function goToPath() {
    var path = folderField.text
    if (path.charAt(0) !== "/" || path.indexOf("\0") >= 0) { error = "Enter an absolute local folder path"; return }
    navigate("file://" + path.split("/").map(encodeURIComponent).join("/"))
    list.forceActiveFocus()
  }
  function toggle(index) {
    if (index < 0 || index >= files.count || files.isFolder(index)) return
    var url = String(files.get(index, "fileUrl"))
    if (!multiple) { selectedFiles = [url]; return }
    var next = selectedFiles.slice()
    var at = next.indexOf(url)
    if (at >= 0) next.splice(at, 1)
    else if (next.length < 10) next.push(url)
    else { error = "Choose at most ten files"; return }
    selectedFiles = next
    error = ""
  }
  function activate(index) {
    if (index < 0 || index >= files.count) return
    cursor = index
    if (files.isFolder(index)) navigate(files.get(index, "fileUrl"))
    else toggle(index)
  }
  function accept() {
    if (!selectedFiles.length) return
    visible = false
    accepted()
  }
  function move(delta) {
    cursor = Math.max(0, Math.min(files.count - 1, cursor + delta))
    list.positionViewAtIndex(cursor, ListView.Contain)
  }
  onCurrentFolderChanged: {
    try { folderField.text = decodeURIComponent(String(currentFolder).replace(/^file:\/\//, "")) }
    catch (e) { folderField.text = String(currentFolder).replace(/^file:\/\//, "") }
  }
  Keys.onEscapePressed: reject()
  FolderListModel {
    id: files
    folder: picker.visible ? picker.currentFolder : ""
    nameFilters: picker.nameFilters
    showDirsFirst: true
    showDotAndDotDot: false
    showOnlyReadable: true
    sortField: FolderListModel.Name
    sortCaseSensitive: false
  }
  Rectangle { anchors.fill: parent; color: Qt.rgba(0, 0, 0, 0.45) }
  MouseArea { anchors.fill: parent; onClicked: picker.reject() }
  Rectangle {
    objectName: "file-picker-card"
    anchors.centerIn: parent
    width: Math.min(parent.width - Style.space(24), Style.space(680))
    height: Math.min(parent.height - Style.space(24), Style.space(540))
    color: app.background
    radius: Style.cornerRadius
    border.width: 1
    border.color: app.border
    MouseArea { anchors.fill: parent }
    ColumnLayout {
      anchors.fill: parent
      anchors.margins: Style.space(12)
      spacing: Style.space(8)
      Text { Layout.fillWidth: true; Layout.minimumWidth: 0; text: picker.title; elide: Text.ElideRight; color: app.foreground; font.family: app.fontFamily; font.pixelSize: Style.font.title; font.bold: true }
      RowLayout {
        Layout.fillWidth: true
        Button { objectName: "file-picker-up"; app: picker.app; text: "Up"; implicitWidth: Style.space(60); enabled: String(files.parentFolder) !== ""; onClicked: picker.navigate(files.parentFolder) }
        Rectangle {
          Layout.fillWidth: true
          Layout.minimumWidth: 0
          Layout.preferredWidth: 0
          Layout.preferredHeight: Style.space(38)
          radius: Style.cornerRadius
          color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.05)
          border.width: 1
          border.color: folderField.activeFocus ? app.accent : app.border
          TextInput {
            id: folderField
            objectName: "file-picker-path"
            anchors.fill: parent
            anchors.margins: Style.space(8)
            verticalAlignment: TextInput.AlignVCenter
            clip: true
            maximumLength: 4096
            color: app.foreground
            selectionColor: app.accent
            font.family: app.fontFamily
            font.pixelSize: Style.font.body
            onAccepted: picker.goToPath()
          }
        }
      }
      ListView {
        id: list
        objectName: "file-picker-list"
        Layout.fillWidth: true
        Layout.minimumWidth: 0
        Layout.fillHeight: true
        clip: true
        model: picker.ready ? files : null
        boundsBehavior: Flickable.StopAtBounds
        Keys.onDownPressed: picker.move(1)
        Keys.onUpPressed: picker.move(-1)
        Keys.onSpacePressed: picker.activate(picker.cursor)
        Keys.onReturnPressed: picker.confirmCursor()
        Keys.onEnterPressed: picker.confirmCursor()
        Keys.onPressed: function (event) {
          if (event.key === Qt.Key_Backspace && String(files.parentFolder)) { picker.navigate(files.parentFolder); event.accepted = true }
        }
        WheelScroll { view: list }
        delegate: Rectangle {
          id: row
          required property int index
          required property string fileName
          required property url fileUrl
          required property bool fileIsDir
          readonly property bool chosen: picker.selectedFiles.indexOf(String(fileUrl)) >= 0
          Accessible.role: Accessible.ListItem
          Accessible.name: fileName
          Accessible.selected: chosen
          objectName: "file-picker-entry-" + fileName
          width: list.width
          height: Style.space(38)
          radius: Style.cornerRadius
          color: chosen ? app.selected : "transparent"
          border.width: index === picker.cursor && list.activeFocus ? 1 : 0
          border.color: app.accent
          RowLayout {
            anchors.fill: parent
            anchors.margins: Style.space(8)
            Icon { name: row.fileIsDir ? "folder" : row.chosen ? "check" : "file"; color: app.muted; size: Style.font.body }
            Text { Layout.fillWidth: true; text: row.fileName; textFormat: Text.PlainText; elide: Text.ElideMiddle; color: app.foreground; font.family: app.fontFamily; font.pixelSize: Style.font.body }
          }
          MouseArea { anchors.fill: parent; onClicked: { picker.activate(row.index); list.forceActiveFocus() } }
        }
        Text {
          anchors.centerIn: parent
          width: parent.width - Style.space(16)
          horizontalAlignment: Text.AlignHCenter
          wrapMode: Text.Wrap
          visible: files.count === 0
          text: files.status === FolderListModel.Loading ? "Loading files…" : "No readable matching files here. Check the folder path."
          color: app.muted
          font.family: app.fontFamily
          font.pixelSize: Style.font.bodySmall
        }
      }
      Text {
        Layout.fillWidth: true
        Layout.minimumWidth: 0
        text: error || (picker.multiple ? picker.selectedFiles.length + " selected · Click or Space toggles a file" : "Select an image")
        wrapMode: Text.Wrap
        color: error ? app.urgent : app.muted
        font.family: app.fontFamily
        font.pixelSize: Style.font.caption
      }
      RowLayout {
        Layout.fillWidth: true
        Item { Layout.fillWidth: true }
        Button { objectName: "file-picker-cancel"; Layout.fillWidth: true; Layout.minimumWidth: 0; implicitWidth: Style.space(90); app: picker.app; text: "Cancel"; onClicked: picker.reject() }
        Button { objectName: "file-picker-accept"; Layout.fillWidth: true; Layout.minimumWidth: 0; implicitWidth: Style.space(90); app: picker.app; text: "Open"; primary: true; enabled: picker.selectedFiles.length > 0; onClicked: picker.accept() }
      }
    }
  }
}
