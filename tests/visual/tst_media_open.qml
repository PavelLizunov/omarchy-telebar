import QtQuick
import QtTest
import qs.Commons
import "../../shell" as Shell
import "../../app" as App

TestCase {
  id: tests
  name: "MediaConsumerOpening"
  when: windowShown
  visible: true
  width: 800
  height: 650
  FixtureApp { id: model }
  QtObject {
    id: mediaHost
    property var keys: ({})
    property string fontFamily: "Sans Serif"
    property var shownChat: model.chats[0]
    property var service: model
    property var files: ({})
    property color background: model.background
    property color foreground: model.foreground
    property color selected: model.selected
    property color accent: model.accent
    property color urgent: model.urgent
    function fileOf(file) { return files[file.id] || file }
    function fetchNow(file) {}
    function fetch(file) {}
    function stillThumb(media) { return media.thumb ? media.thumb.file : null }
    function urlOf(file) { return file && file.path ? "file://" + file.path : "" }
    function duration() { return "0:10" }
    function hints() { return "" }
  }
  Component { id: mediaScene; Shell.MediaViewer { width: 800; height: 650; host: mediaHost } }
  Component { id: photoScene; App.PhotoViewer { width: 800; height: 650; app: model } }
  SignalSpy { id: spy }
  function cleanup() { spy.target = null; spy.signalName = ""; spy.clear(); mediaHost.files = ({}); model.photoCanSave = true; model.photoError = ""; model.holdPhotoRequest = false; model.pendingPhotoRequest = null; model.lastRequest = null }

  function test_photo_context_menu_data() {
    return [{ tag: "window-radius0", quick: false, radius: 0 }, { tag: "quick-radius0", quick: true, radius: 0 },
            { tag: "window-radius8", quick: false, radius: 8 }, { tag: "quick-radius8", quick: true, radius: 8 }]
  }
  function test_photo_context_menu(data) {
    Style.cornerRadius = data.radius
    var photo = model.photoMessage
    var view = createTemporaryObject(data.quick ? mediaScene : photoScene, tests,
                                    data.quick ? { items: [photo], messageId: 50 } : { messages: [photo], messageId: 50 })
    verify(view !== null)
    spy.target = view
    spy.signalName = "closed"
    mouseClick(view, 400, 300, Qt.RightButton)
    tryCompare(view.actions.menu, "visible", true)
    compare(spy.count, 0, "Right click must not close the viewer")
    var card = view.actions.menu.children[1]
    compare(card.radius, data.radius)
    verify(card.x >= 0 && card.y >= 0 && card.x + card.width <= view.width && card.y + card.height <= view.height)
    mouseClick(view, 10, 10, Qt.LeftButton)
    compare(view.actions.menu.visible, false, "Outside click dismisses only the menu")
    compare(spy.count, 0)
    mouseClick(view, view.width - 2, view.height - 2, Qt.RightButton)
    tryCompare(view.actions.menu, "visible", true)
    verify(card.x + card.width <= view.width && card.y + card.height <= view.height, "Edge menu stays inside viewer")
    compare(model.lastRequest.cmd, "message.properties")
    compare(model.lastRequest.args.messageId, 50)
    keyClick(Qt.Key_Escape)
    compare(view.actions.menu.visible, false)
    compare(spy.count, 0, "Escape dismisses the menu first")
    verify(view.activeFocus, "Menu dismissal restores viewer focus")
    keyClick(Qt.Key_F10, Qt.ShiftModifier)
    tryCompare(view.actions.menu, "visible", true)
    keyClick(Qt.Key_Return)
    compare(model.lastRequest.cmd, "file.save")
    compare(model.lastRequest.args.fileId, 50)
    compare(model.lastRequest.args.chatId, photo.chatId)
    tryCompare(view.actions, "status", "Saved to Downloads")
    verify(view.activeFocus)
    mouseClick(view, 400, 300, Qt.RightButton)
    tryCompare(view.actions.menu, "visible", true)
    mouseClick(view.actions.menu, card.x + card.width / 2, card.y + card.height - 22, Qt.LeftButton)
    compare(model.lastRequest.cmd, "file.copyImage")
    tryCompare(view.actions, "status", "Image copied")
    mouseClick(view, 400, 300, Qt.RightButton)
    tryCompare(view.actions.menu, "visible", true)
    mouseClick(view.actions.menu, card.x + card.width / 2, card.y + 22, Qt.LeftButton)
    compare(model.lastRequest.cmd, "file.save")
    mouseClick(view, 400, 300, Qt.RightButton)
    tryCompare(view.actions.menu, "visible", true)
    keyClick(Qt.Key_Down)
    keyClick(Qt.Key_Return)
    compare(model.lastRequest.cmd, "file.copyImage")
    model.photoError = "Clipboard unavailable"
    view.actions.perform("copy")
    tryCompare(view.actions, "status", "Clipboard unavailable")
    model.photoError = ""
    view.actions.file = { id: 50, path: "" }
    view.actions.perform("copy")
    compare(view.actions.status, "Downloading… try again when it is done")
    compare(model.lastRequest.cmd, "file.copyImage", "No request for an incomplete file")
    model.photoCanSave = false
    mouseClick(view, 400, 300, Qt.RightButton)
    tryCompare(view.actions, "status", "This photo cannot be saved or copied")
    compare(view.actions.menu.visible, false)
    compare(view.actions.allowed, false)
    model.photoCanSave = true
    model.holdPhotoRequest = true
    mouseClick(view, 400, 300, Qt.RightButton)
    verify(model.pendingPhotoRequest !== null)
    if (data.quick) view.items = []
    else view.messages = []
    model.completePhotoRequest()
    wait(0)
    compare(view.actions.menu.visible, false, "Stale permissions must not open a menu for another message")
    view.forceActiveFocus()
    wait(0)
    keyClick(Qt.Key_Escape)
    compare(spy.count, 1)
  }
  function test_video_photo_and_delayed_video_open() {
    var photo = model.photoMessage
    var video = { id: 60, chatId: 101, date: 1790726000, content: { kind: "video", text: "Synthetic video", media: { duration: 10, file: { id: 60, path: "/synthetic/video.mp4" }, thumb: { file: { id: 50, path: model.imagePath } } } } }
    var view = createTemporaryObject(mediaScene, tests, { items: [photo, video], messageId: 50 })
    verify(view !== null, "Actual media viewer and its icons must load")
    compare(view.kind, "photo")
    view.step(1)
    compare(view.kind, "video")
    spy.target = view
    spy.signalName = "played"
    view.play()
    compare(model.lastRequest.cmd, "file.open")
    compare(model.lastRequest.args.fileId, 60)
    tryCompare(spy, "count", 1)
    spy.clear()
    mediaHost.files = { "60": { id: 60, path: "" } }
    view.play()
    compare(view.waitingFileId, 60)
    mediaHost.files = { "60": { id: 60, path: "/synthetic/video.mp4" } }
    tryCompare(spy, "count", 1)
    view.step(-1)
    compare(view.kind, "photo")
    spy.clear()
    spy.signalName = "closed"
    view.forceActiveFocus()
    keyClick(Qt.Key_Escape)
    compare(spy.count, 1)
    var fullPhoto = createTemporaryObject(photoScene, tests, { messages: [photo], messageId: 50 })
    verify(fullPhoto !== null)
    compare(fullPhoto.index, 0)
  }
}
