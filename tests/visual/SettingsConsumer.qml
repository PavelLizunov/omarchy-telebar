import QtQuick
import qs.Commons
import "../../app" as App

Rectangle {
  id: preview
  objectName: "review-root"
  color: model.background
  property int themeRadius: 0
  property int fontSize: 15
  property string state: "connection"
  property bool ready: false
  property bool storiesShown: true
  FixtureApp { id: model; showStories: preview.storiesShown }
  Component.onCompleted: { Style.fontBaseSize = fontSize; Style.cornerRadius = themeRadius }
  App.SettingsView { id: settings; objectName: "review-content"; anchors.fill: parent; app: model }
  Timer {
    interval: 40
    running: true
    repeat: true
    property int phase: 0
    onTriggered: {
      if (!settings.profile) return
      if (phase++ === 0) {
        if (preview.state === "edit") settings.startEditing({ field: "bio", label: "Bio" })
        else if (preview.state === "confirm") settings.ask("Clear the cache? Downloaded photos, videos and files are deleted from this computer; they download again when you open them.", function () {})
        else if (preview.state === "password-flow") settings.startPasswordFlow("change")
        else if (preview.state === "proxy-flow") settings.startProxyFlow("mtproto")
        else if (preview.state === "folder") settings.openFolder(null)
        else {
          if (preview.state === "sessions") settings.sessionsOpen = true
          if (preview.state === "blocked") { settings.blockedOpen = true; settings.blocked = { total: 1, senders: [{ type: "user", id: 101, name: "Blocked synthetic user" }] } }
           var kinds = { profile: "profilePhoto", privacy: "privacy", blocked: "blockedSender", security: "password", notifications: "scope", downloads: "download", folders: "folder", sessions: "session", account: "storage", globals: "global", stories: "showStories", shortcuts: "action", connection: "bridge" }
          var kind = kinds[preview.state] || "bridge"
          var index = settings.rows.findIndex(function (row) { return row.kind === kind })
          if (index > 0) settings.move(index - settings.cursor)
        }
        return
      }
      preview.ready = true
      stop()
    }
  }
}
