import QtQuick
import QtTest
import qs.Commons

TestCase {
  id: tests
  name: "CompactEmojiChatAccess"
  when: windowShown
  visible: true
  width: 900
  height: 850
  Component { id: scene; EmojiChatConsumer {} }
  function cleanup() { Style.fontBaseSize = 12; Style.cornerRadius = 0 }
  function list(view) { return findChild(view, "chat-message-list") }
  function test_compact_open_preserves_chat_viewport_data() {
    return [{tag:"normal-small",w:520,h:400,font:12,longDraft:false},
      {tag:"draft-small",w:520,h:400,font:12,longDraft:true},
      {tag:"narrow-scaled",w:350,h:480,font:18,longDraft:false},
      {tag:"wide",w:900,h:760,font:12,longDraft:false}]
  }
  function test_compact_open_preserves_chat_viewport(data) {
    var view = createTemporaryObject(scene, tests, {width:data.w,height:data.h,fontSize:data.font,longDraft:data.longDraft})
    tryCompare(view,"ready",true)
    var messages = list(view)
    verify(messages !== null)
    verify(messages.height >= Math.min(Style.space(100), view.chat.height * 0.25), "Open emoji must leave a usable chat viewport, not require fullscreen")
    verify(view.chat.emoji.height > Style.space(90), "Emoji remains usable with scrolling")
    var messageAt = messages.mapToItem(view.chat, 0, 0)
    verify(messageAt.y + messages.height <= view.chat.emoji.y + 1)
    var editor = view.chat.composer.parent.parent.parent
    var editorAt = editor.mapToItem(view.chat, 0, 0)
    verify(editorAt.y + editor.height <= view.chat.height + 1, "Composer must remain inside the same window")
    verify(view.chat.emoji.y + view.chat.emoji.height <= editorAt.y + 1, "Picker cannot overlap composer")
    var search = findChild(view, "emoji-search"), grid = findChild(view, "emoji-grid")
    var at = search.mapToItem(view.chat.emoji, 0, 0)
    verify(at.y >= 0 && at.y + search.height <= view.chat.emoji.height)
    verify(grid.height >= Style.space(24), "At least a clickable emoji row remains")
  }

  function test_header_scroll_keyboard_close_and_reactions() {
    var view = createTemporaryObject(scene, tests, {width:350,height:480,fontSize:18,themeRadius:8})
    tryCompare(view,"ready",true)
    var controls = findChild(view,"emoji-controls-scroll")
    verify(controls.contentWidth > controls.width)
    mouseWheel(controls,controls.width/2,controls.height/2,0,-120)
    verify(controls.contentX > 0, "Small picker headers can scroll instead of wrapping over the grid")
    var close = findChild(view,"emoji-close")
    close.forceActiveFocus()
    keyClick(Qt.Key_Return)
    compare(view.chat.emojiOpen,false)
    compare(view.chat.composer.text,"Synthetic unsent draft")
    view.chat.emojiOpen = true
    view.chat.emoji.openReactions(["👍", "❤️"])
    compare(view.chat.emoji.reacting,true)
    compare(view.chat.emoji.results.length,2)
    var grid = findChild(view,"emoji-grid")
    verify(grid.height >= Style.space(24))
    close.forceActiveFocus()
    keyClick(Qt.Key_Space)
    compare(view.chat.emojiOpen,false)
    compare(view.chat.reactionTargets.length,0)
  }

  function test_click_emoji_type_draft_close_and_resize() {
    var view = createTemporaryObject(scene, tests, {width:520,height:400,longDraft:true})
    tryCompare(view,"ready",true)
    var draft = view.chat.composer.text
    view.chat.composer.cursorPosition = draft.length
    var grid = findChild(view, "emoji-grid")
    verify(grid.currentItem !== null)
    mouseClick(grid.currentItem,grid.currentItem.width/2,grid.currentItem.height/2)
    compare(view.chat.composer.text,draft + "😀")
    compare(view.chat.emojiOpen,true)
    var messages = list(view)
    var row = messages.itemAtIndex(1)
    verify(row !== null)
    mouseClick(row.bubbleItem, row.bubbleItem.width - 5, row.bubbleItem.height - 5, Qt.RightButton)
    verify(view.chat.modalOpen, "Message menu remains reachable while emoji are open")
    keyClick(Qt.Key_Escape)
    compare(view.chat.modalOpen,false)
    var editor = view.chat.composer.parent.parent
    mouseClick(editor,editor.width/2,editor.height/2)
    verify(view.chat.composer.activeFocus,"Message editor remains reachable by pointer")
    keyClick(Qt.Key_Z)
    verify(view.chat.composer.text.indexOf("z") >= 0)
    var keep = view.chat.composer.text
    view.chat.composer.cursorPosition = keep.length
    for (var height of [814,400,480,400]) {
      view.height=height; wait(30); compare(view.chat.composer.text,keep); verify(list(view).height>=100)
      var caret = view.chat.composer.cursorRectangle
      verify(caret.y >= editor.contentY && caret.y + caret.height <= editor.contentY + editor.height + 1, "Focused caret remains visible after shrinking")
    }
    var close = findChild(view,"emoji-close")
    mouseClick(close,close.width/2,close.height/2)
    compare(view.chat.emojiOpen,false)
    verify(view.chat.composer.activeFocus)
    compare(view.chat.composer.text,keep)
    view.chat.openEmoji()
    wait(30)
    findChild(view,"emoji-search").forceActiveFocus()
    keyClick(Qt.Key_Escape)
    compare(view.chat.emojiOpen,false)
    compare(view.chat.composer.text,keep)
  }

  function test_narrow_controls_scroll_and_keyboard_close() {
    var view = createTemporaryObject(scene, tests, {width:350,height:480,fontSize:18,themeRadius:8})
    tryCompare(view,"ready",true)
    var controls = findChild(view,"emoji-controls-scroll")
    verify(controls.contentWidth > controls.width)
    mouseWheel(controls,controls.width/2,controls.height/2,0,-120)
    verify(controls.contentX > 0,"All tabs and tones remain reachable horizontally")
    controls.contentX = controls.contentWidth - controls.width
    findChild(view,"emoji-search").forceActiveFocus()
    keyClick(Qt.Key_5, Qt.AltModifier)
    compare(view.model.emojiState.tone,5)
    keyClick(Qt.Key_Tab)
    compare(view.chat.emoji.tab,1)
    keyClick(Qt.Key_Tab)
    compare(view.chat.emoji.tab,2)
    keyClick(Qt.Key_Tab)
    compare(view.chat.emoji.tab,0)
    var close = findChild(view,"emoji-close")
    close.forceActiveFocus()
    keyClick(Qt.Key_Escape)
    compare(view.chat.emojiOpen,false)
    view.chat.openEmoji()
    wait(30)
    close.forceActiveFocus()
    keyClick(Qt.Key_Return)
    compare(view.chat.emojiOpen,false)
    verify(view.chat.composer.activeFocus)
  }
}
