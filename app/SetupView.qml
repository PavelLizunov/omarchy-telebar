import QtQuick
import qs.Commons
import "Model.js" as Model

// Public client identity from Telegram Android BuildVars.java (aac1efcb83d6575).
// This is not a user's login secret. Custom values use the same keyring owner.
FocusScope {
  id: view

  property var app
  property var client
  property bool busy: false
  property bool customApi: false
  property string error: ""

  function submit() {
    if (view.busy) return
    var id = view.customApi ? apiId.text.trim() : "4"
    var hash = view.customApi ? apiHash.text.trim() : "014b35b6184100b085b0d0572f9b5103"
    apiId.error = Model.validApiId(id) ? "" : "The API id is a number."
    apiHash.error = Model.validApiHash(hash) ? "" : "The API hash is 32 characters, 0-9 and a-f."
    if (apiId.error || apiHash.error) return
    view.busy = true
    view.error = ""
    client.request("credentials.set", { apiId: parseInt(id, 10), apiHash: hash }, function (answer) {
      view.busy = false
      if (!answer.ok) view.error = answer.error || "The credentials were not accepted."
      if (answer.ok) apiHash.clear()
    })
  }

  Flickable {
    anchors.fill: parent
    contentHeight: column.implicitHeight + Style.space(80)
    clip: true

    Column {
      id: column
      width: Math.min(parent.width - Style.space(64), Style.space(520))
      anchors.horizontalCenter: parent.horizontalCenter
      y: Math.max(Style.space(40), (view.height - implicitHeight) / 2)
      spacing: Style.spacing.md

      Row {
        spacing: Style.space(14)

        RingMark {
          anchors.verticalCenter: parent.verticalCenter
          size: Math.round(Style.font.displayLarge * 1.35)
          color: app.foreground
        }
        Text {
          anchors.verticalCenter: parent.verticalCenter
          text: "Telebar"
          color: app.foreground
          font.family: app.fontFamily
          font.pixelSize: Style.font.displayLarge
          font.bold: true
        }
      }

      Text {
        width: parent.width
        wrapMode: Text.WordWrap
        text: "An unofficial Telegram client for Omarchy. It is not made or endorsed by Telegram."
        color: app.foreground
        font.family: app.fontFamily
        font.pixelSize: Style.font.title
      }

      Text {
        width: parent.width
        wrapMode: Text.WordWrap
        textFormat: Text.StyledText
        linkColor: app.accentText
        text: view.customApi
          ? "Sign in at <a href=\"https://my.telegram.org/apps\">my.telegram.org</a>, open "
            + "<b>API development tools</b>, and copy your API id and hash here. They are kept in your keyring."
          : "Continue with the built-in connection settings, then sign in by phone or QR. "
            + "You do not need to register an API application."
        color: app.foreground
        opacity: 0.85
        font.family: app.fontFamily
        font.pixelSize: Style.font.body
        onLinkActivated: function (link) { Qt.openUrlExternally(link) }
      }

      Item { width: 1; height: Style.space(8) }

      Field {
        id: apiId
        visible: view.customApi
        enabled: !view.busy
        app: view.app
        width: parent.width
        label: "API id"
        placeholder: "1234567"
        maximumLength: 10
        inputMethodHints: Qt.ImhDigitsOnly
        focus: view.customApi
        KeyNavigation.tab: apiHash
        onAccepted: apiHash.forceActiveFocus()
      }

      Field {
        id: apiHash
        visible: view.customApi
        enabled: !view.busy
        app: view.app
        width: parent.width
        label: "API hash"
        placeholder: "32 characters"
        secret: true
        maximumLength: 32
        KeyNavigation.tab: continueButton
        onAccepted: view.submit()
      }

      Text {
        visible: view.error !== ""
        width: parent.width
        wrapMode: Text.WordWrap
        text: view.error
        textFormat: Text.PlainText
        color: app.urgent
        font.family: app.fontFamily
        font.pixelSize: Style.font.bodySmall
      }

      Button {
        id: continueButton
        app: view.app
        text: "Continue to Telegram"
        primary: true
        busy: view.busy
        KeyNavigation.tab: modeButton
        onClicked: view.submit()
      }

      Button {
        id: modeButton
        app: view.app
        text: view.customApi ? "Use built-in connection" : "Use my own API"
        enabled: !view.busy
        KeyNavigation.tab: view.customApi ? apiId : continueButton
        onClicked: {
          view.customApi = !view.customApi
          view.error = ""
          Qt.callLater(function () {
            if (view.customApi) apiId.forceActiveFocus()
            else continueButton.forceActiveFocus()
          })
        }
      }

      Text {
        visible: !view.customApi
        width: parent.width
        wrapMode: Text.WordWrap
        text: "Built-in settings use a public Telegram Android API profile. Telegram may limit shared profiles; you can use your own API instead."
        textFormat: Text.PlainText
        color: app.muted
        font.family: app.fontFamily
        font.pixelSize: Style.font.bodySmall
      }
    }
  }

  Component.onCompleted: continueButton.forceActiveFocus()
}
