import QtQuick
import qs.Commons
import "../../app" as App

Rectangle {
  id: preview
  width: 300
  height: 650
  color: model.background
  property int fontSize: 15
  property int themeRadius: 0
  property bool ready: false
  property bool accountMenu: false
  FixtureApp { id: model }
  Component.onCompleted: {
    Style.fontBaseSize = fontSize
    Style.cornerRadius = themeRadius
    search.searchInChat(101, "Very long conversation title for message search")
    var input = findSearch(search)
    if (input) input.text = "demo"
    Qt.callLater(function () {
      search.messageResults = [{ id: 40, chatId: 101, date: model.nowMs / 1000, senderName: "Alex Demo", content: { kind: "text", text: "A matching message with enough content to wrap or truncate safely.", entities: [] } }]
      search.searching = false
      if (preview.accountMenu) {
        model.accounts = Array.from({ length: 10 }, function (_, i) { return { id: "account" + i, name: "Long example account " + i, phone: "+10000000000", unread: i } })
        var menu = locateMenu(preview)
        if (menu) menu.visible = true
      }
      preview.ready = true
    })
  }
  function findSearch(item) {
    if (item.maximumLength === 128 && item.selectAll) return item
    for (var child of item.children || []) { var found = findSearch(child); if (found) return found }
    return null
  }
  function locateMenu(item) {
    if (item.objectName === "account-menu") return item
    for (var child of item.children || []) { var found = locateMenu(child); if (found) return found }
    return null
  }
  App.ChatList { id: search; anchors.fill: parent; app: model; menuHost: preview; chats: model.chats; allChats: model.chats; tabs: [{ key: "main", title: "All" }]; nowMs: model.nowMs }
}
