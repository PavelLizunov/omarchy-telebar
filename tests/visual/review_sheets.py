#!/usr/bin/env python3
"""Create labeled contact sheets from existing MCP captures; originals stay unchanged."""
import argparse
import json
from pathlib import Path
import subprocess

GROUPS = {
    "01-chat": ["chat", "compact", "long-chat", "topics", "topics-error", "info", "call", "sidebar-r0", "sidebar-r8"],
    "02-auth": ["setup", "login", "password", "login-code", "login-qr", "login-qr-pending", "login-proxy", "login-error"],
    "03-compose": ["new-chat", "new-chat-members", "new-chat-groupName", "new-chat-channel", "forward", "poll", "poll-quiz", "menu", "people"],
    "04-settings-account": ["settings-profile", "settings-privacy", "settings-blocked", "settings-security", "settings-notifications", "settings-downloads", "settings-stories", "settings-account", "settings-sessions"],
    "05-settings-edit": ["settings-folders", "settings-folder", "settings-connection", "settings-globals", "settings-shortcuts", "settings-password-flow", "settings-proxy-flow", "settings-edit", "settings-confirm"],
    "06-chat-states": ["chat-state-normal", "chat-state-draft", "chat-state-reply", "chat-state-edit", "chat-state-selection", "chat-state-prompt", "chat-state-attachments", "chat-state-recording", "chat-state-info", "chat-state-menu", "chat-state-emoji", "chat-state-stickers"],
    "07-quick": ["quick", "quick-reply", "quick-wide", "quick-narrow", "quick-light", "quick-attachments", "quick-recording", "quick-video-recording", "quick-long-draft", "quick-picker"],
    "08-media": ["media-photo", "media-video", "media-gif", "media-videoNote", "media-voice", "media-audio", "media-file", "media-sticker", "media-shell-photo", "media-shell-video", "photo", "story"],
    "09-auxiliary": ["file-picker-attachments", "file-picker-photo", "search", "accounts-ten", "bar-menu-r0", "bar-menu-r8", "emoji", "stickers", "icons"],
    "10-chat-extras": ["chat-state-date", "chat-state-location", "chat-state-scheduled", "chat-state-scheduled-loading", "chat-state-scheduled-populated", "chat-state-pinned", "chat-state-link-preview", "chat-state-send-menu", "chat-state-more-menu", "chat-state-dice-menu", "chat-state-contact-picker"],
    "11-info-story": ["info-tab-members", "info-tab-photos", "info-tab-files", "info-tab-links", "info-tab-voice", "info-tab-music", "info-tab-gifs", "info-tab-loading", "info-channel", "story-photo", "story-loading", "story-download", "story-live"],
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--paired-radius", action="store_true")
    args = parser.parse_args()
    results = json.loads((args.directory / "results.json").read_text())
    captured = {entry["case"]["name"] for entry in results if (args.directory / (entry["case"]["name"] + ".png")).is_file()}
    groups = {name + "-radius" + str(radius): [case + "-radius" + str(radius) for case in cases] for name, cases in GROUPS.items() for radius in [0, 8]} if args.paired_radius else GROUPS
    grouped = {name for group in groups.values() for name in group}
    assert captured <= grouped, captured - grouped
    for name, cases in groups.items():
        sources = [str(args.directory / (case + ".png")) for case in cases if case in captured]
        if not sources:
            continue
        destination = args.directory / ("review-sheet-" + name + ".png")
        if destination.exists():
            raise FileExistsError(destination)
        subprocess.run(["/usr/bin/montage", "-font", "/usr/share/fonts/liberation/LiberationSans-Regular.ttf", "-pointsize", "13", "-background", "#171b1e", "-fill", "#eeeeee", "-set", "label", "%t", *sources, "-thumbnail", "330x390", "-geometry", "330x415+8+8", "-tile", "3x", str(destination)], check=True, timeout=30)
        print(destination)


if __name__ == "__main__":
    main()
