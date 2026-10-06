import QtQuick
import QtTest
import qs.Commons
import "../../app" as App
import "../../app/Model.js" as Model

TestCase {
  id: tests
  name: "RichMessageConsumers"
  when: windowShown
  visible: true
  width: 800
  height: 700
  Component { id: scene; RichConsumer {} }
  Component { id: photos; App.PhotoViewer { width: 800; height: 650; app: photoHost } }
  QtObject {
    id: photoHost
    property var shortcuts: ({})
    property string fontFamily: "Sans Serif"
    property color background: "#101315"
    property color foreground: "#cacccc"
    property color selected: "#283539"
    property color accent: "#8ab4b8"
    property color urgent: "#dd8e8e"
    property var chosen: null
    function fileState(file) { return file }
    function download() {}
    function openPhoto(message) { chosen = message }
  }
  Component { id: reader; App.RichMessageView { width: 300; compact: true; client: fakeClient } }
  QtObject {
    id: fakeClient
    property string activeAccount: "default"
    property var callback: null
    property int requests: 0
    function request(cmd, args, done) { requests++; callback = done }
    function fileOf(file) { return file }
    function stillOf(kind, media) { return media.file }
    function fetchPicture() {}
  }
  function init() { failOnWarning(/.*/) }
  function cleanup() { Style.cornerRadius = 0; fakeClient.callback = null; fakeClient.requests = 0; fakeClient.activeAccount = "default" }
  function test_order_and_radius_data() {
    return [{ tag: "window0", compact: false, radius: 0 }, { tag: "window8", compact: false, radius: 8 },
            { tag: "quick0", compact: true, radius: 0 }, { tag: "quick8", compact: true, radius: 8 }]
  }
  function test_order_and_radius(data) {
    var view = createTemporaryObject(scene, tests, { compact: data.compact, themeRadius: data.radius })
    verify(view !== null)
    tryCompare(view, "ready", true)
    var post = null
    tryVerify(function () { post = findChild(view, data.compact ? "quick-rich-post-27" : "rich-post-27"); return post !== null })
    compare(post.blocks.length, 3)
    var before = findChild(post, "rich-block-0")
    var photo = findChild(post, "rich-block-1")
    var after = findChild(post, "rich-block-2")
    tryVerify(function () { return photo.height > 0 && after.y > photo.y })
    verify(before.y + before.height <= photo.y)
    verify(photo.y + photo.height <= after.y)
    verify(post.width > 0 && post.width <= view.width)
    compare(Style.cornerRadius, data.radius)
    verify(findChild(post, "rich-text-0").text.indexOf("<b>") >= 0)
    if (!data.compact) compare(view.consumer.bubbleItem.radius, data.radius)
    var changed = Object.assign({}, view.richContent, { blocks: [view.richContent.blocks[2], view.richContent.blocks[1], view.richContent.blocks[0]] })
    view.richContent = changed
    if (data.compact) view.serviceModel.messageEvent("messageContent", { chatId: 101, messageId: 27, content: changed })
    tryVerify(function () { return findChild(post, "rich-text-0").text.indexOf("After") >= 0 })
  }
  function test_partial_error_and_stale_response() {
    var message = { id: 1, chatId: 101, content: { kind: "rich", full: false, blocks: [] } }
    var view = createTemporaryObject(reader, tests, { message: message })
    verify(view !== null)
    tryVerify(function () { return fakeClient.callback !== null })
    verify(view.loading)
    var old = fakeClient.callback
    view.message = { id: 2, chatId: 101, content: { kind: "rich", full: false, blocks: [] } }
    old({ ok: true, result: { kind: "rich", full: true, blocks: [{ kind: "text", text: "stale", entities: [] }] } })
    compare(view.fetched, null)
    fakeClient.callback({ ok: false, error: "Synthetic unavailable post" })
    compare(view.error, "Synthetic unavailable post")
    verify(!view.loading)
    view.load()
    fakeClient.callback({ ok: true, result: { kind: "rich", full: true, blocks: [{ kind: "text", text: "Full post", entities: [] }] } })
    compare(view.blocks[0].text, "Full post")
    view.message = message
    var accountResponse = fakeClient.callback
    fakeClient.activeAccount = "other"
    accountResponse({ ok: true, result: { full: true, blocks: [] } })
    compare(view.fetched, null)
  }
  function test_multiple_embedded_photos_selection() {
    var blocks = [
      { kind: "photo", text: "", media: { file: { id: 50, path: "", active: false } } },
      { kind: "photo", text: "", media: { file: { id: 60, path: "", active: false } } }
    ]
    var message = { id: 27, chatId: 101, content: { kind: "rich", blocks: blocks } }
    var view = createTemporaryObject(photos, tests, { messages: [message], messageId: 27, fileId: 60 })
    verify(view !== null)
    compare(view.photos.length, 2)
    compare(view.file.id, 60)
    view.step(-1)
    compare(photoHost.chosen.id, 27)
    compare(photoHost.chosen.content.media.file.id, 50)
  }
  function test_compact_media_keyboard_spoiler() {
    var message = { id: 1, chatId: 101, content: { kind: "rich", full: true, blocks: [
      { kind: "photo", text: "", entities: [], spoiler: true, media: { width: 100, height: 100, file: { id: 55, path: "" } } }
    ] } }
    var view = createTemporaryObject(reader, tests, { message: message })
    var picture = findChild(view, "rich-media-0")
    verify(picture !== null)
    var revealed = false, selected = null
    view.revealRequested.connect(function () { revealed = true; view.revealed = true })
    view.mediaActivated.connect(function (message) { selected = message })
    picture.forceActiveFocus()
    keyClick(Qt.Key_Return)
    verify(revealed)
    compare(selected, null)
    keyClick(Qt.Key_Space)
    compare(selected.id, 1)
    compare(selected.content.media.file.id, 55)
  }
}
