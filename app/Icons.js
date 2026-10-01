// Original Omagram interface drawings, authored for this fork. 24-unit grid.
// Font-code aliases keep existing action tables working during the SVG migration.
var paths = {
  search: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.3 15.3 5 5"/>',
  close: '<path d="m6 6 12 12M18 6 6 18"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  minus: '<path d="M5 12h14"/>',
  check: '<path d="m4 12 5 5L20 6"/>',
  checkCircle: '<circle cx="12" cy="12" r="9"/><path d="m7 12 3 3 7-7"/>',
  back: '<path d="m10 5-7 7 7 7M4 12h17"/>',
  forward: '<path d="m14 5 7 7-7 7M20 12H3"/>',
  down: '<path d="m5 8 7 7 7-7"/>',
  up: '<path d="m5 16 7-7 7 7"/>',
  left: '<path d="m15 5-7 7 7 7"/>',
  right: '<path d="m9 5 7 7-7 7"/>',
  panelClose: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16m7-12-4 4 4 4"/>',
  panelOpen: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16m4-12 4 4-4 4"/>',
  edit: '<path d="m6 15 9-9a2.1 2.1 0 0 1 3 3l-9 9-4 1 1-4Z"/>',
  settings: '<path d="M9 5q3-4 6 0 5-1 5 4 4 3 0 6 0 5-5 4-3 4-6 0-5 1-5-4-4-3 0-6 0-5 5-4Z"/><circle cx="12" cy="12" r="3"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7v.1"/>',
  lock: '<rect x="5" y="10" width="14" height="10" rx="3"/><path d="M8 10V8a4 4 0 0 1 8 0v2"/>',
  pin: '<path d="M8 5h8M9 5v5q-3 2-3 5h12q0-3-3-5V5M12 15v5"/>',
  bell: '<path d="M5 17h14l-2-3V9a5 5 0 0 0-10 0v5l-2 3ZM10 20h4M12 2v2"/>',
  bellOff: '<path d="M4 3l17 18M7 7v7l-2 3h11M10 20h4M11 4a5 5 0 0 1 6 5v3"/>',
  bellSleep: '<path d="M5 17h12l-2-3V9a4 4 0 0 0-8 0v5l-2 3ZM9 20h4M18 3h4l-4 5h4"/>',
  calendar: '<rect x="4" y="6" width="16" height="14" rx="3"/><path d="M8 4v4M16 4v4M4 11h16M9 15h6"/>',
  play: '<path d="m8 4 12 8-12 8V4Z"/>',
  pause: '<path d="M8 4v16M16 4v16"/>',
  stop: '<rect x="5" y="5" width="14" height="14" rx="1"/>',
  download: '<path d="M12 4v11m-4-4 4 4 4-4M5 18q0 2 2 2h10q2 0 2-2"/>',
  upload: '<path d="M12 15V4m-4 4 4-4 4 4M5 18q0 2 2 2h10q2 0 2-2"/>',
  send: '<path d="M5 5q-1-1 1 0l14 6q2 1 0 2l-14 6q-2 1-1-1l3-6-3-7ZM8 12h11"/>',
  attach: '<path d="m8 15 8-8a2 2 0 0 1 3 3l-9 9a4 4 0 0 1-6-6L14 3a5 5 0 0 1 7 7L11 20"/>',
  microphone: '<rect x="9" y="4" width="6" height="10" rx="3"/><path d="M6 11v1a6 6 0 0 0 12 0v-1M12 18v3"/>',
  video: '<rect x="4" y="7" width="11" height="11" rx="3"/><path d="m15 11 5-3v9l-5-3"/>',
  record: '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="3"/>',
  music: '<path d="M10 17V5l10-2v12M10 8l10-2"/><ellipse cx="6" cy="18" rx="4" ry="3"/><ellipse cx="16" cy="16" rx="4" ry="3"/>',
  file: '<path d="M7 4h6l5 5v9q0 2-2 2H7q-2 0-2-2V6q0-2 2-2ZM13 4v5h5"/>',
  folder: '<path d="M4 7q0-2 2-2h4l2 3h6q2 0 2 2v8q0 2-2 2H6q-2 0-2-2V7Z"/>',
  bookmark: '<path d="M8 4h8q2 0 2 2v14l-6-3-6 3V6q0-2 2-2Z"/>',
  heart: '<path d="M12 21 3.5 12.5C-2 6 5 0 12 7c7-7 14-1 8.5 5.5L12 21Z"/>',
  at: '<circle cx="11" cy="12" r="3.5"/><path d="M14.5 8.5v6a2.5 2.5 0 0 0 5 0V12a8.5 8.5 0 1 0-4.2 7.4"/>',
  smile: '<circle cx="12" cy="12" r="9"/><path d="M8 9v.1M16 9v.1M7 14q5 7 10 0"/>',
  sticker: '<path d="M7 4h10q3 0 3 3v7l-6 6H7q-3 0-3-3V7q0-3 3-3ZM14 20v-3q0-3 3-3h3"/>',
  people: '<circle cx="9" cy="8" r="4"/><path d="M2 21v-3a7 7 0 0 1 14 0v3M17 4a4 4 0 0 1 0 8M20 16q2 1 2 5"/>',
  channel: '<path d="M7 9h2l10-4v14l-10-4H7q-3 0-3-3t3-3ZM8 15l2 5"/>',
  chat: '<path d="M8 5h8q4 0 4 4v4q0 4-4 4h-5l-5 3v-4q-2-1-2-3V9q0-4 4-4Z"/>',
  reply: '<path d="m9 5-6 6 6 6M3 11h10q7 0 7 8"/>',
  window: '<rect x="4" y="4" width="16" height="16" rx="3"/><path d="M4 9h16M10 15h6m-3-3 3 3-3 3"/>',
  image: '<rect x="4" y="4" width="16" height="16" rx="3"/><path d="m5 17 5-6 4 4 3-3 3 4"/>',
  volumeOff: '<path d="M3 9h4l6-6v18l-6-6H3V9ZM17 9l5 6m0-6-5 6"/>',
  expand: '<path d="M3 9V3h6M15 3h6v6M21 15v6h-6M9 21H3v-6"/>',
  more: '<circle cx="4" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="20" cy="12" r="1"/>',
  trash: '<path d="M5 7h14M9 7V4h6v3M7 7l1 11q0 2 2 2h4q2 0 2-2l1-11M12 11v5"/>',
  refresh: '<path d="M20 8a9 9 0 1 0 1 7M20 3v6h-6"/>',
  spinner: '<path d="M12 3a9 9 0 1 1-9 9"/>',
  shield: '<path d="m12 2 9 4v7q-2 7-9 10-7-3-9-10V6l9-4ZM8 12l3 3 5-6"/>'
}

