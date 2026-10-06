import QtQuick
import Quickshell
import Quickshell.Io

// A connection to omagramd, used by the window and by the plugin inside Omarchy's shell. A
// request gets exactly one callback with the service's answer; everything else the service
// says arrives as serviceEvent(name, message). While there is no connection it is retried,
// and the window also starts the service (the service itself allows only one instance).
Item {
  id: client

  property string binDir: ""
  // Where the service listens. Without XDG_RUNTIME_DIR there is no telling whose runtime
  // directory it is, so nothing is guessed and no connection is made.
  readonly property string runtimeDir: Quickshell.env("XDG_RUNTIME_DIR") || ""
  readonly property string socketPath: client.runtimeDir.charAt(0) === "/" ? client.runtimeDir + "/omagram/omagram.sock" : ""
  readonly property bool connected: !!client.sock && client.sock.connected

  // Omagram's own window: notification clicks that open a chat are handed to it.
  property bool window: false
  // Off inside Omarchy's shell, where the plugin's service entry keeps omagramd running.
  property bool autoStart: true
  // Optional target account id for multi-account requests
  property string accountId: ""
  property int accountGeneration: 0
  onAccountIdChanged: accountGeneration++

  property int nextId: 1
  property var callbacks: ({})
  property real lastStartAttempt: 0
  property var sock: null
  property var incoming: []
  property int incomingOffset: 0
  property real incomingBytes: 0

  function queueLine(line) {
    if (line === '{"event":"quit"}') {
      client.autoStart = false
      client.handleLine(line)
      return
    }
    if (client.incoming.length - client.incomingOffset >= 8192 || client.incomingBytes + line.length > 16 * 1024 * 1024) {
      console.warn("Telebar diagnostics: UI inbound queue exceeded; reconnecting")
      client.incoming = []
      client.incomingOffset = 0
      client.incomingBytes = 0
      if (client.sock) client.sock.connected = false
      return
    }
    client.incoming.push(line)
    client.incomingBytes += line.length
    if (!incomingPump.running) incomingPump.start()
  }

  Timer {
    id: incomingPump
    interval: 1
    onTriggered: {
      var started = Date.now(), count = 0
      while (client.incomingOffset < client.incoming.length && count++ < 32) {
        var line = client.incoming[client.incomingOffset]
        client.incoming[client.incomingOffset++] = null
        client.incomingBytes -= line.length
        client.handleLine(line)
        if (Date.now() - started >= 6) break
      }
      if (client.incomingOffset >= client.incoming.length) {
        client.incoming = []
        client.incomingOffset = 0
        client.incomingBytes = 0
      } else {
        if (client.incomingOffset >= 1024) {
          client.incoming = client.incoming.slice(client.incomingOffset)
          client.incomingOffset = 0
        }
        incomingPump.start()
      }
    }
  }
  property var uiContext: ({})
  property var metrics: ({ timer_max_ms: 0, timer_late_count: 0, frame_count: 0, frame_max_ms: 0,
    frame_slow_count: 0, pending_max: 0, rx_lines: 0, rx_bytes: 0, tx_requests: 0,
    response_max_ms: 0, response_slow_count: 0, handler_max_ms: 0, handler_slow_count: 0, parse_errors: 0 })
  property real lastHeartbeat: 0
  property real lastReport: 0

  function measure(name, value) {
    client.metrics[name] = Math.max(client.metrics[name] || 0, value)
  }

  signal hello(var result)
  signal serviceEvent(string name, var message)

  function request(cmd, args, callback) {
    // The socket itself, not `connected`: that binding may not have caught up yet inside the
    // socket's own connection handler, which is where hello is sent.
    if (!client.sock || !client.sock.connected) {
      if (callback) callback({ ok: false, error: "Telebar's service is not running" })
      return
    }
    var id = client.nextId++
    var control = ["hello", "account.list", "account.switch", "account.add", "app.quit", "diagnostics.ui", "diagnostics.status"].indexOf(cmd) >= 0
    if (callback && Object.keys(client.callbacks).length >= 512) {
      callback({ ok: false, error: "Too many pending requests; wait for the service" })
      return
    }
    if (callback) client.callbacks[id] = { callback: callback, account: client.accountId,
                                          generation: client.accountGeneration, control: control, started: Date.now(), command: cmd }
    var payload = { id: id, cmd: cmd, args: args || {} }
    if (!control && client.accountId && (!args || !args.account)) payload.account = client.accountId
    client.sock.write(JSON.stringify(payload) + "\n")
    client.sock.flush()
    if (cmd !== "diagnostics.ui") {
      client.metrics.tx_requests++
      client.measure("pending_max", Object.keys(client.callbacks).length)
    }
  }

  function handleLine(line) {
    var started = Date.now()
    client.metrics.rx_lines++
    client.metrics.rx_bytes += line.length
    var message = null
    try { message = JSON.parse(line) } catch (e) { client.metrics.parse_errors++; return }
    if (!message || typeof message !== "object") return
    if (typeof message.id === "number") {
      var pending = client.callbacks[message.id]
      if (pending) {
        delete client.callbacks[message.id]
        var elapsed = Math.max(0, Date.now() - pending.started)
        client.measure("response_max_ms", elapsed)
        if (elapsed > 250) client.metrics.response_slow_count++
        if (pending.control || (pending.account === client.accountId && pending.generation === client.accountGeneration))
          pending.callback(message)
      }
      var handled = Date.now() - started
      client.measure("handler_max_ms", handled)
      if (handled > 50) client.metrics.handler_slow_count++
      return
    }
    if (typeof message.event === "string") client.serviceEvent(message.event, message)
    var handled = Date.now() - started
    client.measure("handler_max_ms", handled)
    if (handled > 50) client.metrics.handler_slow_count++
  }

  function heartbeat() {
    var now = Date.now()
    if (client.lastHeartbeat) {
      var lag = Math.max(0, now - client.lastHeartbeat - 1000)
      client.measure("timer_max_ms", lag)
      if (lag > 250) client.metrics.timer_late_count++
      if (lag > 1000) console.warn("Telebar diagnostics: UI timer delay " + lag + " ms")
    }
    client.lastHeartbeat = now
    var oldest = 0
    for (var id in client.callbacks) {
      var pending = client.callbacks[id]
      var age = Math.max(0, now - pending.started)
      oldest = Math.max(oldest, age)
      if (age > 180000) {
        delete client.callbacks[id]
        if (pending.control || (pending.account === client.accountId && pending.generation === client.accountGeneration))
          pending.callback({ ok: false, error: "The service did not answer within three minutes" })
        // Command name only: never log request arguments or Telegram errors.
        console.warn("Telebar diagnostics: request timeout " + pending.command)
      }
    }
    if (!client.connected || now - client.lastReport < 5000) return
    client.lastReport = now
    var values = Object.assign({}, client.metrics, client.uiContext, {
      window: client.window, connected: client.connected, pending: Object.keys(client.callbacks).length,
      oldest_ms: oldest, generation: client.accountGeneration })
    client.request("diagnostics.ui", values)
    client.metrics = { timer_max_ms: 0, timer_late_count: 0, frame_count: 0, frame_max_ms: 0,
      frame_slow_count: 0, pending_max: 0, rx_lines: 0, rx_bytes: 0, tx_requests: 0,
      response_max_ms: 0, response_slow_count: 0, handler_max_ms: 0, handler_slow_count: 0, parse_errors: 0 }
  }

  Timer {
    interval: 1000
    repeat: true
    running: true
    onTriggered: client.heartbeat()
  }

  // Deliberately active during the diagnostic session: samples UI animation ticks,
  // not GPU timings or compositor-presented frames.
  FrameAnimation {
    running: client.window && client.connected && !!client.uiContext.visible && !!client.uiContext.focused
    onTriggered: {
      client.metrics.frame_count++
      client.measure("frame_max_ms", frameTime * 1000)
      if (frameTime > 0.05) client.metrics.frame_slow_count++
    }
  }

  function failPending() {
    var pending = client.callbacks
    client.callbacks = ({})
    for (var id in pending) {
      var entry = pending[id]
      if (entry.control || (entry.account === client.accountId && entry.generation === client.accountGeneration))
        entry.callback({ ok: false, error: "The connection to Telebar's service was lost" })
    }
  }

  function startService() {
    var now = Date.now()
    if (!client.autoStart || now - client.lastStartAttempt < 10000 || !client.binDir) return
    client.lastStartAttempt = now
    Quickshell.execDetached(["/usr/bin/python3", client.binDir + "omagram", "--service"])
  }

  // A fresh Socket for every attempt. One whose connection was refused does not try again
  // when its `connected` is set back to true, so a shell that started before the service
  // was listening would otherwise never connect.
  //
  // Connected only once it is `sock`: a local connection can complete synchronously, and its
  // handler would otherwise run before the assignment and be taken for a stale socket.
  function reconnect() {
    if (!client.socketPath) return
    incomingPump.stop()
    client.incoming = []
    client.incomingOffset = 0
    client.incomingBytes = 0
    var old = client.sock
    client.sock = socketComponent.createObject(client)
    if (old) old.destroy()
    if (client.sock) client.sock.connected = true
  }

  Component {
    id: socketComponent

    Socket {
      id: socket
      path: client.socketPath
      connected: false

      parser: SplitParser {
        onRead: function (line) { if (socket === client.sock) client.queueLine(line) }
      }

      // A socket being replaced can still report; only the current one speaks for the client.
      onConnectionStateChanged: {
        if (socket !== client.sock) return
        if (connected) {
          retry.interval = 500
          client.request("hello", { window: client.window }, function (answer) { if (answer.ok) client.hello(answer.result) })
        } else {
          client.failPending()
        }
      }
    }
  }

  Component.onCompleted: client.reconnect()

  // Retries for as long as there is no connection, backing off to five seconds.
  Timer {
    id: retry
    interval: 500
    repeat: true
    running: !client.connected
    onTriggered: {
      client.startService()
      interval = Math.min(5000, interval * 2)
      client.reconnect()
    }
  }
}
