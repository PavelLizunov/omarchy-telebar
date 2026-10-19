import QtQuick
import QtQuick.Dialogs

// The desktop platform owns file browsing; callers keep selection and cancel routes.
FileDialog {
  property var app
  property bool multiple: false
  fileMode: multiple ? FileDialog.OpenFiles : FileDialog.OpenFile
  nameFilters: [qsTr("All files (*)")]
}
