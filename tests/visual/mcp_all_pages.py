#!/usr/bin/env python3
"""Bounded actual MCP pass. Outputs provenance; inspection is a separate human step."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests/visual"
SERVER = Path("/home/slovn/Work/qml-preview-mcp/mcp-qml-preview.py")
IMPORTS = Path("/tmp/opencode/qml-mock")


def cases():
    out = []
    def add(name, fixture="Preview.qml", width=552, height=650, **props):
        out.append({"name": name, "fixture": fixture, "width": width, "height": height, "props": props})
    for scene in ["chat", "compact", "long-chat", "setup", "login", "password", "topics", "topics-error", "new-chat", "forward", "poll", "info", "quick", "quick-reply", "photo", "story", "emoji", "stickers", "menu", "people", "call", "icons"]:
        add(scene, width=1100 if scene in ["chat", "long-chat", "icons"] else 552, scene=scene)
    for variant in ["code", "qr", "qr-pending", "proxy", "error"]:
        add("login-" + variant, scene="login", variant=variant, fontSize=15, themeRadius=8)
    for mode in ["members", "groupName", "channel"]:
        add("new-chat-" + mode, width=444, scene="new-chat", variant=mode, fontSize=15)
    add("poll-quiz", width=444, scene="poll", variant="quiz", fontSize=15)
    for state in ["profile", "privacy", "blocked", "security", "notifications", "downloads", "folders", "folder", "sessions", "account", "globals", "stories", "shortcuts", "connection", "password-flow", "proxy-flow", "edit", "confirm"]:
        add("settings-" + state, "SettingsConsumer.qml", width=552 if state not in ["edit", "confirm", "folder", "password-flow", "proxy-flow"] else 444, state=state, storiesShown=False, themeRadius=8)
    for state in ["normal", "draft", "reply", "edit", "selection", "prompt", "attachments", "recording", "info", "menu", "emoji", "stickers", "date", "location", "scheduled", "scheduled-loading", "scheduled-populated", "pinned", "link-preview", "send-menu", "more-menu", "dice-menu", "contact-picker"]:
        add("chat-state-" + state, "Narrow.qml", height=600, state=state, themeRadius=0)
    for name, props, width in [
        ("quick-wide", {"compactMode": False, "themeRadius": 0}, 900),
        ("quick-narrow", {"themeRadius": 8}, 350),
        ("quick-light", {"themeRadius": 8, "lightTheme": True}, 350),
        ("quick-attachments", {"state": "attachments", "themeRadius": 0}, 350),
        ("quick-recording", {"state": "recording", "themeRadius": 8}, 350),
        ("quick-video-recording", {"state": "video-recording", "themeRadius": 8}, 350),
        ("quick-long-draft", {"state": "long-draft", "themeRadius": 0}, 350),
        ("quick-picker", {"state": "picker", "themeRadius": 0}, 350)]:
        add(name, "QuickConsumer.qml", width=width, height=600, **props)
    for state in ["photo", "video", "gif", "videoNote", "voice", "audio", "file", "sticker", "shell-photo", "shell-video"]:
        add("media-" + state, "MediaConsumer.qml", width=552, height=600, state=state, themeRadius=8)
    add("file-picker-attachments", "FilePickerConsumer.qml", width=900, dialogKind="attachments", themeRadius=0)
    add("file-picker-photo", "FilePickerConsumer.qml", width=444, dialogKind="photo", themeRadius=8)
    add("search", "SearchConsumer.qml", width=350, themeRadius=0)
    add("accounts-ten", "SearchConsumer.qml", width=350, height=500, accountMenu=True, themeRadius=8)
    for radius in [0, 8]:
        add("bar-menu-r" + str(radius), "BarMenuConsumer.qml", width=300, height=250, themeRadius=radius)
        add("sidebar-r" + str(radius), "Sidebar.qml", width=900 if radius == 0 else 552, themeRadius=radius, fontSize=15, sidebarWidth=300 if radius == 0 else 72, focusHandle=True)
    add("bar-host", "Bar.qml", width=480, height=240)
    for state in ["members", "photos", "files", "links", "voice", "music", "gifs", "loading"]:
        add("info-tab-" + state, "InfoConsumer.qml", width=444, state=state)
    add("info-channel", "InfoConsumer.qml", width=444, state="members", chatKind="channel")
    for state in ["photo", "loading", "download", "live"]:
        add("story-" + state, "StoryConsumer.qml", width=444, state=state)
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--only", help="Comma-separated case names")
    parser.add_argument("--paired-radius", action="store_true", help="Render every state at radii 0 and 8")
    args = parser.parse_args()
    if not args.output.parent.is_dir() or args.output.exists():
        parser.error("Output must be new with an existing parent")
    args.output.mkdir(mode=0o700)
    # Retain identities beyond the per-render MCP dependency budget, including data.
    identity_paths = [p for directory in [ROOT / "app", ROOT / "shell", FIXTURES, IMPORTS] for p in directory.rglob("*") if p.is_file()]
    identity_paths += [Path(__file__).resolve()]
    identity = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(identity_paths))}
    (args.output / "source-snapshot.json").write_text(json.dumps(identity, indent=2))
    run = cases()
    if args.only:
        names = set(args.only.split(","))
        unknown = names - {case["name"] for case in run}
        if unknown:
            parser.error("Unknown cases: " + ", ".join(sorted(unknown)))
        run = [case for case in run if case["name"] in names]
    if args.paired_radius:
        run = [variant for case in run for variant in ([case] if case["name"] == "bar-host" else [{**case, "name": case["name"] + "-radius" + str(radius), "props": {**case["props"], "themeRadius": radius}} for radius in [0, 8]])]
    # Explicit hashes cover app/shell sources, fixture model, assets and existing inert adapters.
    dependencies = sorted(p for directory in [ROOT / "app", ROOT / "shell"] for p in directory.glob("*") if p.suffix in [".qml", ".js"])
    dependencies += [FIXTURES / name for name in ["FixtureApp.qml", "Preview.qml", "Readiness.js", "sample-photo.svg"]]
    dependencies += sorted(p for p in IMPORTS.rglob("*") if p.is_file())
    assert len(dependencies) <= 127, "MCP dependency limit; current root is hashed separately"
    process = subprocess.Popen([sys.executable, str(SERVER)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    rid = 0
    def request(method, params):
        nonlocal rid
        rid += 1
        process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "method": method, "params": params}) + "\n")
        process.stdin.flush()
        while True:
            line = process.stdout.readline()
            if not line:
                raise RuntimeError("MCP server exited")
            result = json.loads(line)
            if result.get("id") == rid:
                return result
    results = []
    try:
        request("initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "omagram-pages-review", "version": "1"}})
        process.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
        process.stdin.flush()
        catalog = request("tools/list", {})
        (args.output / "tools.json").write_text(json.dumps(catalog, ensure_ascii=False, indent=2))
        health = request("tools/call", {"name": "healthcheck", "arguments": {}})
        (args.output / "healthcheck.json").write_text(json.dumps(health, ensure_ascii=False, indent=2))
        for case in run:
            path = FIXTURES / case["fixture"]
            options = {"qmlPath": str(path), "outputPath": str(args.output / (case["name"] + ".png")), "width": case["width"], "height": case["height"], "dpr": 2 if case["name"].startswith("quick-light") else 1, "locale": "ru_RU", "imageMode": "path", "warningsPolicy": "error", "initialProperties": case["props"], "importPaths": [str(IMPORTS)], "dependencyPaths": [str(p) for p in dependencies], "timeoutMs": 5000}
            options["snapshot"] = {"maxDepth": 8, "maxItems": 128}
            if case["name"] != "bar-host":
                options["readyProperty"] = "ready"
            if case["fixture"] in ["Preview.qml", "SettingsConsumer.qml", "MediaConsumer.qml", "Narrow.qml", "QuickConsumer.qml", "FilePickerConsumer.qml", "InfoConsumer.qml", "StoryConsumer.qml"]:
                options["measureObjects"] = ["review-root", "review-content"]
                options["geometryChecks"] = [{"kind": "inside", "a": "review-content", "container": "review-root"}, {"kind": "size", "a": "review-root", "field": "width", "value": case["width"]}]
            if case["fixture"] == "QuickConsumer.qml":
                tools = ["quick-tool-" + action for action in ["attach", "stickers", "voice", "video"]]
                options["measureObjects"] += ["quick-composer-box", "quick-composer", *tools]
                options["geometryChecks"] += [{"kind": "inside", "a": tool, "container": "quick-composer-box"} for tool in tools]
                options["geometryChecks"] += [{"kind": "noOverlap", "a": tool, "b": "quick-composer"} for tool in tools]
                options["geometryChecks"] += [{"kind": "align", "a": tools[0], "b": tool, "field": "centerY"} for tool in tools[1:]]
                options["geometryChecks"] += [{"kind": "equal", "a": tools[0], "b": tool, "field": "height"} for tool in tools[1:]]
                options["geometryChecks"] += [{"kind": "gap", "a": tools[i], "b": tools[i+1], "axis": "x", "value": 0} for i in range(3)]
                options["geometryChecks"] += [{"kind": "centered", "a": "review-content", "container": "review-root", "axis": "x"}]
            if case["fixture"] == "Narrow.qml" and case["props"].get("state") not in ["recording", "scheduled", "scheduled-loading", "scheduled-populated"]:
                options["measureObjects"] += ["composer-box", "composer-text", "composer-actions", "chat-info-panel"]
                options["geometryChecks"] += [{"kind": "inside", "a": name, "container": "review-content"} for name in ["composer-box", "composer-actions"]]
                options["geometryChecks"] += [{"kind": "inside", "a": "composer-text", "container": "composer-box"}, {"kind": "noOverlap", "a": "composer-text", "b": "composer-actions"}]
            if case["fixture"] == "FilePickerConsumer.qml" or case["name"].startswith("quick-picker"):
                objects = ["file-picker-card", "file-picker-path", "file-picker-list", "file-picker-up", "file-picker-cancel", "file-picker-accept"]
                options.setdefault("measureObjects", []).extend(objects)
                options.setdefault("geometryChecks", []).extend({"kind": "inside", "a": obj, "container": "file-picker-card"} for obj in objects[1:])
                # The owning RowLayout intentionally right-aligns buttons with a spacer.
                # Equal sizes/containment apply; mirror symmetry around the card does not.
                options["geometryChecks"] += [{"kind": "equal", "a": "file-picker-cancel", "b": "file-picker-accept", "field": "width"}, {"kind": "noOverlap", "a": "file-picker-cancel", "b": "file-picker-accept"}]
            reply = request("tools/call", {"name": "render_qml", "arguments": options})
            if reply.get("error"):
                raise RuntimeError(reply["error"])
            result = reply.get("result", {})
            metadata = result.get("structuredContent")
            if metadata is None:
                for block in result.get("content", []):
                    if block.get("type") == "text":
                        try:
                            metadata = json.loads(block["text"])
                            break
                        except ValueError:
                            pass
            entry = {"case": case, "arguments": options, "isError": bool(result.get("isError") or reply.get("error")), "metadata": metadata, "response": reply, "inspected": False}
            results.append(entry)
            (args.output / (case["name"] + ".json")).write_text(json.dumps(entry, ensure_ascii=False, indent=2))
            (args.output / "results.json").write_text(json.dumps(results, ensure_ascii=False, indent=2))
            print(case["name"], "FAIL" if entry["isError"] else "PASS", flush=True)
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.terminate()
            process.wait(timeout=5)
    changed = [path for path, digest in identity.items() if not Path(path).is_file() or hashlib.sha256(Path(path).read_bytes()).hexdigest() != digest]
    current_paths = {str(p) for directory in [ROOT / "app", ROOT / "shell", FIXTURES, IMPORTS] for p in directory.rglob("*") if p.is_file()}
    changed += sorted(current_paths - set(identity))
    assert not changed, "Dependencies changed during the batch: " + repr(changed)
    print("TOTAL", len(results), "PASS", sum(not r["isError"] for r in results), "FAIL/BLOCKED", sum(r["isError"] for r in results))


if __name__ == "__main__":
    main()
