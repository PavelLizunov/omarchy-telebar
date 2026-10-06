import QtMultimedia

// The device list notifies when the default output changes. Keep the existing player
// and position; only rebind its output, instead of requiring an application restart.
AudioOutput {
  id: output
  property var devices: MediaDevices {}
  device: devices.defaultAudioOutput
}
