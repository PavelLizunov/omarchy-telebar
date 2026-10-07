import QtQuick
Item {
  property string path: ""
  property bool connected: false
  property var parser: null
  signal connectionStateChanged()
  function write(value) {}
  function flush() {}
}
