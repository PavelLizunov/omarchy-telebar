//! Finite Wayland clipboard helpers with explicit session environment. Selection
//! persistence is owned by a managed foreground wl-copy child, never its fork.
use crate::{
    commands::{Action, Result},
    process,
    storage::Storage,
    uploads,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
const MAX: usize = 10 * 1024 * 1024;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";
#[derive(Clone, Debug)]
pub struct Context {
    pub runtime: PathBuf,
    pub databases: Vec<PathBuf>,
    pub environment: BTreeMap<String, String>,
}
pub fn png(data: &[u8], cancel: &AtomicBool) -> Result<Vec<u8>> {
    if data.is_empty() || data.len() > MAX {
        return Err("the clipboard image is empty or exceeds 10 MiB".into());
    }
    if data.starts_with(PNG) {
        return Ok(data.into());
    }
    let decoder = if data.starts_with(b"\xff\xd8\xff") {
        "mjpeg"
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        "webp"
    } else {
        return Err("that file is not a supported image".into());
    };
    let output = process::run_cancel(
        "/usr/bin/prlimit",
        &[
            "--as=1073741824",
            "--cpu=10",
            "--",
            "/usr/bin/ffmpeg",
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-max_alloc",
            "67108864",
            "-threads",
            "1",
            "-protocol_whitelist",
            "pipe",
            "-f",
            "image2pipe",
            "-c:v",
            decoder,
            "-max_pixels",
            "16000000",
            "-i",
            "pipe:0",
            "-frames:v",
            "1",
            "-an",
            "-sn",
            "-dn",
            "-threads",
            "1",
            "-c:v",
            "png",
            "-f",
            "image2pipe",
            "pipe:1",
        ],
        &BTreeMap::new(),
        data,
        Duration::from_secs(10),
        MAX,
        cancel,
    )
    .map_err(|_| "the image could not be converted to PNG within clipboard limits")?;
    if !output.ok() || !output.stdout.starts_with(PNG) {
        return Err("the image could not be converted to PNG within clipboard limits".into());
    }
    Ok(output.stdout)
}
fn percent(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut decoded = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let a = (bytes[i + 1] as char).to_digit(16);
            let b = (bytes[i + 2] as char).to_digit(16);
            if let (Some(a), Some(b)) = (a, b) {
                decoded.push((16 * a + b) as u8);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}
pub fn paths(data: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(data);
    let mut paths = Vec::new();
    for line in text.split([
        '\n', '\r', '\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{2029}',
    ]) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((scheme, address)) = line.split_once(':') else {
            continue;
        };
        if !scheme.eq_ignore_ascii_case("file") {
            continue;
        }
        let path = if let Some(address) = address.strip_prefix("//") {
            let (authority, path) = address.split_once('/').unwrap_or((address, ""));
            if !matches!(authority, "" | "localhost") {
                continue;
            }
            format!("/{path}")
        } else {
            address.to_owned()
        };
        let path = percent(path.split(['?', '#']).next().unwrap_or(""));
        if !path.starts_with('/') || path.chars().any(|c| c < ' ') || paths.contains(&path) {
            continue;
        }
        paths.push(path);
        if paths.len() >= 10 {
            break;
        }
    }
    paths
}
fn offered(context: &Context, cancel: &AtomicBool) -> Vec<String> {
    process::run_cancel(
        "/usr/bin/wl-paste",
        &["--list-types"],
        &context.environment,
        &[],
        Duration::from_secs(3),
        16 * 1024,
        cancel,
    )
    .ok()
    .filter(process::Output::ok)
    .map(|o| {
        String::from_utf8_lossy(&o.stdout)
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    })
    .unwrap_or_default()
}
fn data(mime: &str, context: &Context, cancel: &AtomicBool) -> Option<Vec<u8>> {
    process::run_cancel(
        "/usr/bin/wl-paste",
        &["--no-newline", "--type", mime],
        &context.environment,
        &[],
        Duration::from_secs(5),
        MAX + 1,
        cancel,
    )
    .ok()
    .filter(|o| o.ok() && !o.stdout.is_empty() && o.stdout.len() <= MAX)
    .map(|o| o.stdout)
}
pub fn read(context: Context, files: bool, cancel: &AtomicBool) -> Result<Action> {
    let types = offered(&context, cancel);
    if files && types.iter().any(|s| s == "text/uri-list") {
        let mut admitted = Vec::new();
        let mut skipped = 0;
        if let Some(data) = data("text/uri-list", &context, cancel) {
            for path in paths(&data) {
                match uploads::admit(&json!(path), &context.databases) {
                    Ok(upload) => {
                        let path = upload.path.to_string_lossy().into_owned();
                        if !admitted.contains(&path) {
                            admitted.push(path);
                        }
                    }
                    Err(_) => skipped += 1,
                }
            }
        }
        if !admitted.is_empty() || skipped > 0 {
            return Ok(Action::Reply(json!({"paths":admitted,"skipped":skipped})));
        }
    }
    for (mime, suffix) in [
        ("image/png", "png"),
        ("image/jpeg", "jpg"),
        ("image/webp", "webp"),
    ] {
        if !types.iter().any(|s| s == mime) {
            continue;
        }
        let Some(data) = data(mime, &context, cancel) else {
            break;
        };
        let home =
            Storage::open(&context.runtime).map_err(|_| "the clipboard could not be read")?;
        let mut random = [0u8; 16];
        use std::io::Read;
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut random))
            .map_err(|_| "the clipboard could not be read")?;
        let token = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let relative = format!("omagram/rec/paste-{token}.{suffix}");
        home.write(&relative, &data, 0o600)
            .map_err(|_| "the clipboard could not be read")?;
        let path = context.runtime.join(relative);
        return Ok(Action::Reply(if files {
            json!({"paths":[path],"skipped":0})
        } else {
            json!({"path":path})
        }));
    }
    Ok(Action::Reply(if files {
        json!({"paths":[],"skipped":0})
    } else {
        json!({"path":""})
    }))
}
pub fn prepare_copy(
    path: &str,
    roots: &[String],
    context: Context,
    cancel: &AtomicBool,
) -> Result<Action> {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("the connection was closed".into());
    }
    let root = roots
        .iter()
        .filter(|r| path.starts_with(&format!("{}/", r.trim_end_matches('/'))))
        .max_by_key(|r| r.len())
        .ok_or("that file is not one of Telebar's")?;
    let source = Storage::open(std::path::Path::new(root))
        .and_then(|s| s.read(&path[root.trim_end_matches('/').len() + 1..], MAX))
        .map_err(|_| "the image could not be copied (maximum 10 MiB)")?
        .filter(|b| !b.is_empty())
        .ok_or("the image is empty or missing")?;
    let data = png(&source, cancel)?;
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("the connection was closed".into());
    }
    Ok(Action::ClipboardOffer(Offer {
        data,
        environment: context.environment,
    }))
}
pub struct Offer {
    pub data: Vec<u8>,
    pub environment: BTreeMap<String, String>,
}
impl std::fmt::Debug for Offer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Offer {{ bytes: {}, content: [redacted] }}",
            self.data.len()
        )
    }
}
impl Offer {
    pub fn spawn(&self) -> Result<process::Managed> {
        if self.data.is_empty() || self.data.len() > MAX || !self.data.starts_with(PNG) {
            return Err("the image could not be copied to the clipboard".into());
        }
        process::Managed::start(
            "/usr/bin/wl-copy",
            &["--foreground", "--type", "image/png"],
            &self.environment,
            &self.data,
            Duration::from_secs(3600),
            4096,
        )
        .map_err(|_| "the image could not be copied to the clipboard".into())
    }
}
pub fn project(value: &Value) -> Value {
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD
        .decode(value.as_str().unwrap_or(""))
        .unwrap_or_default();
    json!(paths(&data))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_uris_only_and_bounded_png() {
        assert_eq!(paths(b"file:///tmp/a%20b\nfile://foreign/tmp/private\nfile:///tmp/a%20b\nfile://localhost/tmp/c\nfile:///tmp/a%00"),vec!["/tmp/a b","/tmp/c"]);
        assert!(png(b"bad", &AtomicBool::new(false)).is_err());
        assert_eq!(png(PNG, &AtomicBool::new(false)).unwrap(), PNG);
    }
}
