//! Assemble a new, inert native candidate. Never install, enable, or replace an
//! existing tree here; lifecycle/cutover acceptance remains a separate owner.
use crate::{executable, session::CUTOVER_READY, storage::Storage};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    path::Path,
};
const MAX_FILES: usize = 512;
const MAX_BYTES: usize = 32 * 1024 * 1024;
const ENTRIES: [(&str, &str); 5] = [
    ("omagram", "--launch"),
    ("omagramd", "--daemon"),
    ("omagram-menu-install", "--menu"),
    ("omagram-build-tdlib", "--build-tdlib"),
    ("omagram-proxy", "--proxy"),
];
struct Input {
    path: String,
    bytes: Vec<u8>,
}
fn collect(source: &Path, store: &Storage, path: &str, files: &mut Vec<Input>) -> io::Result<()> {
    if path.split('/').count() > 8 || files.len() >= MAX_FILES {
        return Err(io::Error::other("package inventory exceeds limit"));
    }
    let metadata = fs::symlink_metadata(source.join(path))?;
    if metadata.uid() != unsafe { libc::getuid() } || metadata.mode() & 0o022 != 0 {
        return Err(io::Error::other(
            "package source is not owned and non-writable",
        ));
    }
    if metadata.is_dir() {
        // Inventory may race a source edit, but all bytes are read through the
        // pinned Storage root without following any descendant symlink.
        let directory = store.open_directory(path)?;
        let mut paths = fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))?
            .take(MAX_FILES + 1)
            .map(|entry| {
                let name = entry?.file_name();
                let name = name
                    .to_str()
                    .ok_or_else(|| io::Error::other("invalid package name"))?;
                if name.is_empty()
                    || name.starts_with('.')
                    || !name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                {
                    return Err(io::Error::other("invalid package name"));
                }
                Ok(format!("{path}/{name}"))
            })
            .collect::<io::Result<Vec<_>>>()?;
        if paths.len() > MAX_FILES {
            return Err(io::Error::other("package inventory exceeds limit"));
        }
        paths.sort();
        for child in paths {
            collect(source, store, &child, files)?;
        }
    } else if metadata.is_file() {
        let extension = Path::new(path)
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("");
        let admitted = match path.split('/').next().unwrap_or("") {
            "app" => matches!(extension, "qml" | "js" | "json"),
            "shell" => matches!(extension, "qml" | "js"),
            "assets" => extension == "svg",
            "LICENSES" => matches!(extension, "txt" | "md"),
            _ => matches!(
                path,
                "manifest.json" | "menu.jsonc" | "LICENSE" | "THIRD_PARTY_NOTICES.md"
            ),
        };
        if !admitted {
            return Err(io::Error::other("unapproved native package file type"));
        }
        let bytes = store
            .read(path, 16 * 1024 * 1024)?
            .ok_or_else(|| io::Error::other("package source disappeared"))?;
        if bytes.len() + files.iter().map(|f| f.bytes.len()).sum::<usize>() > MAX_BYTES {
            return Err(io::Error::other("package content exceeds limit"));
        }
        files.push(Input {
            path: path.into(),
            bytes,
        });
    } else {
        return Err(io::Error::other(
            "package source contains a link or special file",
        ));
    }
    Ok(())
}
fn shim(mode: &str) -> String {
    // Bash privileged mode suppresses BASH_ENV and exported function imports.
    // Fixed argv/quoted location preserve spaces; no shell evaluation.
    format!(
        "#!/usr/bin/bash -p\nset -euo pipefail\nexport PATH=/usr/bin\nunset BASH_ENV ENV CDPATH GLOBIGNORE\nexec -- \"${{BASH_SOURCE[0]%/*}}/telebar-runtime\" {mode} \"$@\"\n"
    )
}
pub fn assemble(source: &Path, destination: &Path, binary: &Path) -> io::Result<()> {
    let store = Storage::open(source)?;
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::other("package parent missing"))?;
    let output_parent = Storage::open(parent)?;
    let output_name = destination
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| io::Error::other("invalid output directory name"))?;
    if destination.starts_with(source)
        || source.starts_with(destination)
        || fs::symlink_metadata(destination).is_ok()
    {
        return Err(io::Error::other(
            "package destination must be new and outside the source",
        ));
    }
    let mut files = Vec::new();
    for root in ["app", "shell", "assets", "LICENSES"] {
        collect(source, &store, root, &mut files)?;
    }
    for path in [
        "manifest.json",
        "menu.jsonc",
        "LICENSE",
        "THIRD_PARTY_NOTICES.md",
    ] {
        collect(source, &store, path, &mut files)?;
    }
    let license = store
        .read("bin/vendor/flowseal/LICENSE", 65536)?
        .ok_or_else(|| io::Error::other("Flowseal license missing"))?;
    if files.iter().any(|f| f.path == "LICENSES/Flowseal-MIT.txt") {
        return Err(io::Error::other("duplicate Flowseal license output"));
    }
    files.push(Input {
        path: "LICENSES/Flowseal-MIT.txt".into(),
        bytes: license,
    });
    let manifest: serde_json::Value = serde_json::from_slice(
        &files
            .iter()
            .find(|f| f.path == "manifest.json")
            .unwrap()
            .bytes,
    )
    .map_err(io::Error::other)?;
    if manifest["schemaVersion"] != 1
        || manifest["id"] != crate::launcher::PLUGIN_ID
        || manifest["kinds"] != json!(["service", "bar-widget", "overlay"])
        || !manifest["entryPoints"].as_object().is_some_and(|entries| {
            entries.len() == 3
                && ["service", "barWidget", "overlay"]
                    .iter()
                    .all(|name| entries.contains_key(*name))
        })
    {
        return Err(io::Error::other("package plugin identity/contract changed"));
    }
    for entry in manifest["entryPoints"]
        .as_object()
        .ok_or_else(|| io::Error::other("package entry points missing"))?
        .values()
    {
        let name = entry
            .as_str()
            .ok_or_else(|| io::Error::other("invalid package entry point"))?;
        if !files.iter().any(|f| f.path == name) {
            return Err(io::Error::other("package entry point missing"));
        }
    }
    if files.iter().any(|f| {
        (f.path.ends_with(".qml") || f.path.ends_with(".js") || f.path == "menu.jsonc")
            && String::from_utf8_lossy(&f.bytes)
                .to_ascii_lowercase()
                .contains("python")
    }) {
        return Err(io::Error::other("package consumer still invokes Python"));
    }
    let binary = executable::checked(binary)?;
    let binary_store = Storage::open(binary.parent().unwrap())?;
    let runtime = binary_store
        .read(
            binary
                .file_name()
                .unwrap()
                .to_str()
                .ok_or_else(|| io::Error::other("invalid binary name"))?,
            512 * 1024 * 1024,
        )?
        .ok_or_else(|| io::Error::other("runtime disappeared"))?;
    if !runtime.starts_with(b"\x7fELF") {
        return Err(io::Error::other("package requires a native ELF runtime"));
    }
    let mut record = serde_json::Map::new();
    let notices = files
        .iter_mut()
        .find(|f| f.path == "THIRD_PARTY_NOTICES.md")
        .unwrap();
    notices.bytes = std::str::from_utf8(&notices.bytes)
        .map_err(io::Error::other)?
        .replace(
            "`bin/vendor/flowseal/LICENSE`",
            "`LICENSES/Flowseal-MIT.txt`",
        )
        .into_bytes();
    for input in &files {
        record.insert(
            input.path.clone(),
            json!(format!("{:x}", Sha256::digest(&input.bytes))),
        );
    }
    // All inputs are admitted before the exclusive output directory exists.
    let output = output_parent.create_new_child(output_name)?;
    for input in files {
        output.write(&input.path, &input.bytes, 0o644)?;
    }
    output.write("bin/telebar-runtime", &runtime, 0o755)?;
    record.insert(
        "bin/telebar-runtime".into(),
        json!(format!("{:x}", Sha256::digest(&runtime))),
    );
    for (name, mode) in ENTRIES {
        let path = format!("bin/{name}");
        let body = shim(mode);
        output.write(&path, body.as_bytes(), 0o755)?;
        record.insert(
            path,
            json!(format!("{:x}", Sha256::digest(body.as_bytes()))),
        );
    }
    output.write("runtime-candidate.json",&serde_json::to_vec_pretty(&json!({"schemaVersion":1,"runtime":"rust","version":env!("CARGO_PKG_VERSION"),"cutoverReady":CUTOVER_READY,"sourceHashes":record}))?,0o644)?;
    let launch_status = if CUTOVER_READY {
        "Production entry points are enabled for the checked cutover batch. A package\nmanifest is not proof of installation, loaded accounts or manual acceptance.\nNever run it alongside another owner of the same TDLib databases.\n"
    } else {
        "Production daemon/window launches remain gated. Do not install this candidate\njust because it builds. Complete acceptance and a safe single-owner cutover first.\n"
    };
    let readme = format!(
        "# Telebar native runtime\n\nThis package contains QML/JavaScript, one Rust ELF runtime and Bash entry shims.\nIt does not contain a Python runtime or fallback. Official TDLib remains in\n~/.local/share/omagram/lib/; accounts, keyring and socket identities are unchanged.\n\n{launch_status}\nruntime-candidate.json identifies the exact package inputs and launch gate.\nRequires Omarchy/Quickshell, systemd's user manager, Bash, OpenSSL, libsecret's\nsecret-tool, ffmpeg, PipeWire tools, wl-clipboard, xdg-utils, qrencode and the\nQt Multimedia/FFmpeg/Lottie modules. Rust/Cargo are build-only dependencies.\nThe entry names omagram, omagramd, omagram-menu-install, omagram-build-tdlib and\nomagram-proxy preserve their command contracts and use the native runtime.\n"
    );
    output.write("README.md", readme.as_bytes(), 0o644)
}
