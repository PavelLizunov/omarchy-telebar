import QtQuick
pragma Singleton
QtObject {
  function env(k) {
    if (k === "HOME") return "/inert-home";
    if (k === "USER") return "fixture";
    return "";
  }
  function execDetached(cmd) {}
}
