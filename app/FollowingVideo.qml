import QtQuick
import QtMultimedia

// Video's Qt convenience type does not expose its AudioOutput device. Use the same
// native player/output composition so videos follow default-output changes too.
Item {
  id: video
  property alias source: player.source
  property alias autoPlay: player.autoPlay
  property alias playbackRate: player.playbackRate
  property alias position: player.position
  property alias playbackState: player.playbackState
  property alias loops: player.loops
  property alias muted: audio.muted
  property alias fillMode: picture.fillMode
  signal stopped()
  function play() { player.play() }
  function pause() { player.pause() }
  function stop() { player.stop() }
  VideoOutput { id: picture; anchors.fill: parent }
  FollowingAudioOutput { id: audio }
  MediaPlayer {
    id: player
    audioOutput: audio
    videoOutput: picture
    onPlaybackStateChanged: if (playbackState === MediaPlayer.StoppedState) video.stopped()
  }
}
