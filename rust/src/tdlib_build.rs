//! Pinned official TDLib build helper; no system installation.
use crate::{launcher::Context, process, storage::Storage, td::Td};
use serde_json::json;
use std::{io, path::Path, time::Duration};
const REPO: &str = "https://github.com/tdlib/td.git";
const COMMIT: &str = "d1085f9cebc5a62379991ae1652673954f229c1f";
pub fn memory_jobs() -> usize {
    let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    let size = unsafe { libc::sysconf(libc::_SC_PAGE_SIZE) };
    let memory = if pages > 0 && size > 0 {
        pages as u64 * size as u64
    } else {
        8 * 1024 * 1024 * 1024
    };
    std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min((memory / (2 * 1024 * 1024 * 1024)).max(1) as usize)
}
fn verify(context: &Context) -> io::Result<()> {
    let library = context.home.join(".local/share/omagram/lib/libtdjson.so");
    let mut td = Td::open(&library)?;
    let version = td
        .execute(&json!({"@type":"getOption","name":"version"}))?
        .unwrap_or_default();
    println!(
        "TDLib {} is ready at {}",
        version["value"].as_str().unwrap_or("?"),
        library.display()
    );
    td.close(Duration::ZERO)
}
fn logged(
    context: &Context,
    args: &[&str],
    cwd: Option<&Path>,
    timeout: Duration,
    environment: &std::collections::BTreeMap<String, String>,
) -> io::Result<bool> {
    let tool = crate::executable::checked(Path::new(args[0]))?;
    let mut command = std::process::Command::new(tool);
    command
        .args(&args[1..])
        .env_clear()
        .envs(environment)
        .env("PATH", "/usr/bin")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    // Build output is retained in a task-owned capped log and the existing finite
    // process owner retains PID/group identity. No detached build or shell.
    process::run_command_logged(
        command,
        timeout,
        &Storage::open(&context.home)?,
        ".cache/omagram/build.log",
        8 * 1024 * 1024,
        args,
    )
}
pub fn run(context: &Context, args: &[String]) -> io::Result<()> {
    if args == ["--check"] {
        return verify(context);
    }
    if !args.is_empty() {
        return Err(io::Error::other("invalid TDLib build arguments"));
    }
    let home = Storage::open(&context.home)?;
    home.ensure_dir(".cache/omagram")?;
    let source = context.home.join(".cache/omagram/td");
    let build = context.home.join(".cache/omagram/build");
    let compiler = ["clang++", "g++"]
        .into_iter()
        .find(|name| crate::executable::checked(Path::new(&format!("/usr/bin/{name}"))).is_ok())
        .ok_or_else(|| io::Error::other("missing clang or gcc"))?;
    for name in ["git", "cmake", "gperf"] {
        crate::executable::checked(Path::new(&format!("/usr/bin/{name}")))?;
    }
    let environment = context.environment.clone();
    let fresh = !source.join(".git").is_dir();
    if fresh {
        if source.exists() {
            return Err(io::Error::other(
                "TDLib source exists without an owned Git checkout",
            ));
        }
        println!("Fetching TDLib 1.8.67…");
        if !logged(
            context,
            &[
                "/usr/bin/git",
                "clone",
                "--filter=blob:none",
                "--no-checkout",
                REPO,
                source
                    .to_str()
                    .ok_or_else(|| io::Error::other("invalid build path"))?,
            ],
            None,
            Duration::from_secs(900),
            &environment,
        )? {
            return Err(io::Error::other("could not fetch TDLib"));
        }
    }
    Storage::open(&source)?;
    // Native build files execute commands. Verify the existing remote and
    // tracked source freshness before fetching/building from this owned cache.
    let remote = process::run(
        "/usr/bin/git",
        &[
            "-C",
            source.to_str().unwrap(),
            "remote",
            "get-url",
            "origin",
        ],
        &environment,
        &[],
        Duration::from_secs(30),
        4096,
    )?;
    if !remote.ok() || String::from_utf8_lossy(&remote.stdout).trim() != REPO {
        return Err(io::Error::other("TDLib cache has an unexpected remote"));
    }
    if !fresh {
        let dirty = process::run(
            "/usr/bin/git",
            &["-C", source.to_str().unwrap(), "status", "--porcelain"],
            &environment,
            &[],
            Duration::from_secs(30),
            65536,
        )?;
        if !dirty.ok() || !dirty.stdout.is_empty() {
            return Err(io::Error::other("TDLib source has local changes"));
        }
    }
    let head = process::run(
        "/usr/bin/git",
        &["-C", source.to_str().unwrap(), "rev-parse", "HEAD"],
        &environment,
        &[],
        Duration::from_secs(30),
        4096,
    )?;
    if !head.ok() || String::from_utf8_lossy(&head.stdout).trim() != COMMIT {
        if !logged(
            context,
            &[
                "/usr/bin/git",
                "-C",
                source.to_str().unwrap(),
                "fetch",
                "--depth",
                "1",
                "origin",
                COMMIT,
            ],
            None,
            Duration::from_secs(900),
            &environment,
        )? {
            return Err(io::Error::other("could not fetch pinned TDLib"));
        }
        // Never overwrite an unrelated dirty source checkout to make a build pass.
        let dirty = process::run(
            "/usr/bin/git",
            &["-C", source.to_str().unwrap(), "status", "--porcelain"],
            &environment,
            &[],
            Duration::from_secs(30),
            65536,
        )?;
        if !dirty.ok() || (!fresh && !dirty.stdout.is_empty()) {
            return Err(io::Error::other("TDLib source has local changes"));
        }
        if !logged(
            context,
            &[
                "/usr/bin/git",
                "-C",
                source.to_str().unwrap(),
                "checkout",
                "--quiet",
                COMMIT,
            ],
            None,
            Duration::from_secs(30),
            &environment,
        )? {
            return Err(io::Error::other("could not select pinned TDLib"));
        }
    }
    home.ensure_dir(".cache/omagram/build")?;
    let mut env = environment;
    env.insert(
        "CC".into(),
        format!(
            "/usr/bin/{}",
            if compiler == "clang++" {
                "clang"
            } else {
                "gcc"
            }
        ),
    );
    env.insert("CXX".into(), format!("/usr/bin/{compiler}"));
    println!("Configuring TDLib…");
    let mut args = vec![
        "/usr/bin/cmake",
        "-DCMAKE_BUILD_TYPE=Release",
        "-DTD_ENABLE_LTO=OFF",
    ];
    if crate::executable::checked(Path::new("/usr/bin/ninja")).is_ok() {
        args.extend(["-G", "Ninja"]);
    }
    args.push(source.to_str().unwrap());
    if !logged(context, &args, Some(&build), Duration::from_secs(600), &env)? {
        return Err(io::Error::other("TDLib configuration failed"));
    }
    let jobs = memory_jobs().to_string();
    println!("Building TDLib with {jobs} parallel jobs…");
    if !logged(
        context,
        &[
            "/usr/bin/cmake",
            "--build",
            ".",
            "--target",
            "tdjson",
            "--parallel",
            &jobs,
        ],
        Some(&build),
        Duration::from_secs(4 * 3600),
        &env,
    )? {
        return Err(io::Error::other("TDLib build failed"));
    }
    let mut candidates = home
        .files(".cache/omagram/build", 4096)?
        .into_iter()
        .filter(|(name, meta)| name.starts_with("libtdjson.so") && meta.is_file())
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    candidates.sort();
    let name = candidates
        .pop()
        .ok_or_else(|| io::Error::other("TDLib build output missing"))?;
    let library = home
        .read(&format!(".cache/omagram/build/{name}"), 512 * 1024 * 1024)?
        .filter(|b| !b.is_empty())
        .ok_or_else(|| io::Error::other("empty TDLib build output"))?;
    home.ensure_dir(".local/share/omagram/lib")?;
    home.write(".local/share/omagram/lib/libtdjson.so", &library, 0o644)?;
    verify(context)
}
