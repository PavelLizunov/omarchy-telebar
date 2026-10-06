import QtQuick

Item {
  property string text: ""
  property bool waitForEnd: false
  signal read(string data)
  signal streamFinished()
}
