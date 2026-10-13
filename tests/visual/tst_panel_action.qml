import QtQuick
import QtTest

TestCase {
  id: tests
  name: "ActualPanelWindowAction"
  width: 500
  height: 600
  visible: true
  when: windowShown
  Component { id: scene; PanelActionConsumer {} }
  function init() { failOnWarning(/.*/) }
  function test_label_and_mark_reach_real_parent_handler() {
    var view=createTemporaryObject(scene,tests)
    tryCompare(view,"ready",true)
    var mark=view.mark,label=view.label
    mouseMove(mark,mark.width/2,mark.height/2)
    tryCompare(label,"visible",true)
    mouseClick(label,label.width/2,label.height/2)
    compare(view.model.openedWindows,1)
    compare(view.panel.opened,false)
    view.panel.open()
    mouseClick(mark,mark.width/2,mark.height/2)
    compare(view.model.openedWindows,2)
    compare(view.panel.opened,false)
  }
  function test_open_keeps_pending_composition_owned() {
    var view=createTemporaryObject(scene,tests)
    tryCompare(view,"ready",true)
    view.quick.reply(101)
    view.quick.focusItem.text="Unsent actual-parent draft"
    view.quick.addAttachments([view.model.imagePath],true)
    var status=JSON.parse(view.panel.actionStatus())
    verify(status.draftPresent)
    compare(status.attachments,1)
    verify(view.panel.openInWindow(0))
    compare(view.model.openedWindows,1)
    compare(view.panel.opened,true)
    compare(view.quick.focusItem.text,"Unsent actual-parent draft")
    compare(view.quick.attachments.length,1)
    view.quick.attachments=[]
    view.quick.sending=true
    view.panel.openInWindow(0)
    compare(view.panel.opened,true)
    view.quick.sending=false
    view.panel.openInWindow(0)
    compare(view.panel.opened,false)
    compare(view.model.quickDraft,"Unsent actual-parent draft")
  }
  function test_unavailable_refuses_without_closing() {
    var view=createTemporaryObject(scene,tests)
    tryCompare(view,"ready",true)
    view.panel.omagram=null
    compare(view.panel.openInWindow(0),false)
    compare(view.panel.opened,true)
    compare(view.model.openedWindows,0)
  }
}