var legacy = {
  0xF0349: "search", 0xF0156: "close", 0xF0493: "settings", 0xF0CB6: "edit",
  0xF033E: "lock", 0xF0403: "pin", 0xF009B: "bellOff", 0xF009C: "bell",
  0xF00A0: "bellSleep", 0xF004D: "back", 0xF00F0: "calendar", 0xF02FD: "info",
  0xF0567: "video", 0xF0387: "music", 0xF0224: "file", 0xF0341: "lock",
  0xF0182: "reply", 0xF0B58: "people", 0xF0B23: "channel", 0xF0065: "at",
  0xF05E1: "checkCircle", 0xF00C0: "bookmark", 0xF0150: "refresh", 0xF03F7: "microphone",
  0xF040A: "play", 0xF03E4: "pause", 0xF01DA: "download", 0xF03CC: "window",
  0xF048A: "send", 0xF044A: "record", 0xF02D5: "heart", 0xF0140: "down",
  0xF04C6: "stop", 0xF04E0: "check", 0xF06D5: "microphone", 0xF00BD: "video",
  0xF0A7C: "sticker", 0xF04A3: "send", 0xF013D: "down", 0xF0459: "attach",
  0xF01D8: "more", 0xF0ABF: "image", 0xF04D2: "heart", 0xF02B1: "more",
  0xF0A93: "bell", 0xF1164: "calendar", 0xF01F2: "smile", 0xF03E2: "attach",
  0xF0419: "plus", 0xF0785: "sticker", 0xF036C: "microphone", 0xF06D0: "heart", 0xF0D78: "sticker"
}

function nameFor(value) {
  if (paths[value]) return value
  var n = typeof value === "number" ? value : (String(value || "").codePointAt(0) || 0)
  return legacy[n] || "more"
}

function source(name, color) {
  var c = String(color)
  if (!/^#[0-9a-fA-F]{6,8}$/.test(c) && !/^[a-zA-Z]+$/.test(c)) c = "#808080"
  return "data:image/svg+xml," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="'
    + c + '" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">' + (paths[name] || paths.more) + '</svg>')
}
