import QtQuick
import QtTest
import qs.Commons as Host

TestCase {
  id: tests
  name: "ActualTelebarBarIcon"
  when: windowShown
  width: 420
  height: 160
  visible: true
  Component { id: scene; BarIconConsumer {} }
  function init() { failOnWarning(/.*/) }
  function cleanup() { Host.Style.cornerRadius = 0; Host.Style.barOverrides = ({}) }
  function test_host_geometry_status_and_theme_data() {
    return [{tag:"square-ready",radius:0,vertical:false,ready:true,unread:0},
      {tag:"round-unread",radius:8,vertical:false,ready:true,unread:7},
      {tag:"unavailable",radius:0,vertical:false,ready:false,unread:7},
      {tag:"vertical",radius:8,vertical:true,ready:true,unread:7}]
  }
  function test_host_geometry_status_and_theme(data) {
    var view = createTemporaryObject(scene, tests, {themeRadius:data.radius,verticalBar:data.vertical,
      serviceReady:data.ready,unreadCount:data.unread})
    tryCompare(view,"ready",true)
    var button=findChild(view,"telebar-icon-button")
    var mark=findChild(view,"telebar-ring-mark")
    var dot=findChild(view,"telebar-unread-dot")
    compare(mark.size,Host.Style.bar.iconCanvas)
    compare(dot.visible,data.ready && data.unread>0)
    compare(button.width,data.vertical ? Host.Style.bar.sizeVertical : Host.Style.bar.iconSlot)
    compare(button.height,data.vertical ? Host.Style.bar.iconSlot : Host.Style.bar.sizeHorizontal)
    compare(view.fixtureBar.clickTarget,button)
    view.ink="#315271"
    compare(mark.color,view.ink)
    Host.Style.barOverrides={"icon-canvas":20}
    compare(mark.size,20)
  }
  function test_hover_and_press_preserve_icon_geometry_and_ink() {
    var view=createTemporaryObject(scene,tests)
    tryCompare(view,"ready",true)
    var button=findChild(view,"telebar-icon-button")
    var mark=findChild(view,"telebar-ring-mark")
    var color=mark.color
    mouseMove(button,button.width/2,button.height/2)
    compare(button.tooltipHovered,true)
    compare(mark.size,16)
    compare(mark.color,color)
    mousePress(button,button.width/2,button.height/2,Qt.LeftButton)
    compare(view.widget.opened,false)
    compare(mark.size,16)
    compare(mark.color,color)
    mouseRelease(button,button.width/2,button.height/2,Qt.LeftButton)
    compare(view.widget.opened,true)
    compare(mark.color,color)
  }
  function test_actual_pointer_routes() {
    var view=createTemporaryObject(scene,tests)
    tryCompare(view,"ready",true)
    var button=findChild(view,"telebar-icon-button")
    view.fixtureService.stopped=true
    mouseClick(button,button.width/2,button.height/2,Qt.LeftButton)
    compare(view.fixtureService.stopped,false)
    compare(view.widget.opened,true)
    mouseClick(button,button.width/2,button.height/2,Qt.LeftButton)
    compare(view.widget.opened,false)
    mouseClick(button,button.width/2,button.height/2,Qt.RightButton)
    compare(view.widget.menuOpen,true)
    view.widget.runMenu("open")
    compare(view.fixtureService.openedWindows,1)
    compare(view.widget.menuOpen,false)
  }
}
