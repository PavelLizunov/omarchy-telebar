import QtQuick
import QtQuick.Layouts
import qs.Commons

// A retained generated recording; retries are explicit and disabled for unknown delivery.
Rectangle {
  id: bar
  property var app
  property var client
  property var record: null
  property bool busy: false
  property string error: ""
  property string consumedToken: ""
  property int serial: 0
  readonly property bool retryable: !!bar.record && (bar.record.state === "rejected" || bar.record.state === "prepareFailed")
  readonly property bool resolved: !!bar.record && (bar.retryable || bar.record.state === "unknown")
  signal notice(string text)
  objectName: "recorded-send-bar"
  color: Qt.rgba(app.foreground.r, app.foreground.g, app.foreground.b, 0.05)
  radius: Style.cornerRadius
  implicitHeight: column.implicitHeight + Style.space(20)
  onRecordChanged: { bar.serial++; bar.busy = false; bar.error = "" }

  function act(retry) {
    if (!bar.record || bar.busy || !bar.resolved || bar.consumedToken === bar.record.token || (retry && !bar.retryable)) return
    var token = bar.record.token
    var account = bar.record.account
    var serial = ++bar.serial
    bar.busy = true
    bar.error = ""
    client.request(retry ? "recording.retry" : "recording.discard", { token: token, account: account }, function (answer) {
      if (serial !== bar.serial || !bar.record || bar.record.token !== token) return
      bar.busy = false
      if (!answer.ok) { bar.error = answer.error || "The recorded message could not be resolved"; bar.notice(bar.error) }
      else bar.consumedToken = token
    })
  }

  ColumnLayout {
    id: column
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.margins: Style.space(10)
    spacing: Style.space(6)
    Text {
      Layout.fillWidth: true
      text: bar.error || (bar.record && bar.record.state === "prepareFailed" ? "The recording could not be prepared. Retry uses the kept recording and its original destination."
          : bar.retryable ? "Telegram rejected the recording. Retry sends it to its original chat and reply."
          : bar.record && bar.record.state === "unknown" ? "Delivery is unknown. Check Telegram before dismissing. Retry is disabled."
          : "Preparing or sending the recorded message…")
      textFormat: Text.PlainText
      wrapMode: Text.Wrap
      color: app.foreground
      font.family: app.fontFamily
      font.pixelSize: Style.font.bodySmall
    }
    RowLayout {
      Layout.fillWidth: true
      visible: bar.resolved
      Button {
        objectName: "recorded-send-retry"
        app: bar.app
        text: "Retry recording"
        enabled: bar.retryable && !bar.busy && bar.consumedToken !== (bar.record ? bar.record.token : "")
        onClicked: bar.act(true)
      }
      Button {
        objectName: "recorded-send-discard"
        app: bar.app
        text: bar.retryable ? "Discard recording" : "Dismiss"
        enabled: !bar.busy && bar.consumedToken !== (bar.record ? bar.record.token : "")
        onClicked: bar.act(false)
      }
      Item { Layout.fillWidth: true }
    }
  }
}
