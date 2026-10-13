import QtQuick
import qs.Commons
import "../../app" as App

// Exact Main/ChatView composer and native Socket. Only the explicitly compiled
// local send-ABI oracle may be used. No TDLib/Telegram/account/device work.
Rectangle {
  id: fixture
  width: 1100
  height: 760
  color: "#101315"
  property string fixtureSocket: ""
  property string finalState: "accepted"
  property int themeRadius: 0
  property bool settled: false
  property string failure: ""
  readonly property bool ready: settled && failure === ""
  property int phase: 0
  property var surface: null
  property var chatView: null
  property var composerInput: null
  property int checks: 0
  function require(condition,reason) {if(!condition){failure=reason;throw new Error(reason)}checks++}
  function findComposer(item) {
    if(item.objectName === "composer-text")return item
    for(var i=0;i<item.children.length;i++){var found=findComposer(item.children[i]);if(found)return found}
    return null
  }
  App.Main {
    id: app
    autoStart: false
    surfaceVisible: false
    socketPath: fixture.fixtureSocket
  }
  Component.onCompleted: {
    require(fixtureSocket.length > 0 && fixtureSocket.length <= 100,"explicit short fixture socket")
    require(finalState === "accepted" || finalState === "rejected","explicit send outcome")
    Style.cornerRadius = themeRadius
    surface = app.contentSurface
    require(surface !== null,"actual Main surface")
    surface.contentItem.parent = fixture
    surface.contentItem.width = fixture.width
    surface.contentItem.height = fixture.height
    settle.start()
  }
  Timer {
    id: settle
    interval: 25
    repeat: true
    property int ticks: 0
    onTriggered: {
      if(++ticks >= 200){stop();fixture.require(false,"finite actual composer acceptance")}
      if(app.auth.state !== "ready" || app.chats.length === 0)return
      if(fixture.phase === 0){app.openChatById(101,false);fixture.phase=1;return}
      if(fixture.phase === 1){
        if(app.messagesFor(101).length !== 3)return
        var composer=findComposer(surface.contentItem)
        if(!composer)return
        var owner=composer.parent
        while(owner && typeof owner.send !== "function")owner=owner.parent
        fixture.require(owner !== null && typeof owner.setComposerText === "function","owning actual ChatView composer")
        fixture.chatView=owner
        fixture.composerInput=composer
        owner.setComposerText("fixture-reject")
        owner.send()
        fixture.phase=2
        return
      }
      if(fixture.phase === 2){
        if(chatView.sendPending)return
        fixture.require(composerInput.text === "fixture-reject","rejected composition must survive")
        fixture.require(chatView.notice === "Could not send: synthetic send refused","native failure feedback")
        fixture.require(app.messagesFor(101).length === 3,"rejection must not invent a message")
        if(fixture.finalState === "rejected"){fixture.settled=true;stop();return}
        chatView.setComposerText("Synthetic send accepted through the actual Rust socket.")
        chatView.send()
        fixture.phase=3
        return
      }
      if(fixture.phase === 3){
        if(chatView.sendPending || app.messagesFor(101).length !== 4)return
        fixture.require(composerInput.text === "","current acknowledged composer clears")
        var messages=app.messagesFor(101)
        var found=messages.filter(function(m){return m.content.text === "Synthetic send accepted through the actual Rust socket."})
        if(found.length === 1 && found[0].sending)return
        fixture.require(found.length === 1 && found[0].outgoing && !found[0].sending,"settled update replaces pending message once")
        fixture.require(app.recordedSend === null,"text send cannot reserve recording")
        fixture.require(surface.visible === false && !app.windowFocused,"consumer stays unshown without read acknowledgements")
        fixture.settled=true
        stop()
      }
    }
  }
}
