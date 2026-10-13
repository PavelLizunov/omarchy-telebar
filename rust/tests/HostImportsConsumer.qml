import QtQuick
import qs.Commons as Commons
import "." as Fixtures

// Actual Main and the reviewed host token source with explicit inert I/O.
// Native Socket remains real. This does not launch production Quickshell.
Rectangle {
  id: fixture
  width: 1100
  height: 760
  color: "#101315"
  property string fixtureSocket: ""
  property int themeRadius: 8
  property bool settled: false
  property string failure: ""
  readonly property bool ready: settled && failure === ""
  function require(condition, reason) {
    if (!condition) { failure = reason; throw new Error(reason) }
  }
  Fixtures.MainConsumer {
    id: actual
    anchors.fill: parent
    fixtureSocket: fixture.fixtureSocket
    expectedAuth: "noLibrary"
    themeRadius: fixture.themeRadius
  }
  Timer {
    interval: 25
    repeat: true
    running: !fixture.ready
    property int ticks: 0
    onTriggered: {
      if (++ticks >= 180) { stop(); fixture.require(false, "finite host token readiness") }
      fixture.require(actual.failure === "", actual.failure)
      if (!actual.ready) return
      fixture.require(Commons.Color.foreground === Qt.rgba(202 / 255, 204 / 255, 204 / 255, 1), "host palette singleton identity")
      fixture.require(actual.main.foreground === Commons.Color.foreground, "Main uses the same admitted palette")
      fixture.require(actual.main.fontFamily === Commons.Style.font.menuFamily, "native typography identity")
      fixture.require(Commons.Style.font.body === 12 && Commons.Style.space(12) === 12, "host font/spacing defaults")
      fixture.require(Commons.Style.cornerRadius === fixture.themeRadius, "host radius identity remains reactive")
      var fill = Commons.Style.normalFillFor(Commons.Color.foreground, Commons.Color.accent, Commons.Color.urgent)
      fixture.require(isFinite(fill.r) && isFinite(fill.g) && isFinite(fill.b) && isFinite(fill.a), "unversioned host Color resolution")
      fixture.require(fill.a > 0 && fill.a < 1, "host normal-state alpha")
      fixture.require(actual.main.contentSurface.visible === false, "native surface remains unshown")
      fixture.settled = true
      stop()
    }
  }
}
