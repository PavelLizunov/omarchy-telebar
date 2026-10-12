//! Byte-preserving managed Omarchy menu block, with original JSONC admission.
use crate::storage::Storage;
use std::{io, path::Path};
const END: &str = "  // <<< io.github.pavellizunov.telebar <<<";
const PREFIX: &str = "  // >>> io.github.pavellizunov.telebar (managed by ";
const DESTINATION: &str = ".config/omarchy/extensions/omarchy-menu.jsonc";
pub fn strip_jsonc(raw: &str) -> String {
    // Exactly whole-line comments only; inline comments remain invalid.
    let mut clean = String::new();
    for line in raw.split_inclusive('\n') {
        if !line.trim_start().starts_with("//") {
            clean.push_str(line);
        }
    }
    let bytes = clean.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    for (i, b) in bytes.iter().enumerate() {
        if *b == b',' {
            let mut next = i + 1;
            while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                next += 1;
            }
            if bytes.get(next).is_some_and(|b| matches!(b, b'}' | b']')) {
                continue;
            }
        }
        out.push(*b);
    }
    String::from_utf8(out).expect("removing ASCII preserves UTF-8")
}
pub fn valid(raw: &str) -> bool {
    let clean = strip_jsonc(raw);
    crate::wire::bounded_json(
        if clean.trim().is_empty() {
            b"{}"
        } else {
            clean.as_bytes()
        },
        32,
        5000,
        4096,
    )
    .is_ok_and(|v| v.is_object())
}
pub fn without_block(raw: &str) -> String {
    let mut out = raw.to_owned();
    let mut offset = 0;
    while let Some(start) = out[offset..].find(PREFIX).map(|n| n + offset) {
        let tail = &out[start + PREFIX.len()..];
        let Some(close) = tail.find(") >>>") else {
            break;
        };
        if tail[..close].contains(['\n', ')']) {
            offset = start + PREFIX.len();
            continue;
        }
        let Some(end) = out[start + PREFIX.len() + close + 5..]
            .find(END)
            .map(|n| n + start + PREFIX.len() + close + 5 + END.len())
        else {
            break;
        };
        let begin = if start > 0 && out.as_bytes()[start - 1] == b'\n' {
            start - 1
        } else {
            start
        };
        let end = if out.as_bytes().get(end) == Some(&b'\n') {
            end + 1
        } else {
            end
        };
        out.replace_range(begin..end, "");
        offset = begin;
    }
    out
}
pub fn body(plugin: &Path) -> io::Result<String> {
    use std::os::unix::fs::MetadataExt;
    let bin = plugin.join("bin");
    let plain = bin
        .to_str()
        .filter(|s| {
            s.starts_with('/')
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._+/-".contains(&b))
                && !s.contains("/../")
                && !s.contains("/./")
        })
        .ok_or_else(|| io::Error::other("plugin path cannot be carried safely by menu command"))?;
    let store = Storage::open(plugin)?;
    let launcher = store.open_read("bin/omagram")?;
    if launcher.metadata()?.mode() & 0o022 != 0 {
        return Err(io::Error::other("launcher is writable by others"));
    }
    let raw = store
        .read("menu.jsonc", 65536)?
        .ok_or_else(|| io::Error::other("menu snippet missing"))?;
    Ok(String::from_utf8(raw)
        .map_err(|_| io::Error::other("invalid menu snippet"))?
        .replace("@PLUGIN_BIN@", plain)
        .trim_end()
        .to_owned())
}
pub fn apply(home: &Storage, plugin: &Path, command: &str) -> io::Result<Option<String>> {
    if command == "print" {
        return body(plugin).map(Some);
    }
    let previous = home.read(DESTINATION, 1024 * 1024)?;
    let previous = previous
        .map(String::from_utf8)
        .transpose()
        .map_err(|_| io::Error::other("invalid menu encoding"))?;
    if !matches!(command, "remove" | "install" | "add") {
        return Err(io::Error::other("invalid menu command"));
    }
    if command != "remove" && previous.as_ref().is_some_and(|s| !valid(s)) {
        return Err(io::Error::other("refusing malformed user JSONC"));
    }
    if previous.is_none() && command == "remove" {
        return Ok(None);
    }
    let mut raw = without_block(
        previous
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("{\n}\n"),
    );
    if command != "remove" {
        let start = raw
            .find('{')
            .ok_or_else(|| io::Error::other("menu is not an object"))?
            + 1;
        let block = format!(
            "\n{PREFIX}omagram-menu-install) >>>\n{}\n{END}\n",
            body(plugin)?
        );
        raw.insert_str(start, &block);
    }
    // Validate before replacing: malformed input never crosses the write seam.
    if !valid(&raw) {
        return Err(io::Error::other("refusing invalid resulting JSONC"));
    }
    // Atomic write rejects foreign/symlink destinations; no data migration.
    home.write(DESTINATION, raw.as_bytes(), 0o644)?;
    Ok(None)
}
