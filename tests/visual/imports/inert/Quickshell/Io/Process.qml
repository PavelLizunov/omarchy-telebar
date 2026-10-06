import QtQuick
Item {
  property bool running: false
  property var command: []
  property var environment: ({})
  property bool clearEnvironment: false
  property bool stdinEnabled: false
  property bool waitForEnd: false
  property var stdout: null
  property var stderr: null
  signal started()
  signal exited(int code)
  function write(s) {}
  function signal(sig) {}
}
