//! Native launcher behind the existing Omagram CLI. Explicit paths for tests;
//! production discovery happens only when this entry point is deliberately used.
use crate::{process, storage::Storage};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{fs::MetadataExt, net::UnixStream, process::CommandExt},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
pub const PLUGIN_ID: &str = "io.github.pavellizunov.telebar";
const LEGACY_ID: &str = "reidenxerx.omagram";
const SHELL_QML: &[u8] =
    b"import Quickshell\nimport \"App\" as App\n\nShellRoot {\n  App.Main {}\n}\n";
#[derive(Clone)]
pub struct Context {
    pub home: PathBuf,
    pub runtime: PathBuf,
    pub plugin: PathBuf,
    pub binary: PathBuf,
    pub environment: BTreeMap<String, String>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    Window,
    Chat(String),
    Link(String),
    Service,
    DesktopEntry,
}
pub fn parse(args: &[String]) -> io::Result<Target> {
    Ok(match args {
        [] => Target::Window,
        [one] if one == "--service" => Target::Service,
        [one] if one == "--desktop-entry" => Target::DesktopEntry,
        [one] if telegram_url(one) => Target::Link(one.clone()),
        [flag, value] if flag == "--link" && telegram_url(value) => Target::Link(value.clone()),
        [flag, value] if flag == "--chat" && chat_id(value) => Target::Chat(value.clone()),
        _ => return Err(io::Error::other("invalid launcher arguments")),
    })
}
fn chat_id(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty() && digits.len() <= 20 && digits.bytes().all(|b| b.is_ascii_digit())
}
pub fn telegram_url(value: &str) -> bool {
    if value.is_empty() || value.chars().count() > 2048 || value.chars().any(|c| c <= ' ') {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    if let Some(tail) = lower.strip_prefix("tg://") {
        return !tail.split(['/', '?', '#']).next().unwrap_or("").is_empty();
    }
    let Some(tail) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = tail.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.split(':').next().unwrap_or("");
    !authority.contains('@') && matches!(host, "t.me" | "www.t.me" | "telegram.me" | "telegram.dog")
}
pub fn language(environment: &BTreeMap<String, String>) -> String {
    let raw = environment
        .get("LC_ALL")
        .filter(|s| !s.is_empty())
        .or_else(|| environment.get("LC_MESSAGES").filter(|s| !s.is_empty()))
        .or_else(|| environment.get("LANG"))
        .map_or("en", String::as_str);
    let value = raw.split('.').next().unwrap_or("en").replace('_', "-");
    let parts: Vec<_> = value.split('-').collect();
    if (2..=3).contains(&parts[0].len())
        && parts[0].bytes().all(|b| b.is_ascii_alphabetic())
        && parts.len() <= 2
        && parts.get(1).is_none_or(|p| {
            (2..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric())
        })
    {
        value
    } else {
        "en".into()
    }
}
/// Reject loader, interpreter and arbitrary caller variables. The systemd user
/// manager owns its session environment; child exec uses this small allowlist.
pub fn session_environment() -> BTreeMap<String, String> {
    std::env::vars()
        .filter(|(k, v)| {
            matches!(
                k.as_str(),
                "LANG"
                    | "LC_ALL"
                    | "LC_MESSAGES"
                    | "DISPLAY"
                    | "WAYLAND_DISPLAY"
                    | "DBUS_SESSION_BUS_ADDRESS"
                    | "HYPRLAND_INSTANCE_SIGNATURE"
                    | "PULSE_SERVER"
                    | "PIPEWIRE_REMOTE"
                    | "XAUTHORITY"
                    | "XDG_DATA_HOME"
                    | "XDG_CONFIG_HOME"
                    | "XDG_CACHE_HOME"
                    | "XDG_CURRENT_DESKTOP"
                    | "OMARCHY_PATH"
                    | "QT_FFMPEG_DECODING_HW_DEVICE_TYPES"
                    | "QT_FFMPEG_ENCODING_HW_DEVICE_TYPES"
            ) && v.len() <= 4096
                && !v.contains('\0')
        })
        .collect()
}
fn home() -> io::Result<PathBuf> {
    let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
    let mut found = std::ptr::null_mut();
    let mut buffer = vec![0u8; 65536];
    // SAFETY: all output buffers live through getpwuid_r; directory bytes are
    // copied before the scratch storage ends. No global passwd buffer is used.
    let code = unsafe {
        libc::getpwuid_r(
            libc::getuid(),
            &mut entry,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut found,
        )
    };
    if code != 0 || found.is_null() || entry.pw_dir.is_null() {
        return Err(io::Error::other("user home unavailable"));
    }
    let path = unsafe { std::ffi::CStr::from_ptr(entry.pw_dir) }
        .to_str()
        .map_err(|_| io::Error::other("invalid user home"))?;
    Ok(PathBuf::from(path))
}
impl Context {
    pub fn discover() -> io::Result<Self> {
        let binary = std::env::current_exe()?;
        // The already-running ELF is not a helper admission override. Ensure
        // its leaf is ours before a namespaced launch delegates to the host.
        let metadata = fs::symlink_metadata(&binary)?;
        if !metadata.is_file()
            || metadata.uid() != unsafe { libc::getuid() }
            || metadata.mode() & 0o022 != 0
        {
            return Err(io::Error::other("installed native image is not owned"));
        }
        let plugin = binary
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| io::Error::other("installed runtime path unavailable"))?
            .to_owned();
        if fs::metadata("/usr")?.uid() == 0 {
            crate::executable::checked(&binary)?;
        }
        let home = home()?;
        let uid = unsafe { libc::getuid() };
        let runtime = PathBuf::from(
            std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| format!("/run/user/{uid}")),
        );
        let mut environment = session_environment();
        environment.insert("HOME".into(), home.to_string_lossy().into_owned());
        environment.insert(
            "XDG_RUNTIME_DIR".into(),
            runtime.to_string_lossy().into_owned(),
        );
        Ok(Self {
            home,
            runtime,
            plugin,
            binary,
            environment,
        })
    }
    pub fn socket(&self) -> PathBuf {
        self.runtime.join("omagram/omagram.sock")
    }
    pub fn service_running(&self) -> bool {
        // Nonblocking connect bounds readiness even if a stale owner leaves a
        // full accept queue. The socket/lock remain the daemon's authority.
        let path = self.socket();
        if path.as_os_str().len() >= 108 || !path.is_absolute() {
            return false;
        }
        use std::os::fd::{FromRawFd, OwnedFd};
        let fd = unsafe {
            libc::socket(
                libc::AF_UNIX,
                libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                0,
            )
        };
        if fd < 0 {
            return false;
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        address.sun_family = libc::AF_UNIX as _;
        use std::os::unix::ffi::OsStrExt;
        for (dst, src) in address.sun_path.iter_mut().zip(path.as_os_str().as_bytes()) {
            *dst = *src as _;
        }
        unsafe {
            libc::connect(
                fd.as_raw_fd(),
                (&address as *const libc::sockaddr_un).cast(),
                std::mem::size_of_val(&address) as _,
            ) == 0
        }
    }
    pub fn managed_start(&self, unit: &str, mode: &str, args: &[String]) -> io::Result<bool> {
        let address = self
            .environment
            .get("DBUS_SESSION_BUS_ADDRESS")
            .ok_or_else(|| io::Error::other("user manager bus unavailable"))?;
        crate::host_service::start(address, unit, &self.binary, mode, args, None)
    }
    pub fn start_service(&self) -> io::Result<bool> {
        if self.service_running() {
            return Ok(true);
        }
        // A timed-out manager call may still have started the owner. Socket
        // readiness is authoritative; do not resubmit or restart an uncertain job.
        let started = self
            .managed_start("telebar-backend", "--daemon", &[])
            .unwrap_or(false);
        let deadline = Instant::now() + Duration::from_secs(if started { 10 } else { 2 });
        while Instant::now() < deadline {
            if self.service_running() {
                return Ok(true);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(false)
    }
    pub fn request_open(&self, target: &Target) -> io::Result<()> {
        let args = match target {
            Target::Chat(id) => {
                json!({"chatId":crate::projection::number(id.parse::<i128>().map_err(io::Error::other)?)})
            }
            Target::Link(url) => json!({"url":url}),
            _ => return Ok(()),
        };
        let mut socket = UnixStream::connect(self.socket())?;
        socket.set_read_timeout(Some(Duration::from_secs(2)))?;
        socket.set_write_timeout(Some(Duration::from_secs(2)))?;
        writeln!(socket, "{}", json!({"id":1,"cmd":"ui.open","args":args}))?;
        let mut buf = [0; 4096];
        let _ = socket.read(&mut buf)?;
        Ok(())
    }
    pub fn focus_existing(&self) -> bool {
        let result = process::run(
            "/usr/bin/hyprctl",
            &["-j", "clients"],
            &self.environment,
            &[],
            Duration::from_secs(4),
            4 * 1024 * 1024,
        );
        let clients = result
            .ok()
            .filter(|r| r.ok())
            .and_then(|r| crate::wire::bounded_json(&r.stdout, 8, 200000, 65536).ok());
        for client in clients
            .as_ref()
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let address = client["address"].as_str().unwrap_or("");
            if client["class"] == "omagram"
                && (3..=18).contains(&address.len())
                && address.starts_with("0x")
                && address[2..].bytes().all(|b| b.is_ascii_hexdigit())
            {
                return process::run(
                    "/usr/bin/hyprctl",
                    &[
                        "dispatch",
                        &format!("hl.dsp.focus({{ window = \"address:{address}\" }})"),
                    ],
                    &self.environment,
                    &[],
                    Duration::from_secs(4),
                    4096,
                )
                .is_ok_and(|r| r.ok());
            }
        }
        false
    }
    pub fn prepare_root(&self) -> io::Result<()> {
        let shell = PathBuf::from(
            self.environment
                .get("OMARCHY_PATH")
                .map_or("/usr/share/omarchy", String::as_str),
        )
        .join("shell");
        for name in ["Commons", "Ui"] {
            let m = fs::metadata(shell.join(name))?;
            if !m.is_dir() || m.uid() != 0 || m.mode() & 0o022 != 0 {
                return Err(io::Error::other("untrusted Omarchy imports"));
            }
        }
        self.prepare_root_with(&shell)
    }
    pub fn prepare_root_with(&self, shell: &Path) -> io::Result<()> {
        let storage = Storage::open(&self.runtime)?;
        let directory = storage.create_directory("omagram/app", 0o700)?;
        let anchor = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
        for (name, target) in [
            ("App", self.plugin.join("app")),
            ("bin", self.plugin.join("bin")),
            ("Commons", shell.join("Commons")),
            ("Ui", shell.join("Ui")),
        ] {
            let path = anchor.join(name);
            match fs::symlink_metadata(&path) {
                Ok(m) if m.file_type().is_symlink() => {
                    if fs::read_link(&path)? == target {
                        continue;
                    }
                    fs::remove_file(&path)?;
                }
                Ok(_) => return Err(io::Error::other("runtime import entry is not a link")),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
            std::os::unix::fs::symlink(target, path)?;
        }
        directory.sync_all()?;
        storage.write("omagram/app/shell.qml", SHELL_QML, 0o600)
    }
    pub fn applications_dir(&self) -> PathBuf {
        self.environment
            .get("XDG_DATA_HOME")
            .filter(|s| s.starts_with('/'))
            .map_or_else(|| self.home.join(".local/share"), PathBuf::from)
            .join("applications")
    }
    pub fn install_desktop_entry(&self) -> io::Result<&'static str> {
        let applications = self.applications_dir();
        let relative = applications
            .strip_prefix(&self.home)
            .ok()
            .and_then(Path::to_str)
            .ok_or_else(|| io::Error::other("desktop entry directory outside owned home"))?;
        let home = Storage::open(&self.home)?;
        let path = format!("{relative}/omagram.desktop");
        let current = match home.read(&path, 64 * 1024) {
            Ok(v) => v,
            Err(e) if e.to_string() == "storage file exceeds limit" => return Ok("foreign"),
            Err(e) => return Err(e),
        };
        let mut kept = BTreeSet::new();
        if let Some(bytes) = &current {
            let Some(text) = std::str::from_utf8(bytes).ok() else {
                return Ok("foreign");
            };
            let mut group = "";
            let mut ours = false;
            for line in text.lines().map(str::trim) {
                if line.starts_with('[') {
                    group = line;
                    continue;
                }
                if group != "[Desktop Entry]" {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    if k.trim() == "X-Omagram-Managed" && matches!(v.trim(), PLUGIN_ID | LEGACY_ID)
                    {
                        ours = true;
                    }
                    if matches!(k.trim(), "NoDisplay" | "Hidden") && v.trim() == "true" {
                        kept.insert(k.trim().to_owned());
                    }
                }
            }
            if !ours {
                return Ok("foreign");
            }
        }
        let Some(wanted) = desktop_entry(&self.plugin, &kept) else {
            return Ok("skipped");
        };
        if current.as_deref() == Some(wanted.as_bytes()) {
            return Ok("unchanged");
        }
        home.write(&path, wanted.as_bytes(), 0o644)?;
        Ok("written")
    }
    pub fn window_command(&self, executable: &Path) -> std::process::Command {
        let mut command = std::process::Command::new(executable);
        command
            .args(["--no-duplicate", "--path"])
            .arg(self.runtime.join("omagram/app"))
            .env_clear()
            .envs(&self.environment)
            .env("PATH", "/usr/bin")
            .env("QS_APP_ID", "omagram")
            // Use the user's FileChooser portal (including custom file managers).
            .env("QT_QPA_PLATFORMTHEME", "xdgdesktopportal")
            .env(
                "QT_FFMPEG_DECODING_HW_DEVICE_TYPES",
                self.environment
                    .get("QT_FFMPEG_DECODING_HW_DEVICE_TYPES")
                    .map_or("vaapi,cuda", String::as_str),
            )
            .env(
                "QT_FFMPEG_ENCODING_HW_DEVICE_TYPES",
                self.environment
                    .get("QT_FFMPEG_ENCODING_HW_DEVICE_TYPES")
                    .map_or("vaapi", String::as_str),
            );
        command
    }
    pub fn run(&self, args: &[String], host: bool) -> io::Result<()> {
        let target = parse(args)?;
        if !crate::session::CUTOVER_READY && target != Target::DesktopEntry {
            return Err(io::Error::other(
                "migration acceptance incomplete; launcher cutover disabled",
            ));
        }
        if target == Target::Service {
            return if self.start_service()? {
                Ok(())
            } else {
                Err(io::Error::other("service did not become ready"))
            };
        }
        if target == Target::DesktopEntry {
            if !host && fs::metadata("/usr")?.uid() != 0 {
                return if self.managed_start(
                    &format!("telebar-desktop-entry-{}", std::process::id()),
                    "--host-window",
                    args,
                )? {
                    Ok(())
                } else {
                    Err(io::Error::other("host desktop-entry owner unavailable"))
                };
            }
            eprintln!("omagram: launcher entry {}", self.install_desktop_entry()?);
            return Ok(());
        }
        if !host {
            return if self.managed_start(
                &format!("telebar-window-{}", std::process::id()),
                "--host-window",
                args,
            )? {
                Ok(())
            } else {
                Err(io::Error::other("window launch unavailable"))
            };
        }
        let _ = self.start_service();
        let _ = self.request_open(&target);
        if self.focus_existing() {
            return Ok(());
        }
        self.prepare_root()?;
        let executable = crate::executable::checked(Path::new("/usr/bin/quickshell"))?;
        Err(self.window_command(&executable).exec())
    }
}
fn plain(path: &Path) -> bool {
    path.to_str().is_some_and(|s| {
        s.starts_with('/')
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._+/-".contains(&b))
    })
}
pub fn desktop_entry(plugin: &Path, kept: &BTreeSet<String>) -> Option<String> {
    let launcher = plugin.join("bin/omagram");
    let icon = plugin.join("assets/omagram.svg");
    if !plain(&launcher) || !plain(&icon) || !launcher.is_file() || !icon.is_file() {
        return None;
    }
    let mut text = format!(
        "[Desktop Entry]\nType=Application\nVersion=1.5\nName=Telebar\nGenericName=Unofficial Telegram client\nComment=Chats, voice and round video messages in your Omarchy theme\nExec={} %u\nTryExec={}\nIcon={}\nTerminal=false\nCategories=Network;InstantMessaging;Chat;\nMimeType=x-scheme-handler/tg;\nKeywords=messages;chat;messenger;telegram;\nStartupWMClass=omagram\n",
        launcher.display(),
        launcher.display(),
        icon.display()
    );
    for key in ["NoDisplay", "Hidden"] {
        if kept.contains(key) {
            text.push_str(&format!("{key}=true\n"));
        }
    }
    text.push_str(&format!("X-Omagram-Managed={PLUGIN_ID}\n"));
    Some(text)
}
