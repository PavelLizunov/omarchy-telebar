import QtQuick
import QtTest
import qs.Commons

TestCase {
  id: tests
  name: "CompactMessageSearch"
  when: windowShown
  visible: true
  width: 650
  height: 700
  Component { id: scene; QuickConsumer {} }
  function cleanup() { Style.fontBaseSize = 12; Style.cornerRadius = 0 }
  function oldMessage(id, text) {
    return { id: id, chatId: 101, date: 1690726000 + id, outgoing: false, senderName: "Alex Demo",
      content: { kind: "text", text: text, entities: [] } }
  }
  function openSearch(view, query) {
    var button = findChild(view, "quick-search-button")
    mouseClick(button, button.width / 2, button.height / 2)
    var input = findChild(view, "quick-search-input")
    tryCompare(input, "activeFocus", true)
    compare(input.Accessible.name, "Search messages in this chat")
    input.text = query
    return input
  }
  function test_old_message_not_chat_title_pointer_navigation_and_latest() {
    var view = createTemporaryObject(scene, tests, { width: 350, height: 560 })
    tryCompare(view, "ready", true)
    var quick = view.consumer, model = view.serviceModel
    model.searchData = [oldMessage(500, "Ancient needle in Telegram history")]
    model.aroundHistory = Array.from({ length: 30 }, function (_, i) { return oldMessage(480 + i, "Context message " + i) })
    var composer = findChild(view, "quick-composer")
    composer.text = "Keep this draft"
    quick.addAttachments([model.imagePath], true)
    var search = openSearch(view, "needle")
    tryCompare(model, "searchRequests", [{ chatId: 101, query: "needle", limit: 30 }])
    tryCompare(quick, "searchingMessages", false)
    compare(quick.results.length, 1)
    compare(quick.results[0].id, 500)
    verify(!quick.history.some(function (m) { return m.id === 500 }), "Result must be older than the loaded history")
    var row = findChild(view, "quick-search-result-500")
    verify(row !== null)
    mouseClick(row, row.width / 2, row.height / 2)
    tryCompare(quick, "searchOpen", false)
    compare(model.lastRequest.cmd, "chat.history")
    compare(model.lastRequest.args.fromMessageId, 500)
    compare(model.lastRequest.args.offset, -15)
    compare(quick.focusedMessageId, 500)
    compare(composer.text, "Keep this draft")
    compare(quick.attachments.length, 1)
    var list = findChild(view, "quick-message-list")
    tryVerify(function () {
      var found = findChild(view, "quick-message-500")
      if (!found) return false
      var p = found.mapToItem(list, 0, 0)
      return p.y >= -1 && p.y < list.height
    })
    var latest = findChild(view, "quick-show-latest")
    tryVerify(function () { return latest.visible && latest.height > 0 && latest.y > 0 })
    wait(30)
    mouseClick(latest, latest.width / 2, latest.height / 2)
    tryCompare(quick, "focusedMessageId", 0)
    tryVerify(function () { return quick.history.some(function (m) { return m.id === 23 }) })
  }
  function test_keyboard_pick_and_empty_state() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    var quick = view.consumer, model = view.serviceModel
    model.searchData = [oldMessage(700, "A needle")]
    var search = openSearch(view, "missing")
    tryCompare(quick, "searchingMessages", false)
    compare(quick.results.length, 0)
    compare(findChild(view, "quick-search-feedback").text, "No messages found")
    search.text = "needle"
    tryCompare(quick, "searchingMessages", false)
    compare(quick.results.length, 1)
    keyClick(Qt.Key_Return)
    tryCompare(quick, "focusedMessageId", 700)
    compare(quick.searchOpen, false)
  }
  function test_debounce_error_retry_and_stale_query_close_account() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    var quick = view.consumer, model = view.serviceModel
    model.holdSearch = true
    model.searchData = [oldMessage(700, "Old needle"), oldMessage(800, "New phrase")]
    var search = openSearch(view, "n")
    search.text = "ne"
    search.text = "needle"
    tryCompare(model, "searchRequests", [{ chatId: 101, query: "needle", limit: 30 }])
    search.text = "phrase"
    tryCompare(model, "searchRequests", [{ chatId: 101, query: "needle", limit: 30 }, { chatId: 101, query: "phrase", limit: 30 }])
    model.completeSearch(0)
    compare(quick.results.length, 0)
    model.searchError = "Synthetic search failure"
    model.completeSearch(1)
    compare(findChild(view, "quick-search-feedback").text, "Synthetic search failure")
    model.searchError = ""
    quick.searchMessages(false)
    model.completeSearch(2)
    compare(quick.results[0].id, 800)
    quick.searchMessages(false)
    quick.closeSearch()
    model.completeSearch(3)
    compare(quick.searchOpen, false)
    quick.focusSearch()
    search.text = "phrase"
    tryVerify(function () { return model.pendingSearches.length === 5 })
    model.accountChanging()
    model.completeSearch(4)
    compare(quick.searchOpen, false)
    compare(quick.messageResults.length, 0)
  }
  function test_pagination_deduplicates_and_stops() {
    var view = createTemporaryObject(scene, tests)
    tryCompare(view, "ready", true)
    var quick = view.consumer, model = view.serviceModel
    model.searchData = Array.from({ length: 35 }, function (_, i) { return oldMessage(1000 - i, "Needle " + i) })
    openSearch(view, "needle")
    tryCompare(quick, "searchingMessages", false)
    compare(quick.results.length, 30)
    compare(quick.nextSearchMessageId, 971)
    var more = findChild(view, "quick-search-more")
    mouseClick(more, more.width / 2, more.height / 2)
    tryCompare(quick, "searchingMessages", false)
    compare(quick.results.length, 35)
    compare(quick.nextSearchMessageId, 0)
    compare(model.searchRequests[1].fromMessageId, 971)
  }
}
