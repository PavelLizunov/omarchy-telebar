//! Native bounded user-manager handoff. No root helper needs to execute inside
//! a recovery user namespace; host-owned children enforce the normal trust checks.
use futures_lite::{StreamExt, future};
use std::{io, os::unix::fs::MetadataExt, path::Path, time::Duration};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
type Properties = Vec<(String, OwnedValue)>;
fn owned(value: Value<'_>) -> io::Result<OwnedValue> {
    value.try_to_owned().map_err(io::Error::other)
}
pub fn properties(
    binary: &Path,
    mode: &str,
    args: &[String],
    lifetime: Option<Duration>,
) -> io::Result<Properties> {
    if !binary.is_absolute()
        || args.len() > 16
        || args.iter().any(|s| s.len() > 4096 || s.contains('\0'))
        || !matches!(mode, "--daemon" | "--host-window" | "--inert-no-library")
    {
        return Err(io::Error::other("invalid managed process contract"));
    }
    if mode == "--host-window" {
        crate::launcher::parse(args)?;
    }
    if mode == "--daemon" && !args.is_empty() {
        return Err(io::Error::other("invalid daemon handoff"));
    }
    if mode == "--inert-no-library"
        && (lifetime.is_none() || args.len() != 1 || !Path::new(&args[0]).is_absolute())
    {
        return Err(io::Error::other("finite explicit inert handoff required"));
    }
    let path = binary
        .to_str()
        .ok_or_else(|| io::Error::other("invalid managed executable"))?;
    let mut argv = vec![path.to_owned(), mode.into()];
    argv.extend_from_slice(args);
    let mut values = vec![
        (
            "Description".into(),
            owned(Value::new("Telebar owned native process"))?,
        ),
        ("Type".into(), owned(Value::new("exec"))?),
        (
            "CollectMode".into(),
            owned(Value::new("inactive-or-failed"))?,
        ),
        ("KillMode".into(), owned(Value::new("control-group"))?),
        ("TimeoutStopUSec".into(), owned(Value::new(8_000_000u64))?),
        (
            "Environment".into(),
            owned(Value::new(vec!["PATH=/usr/bin"]))?,
        ),
        (
            "UnsetEnvironment".into(),
            owned(Value::new(vec![
                "LD_PRELOAD",
                "LD_LIBRARY_PATH",
                "LD_AUDIT",
                "LD_DEBUG",
                "LD_DEBUG_OUTPUT",
                "LD_PROFILE",
                "LD_SHOW_AUXV",
                "LD_TRACE_LOADED_OBJECTS",
                "GLIBC_TUNABLES",
                "GCONV_PATH",
                "LOCPATH",
                "PYTHONPATH",
                "PYTHONHOME",
                "BASH_ENV",
                "ENV",
            ]))?,
        ),
        (
            "ExecStart".into(),
            owned(Value::new(vec![(path.to_owned(), argv, false)]))?,
        ),
    ];
    if let Some(lifetime) = lifetime {
        if lifetime.is_zero() || lifetime > Duration::from_secs(120) {
            return Err(io::Error::other("invalid fixture lifetime"));
        }
        values.push((
            "RuntimeMaxUSec".into(),
            owned(Value::new(lifetime.as_micros() as u64))?,
        ));
    }
    Ok(values)
}
pub fn start(
    address: &str,
    unit: &str,
    binary: &Path,
    mode: &str,
    args: &[String],
    lifetime: Option<Duration>,
) -> io::Result<bool> {
    if !address.starts_with("unix:path=/")
        || address.len() > 4096
        || address.contains(';')
        || unit.len() > 120
        || unit.is_empty()
        || !unit
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err(io::Error::other("explicit local manager and unit required"));
    }
    // This is an owned application binary, not a system-tool admission override.
    // Ancestor/root UID checks occur in the host child before profiles or helpers
    // are touched; only its known native image is delegated from a namespace.
    let metadata = std::fs::symlink_metadata(binary)?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::getuid() }
        || metadata.mode() & 0o022 != 0
        || metadata.mode() & 0o111 == 0
    {
        return Err(io::Error::other(
            "managed native image is not owned or executable",
        ));
    }
    let values = properties(binary, mode, args, lifetime)?;
    let unit = if unit.ends_with(".service") {
        unit.to_owned()
    } else {
        format!("{unit}.service")
    };
    async_io::block_on(future::race(
        async {
            let connection = zbus::connection::Builder::address(address)
                .map_err(io::Error::other)?
                .method_timeout(Duration::from_secs(3))
                .max_queued(16)
                .build()
                .await
                .map_err(io::Error::other)?;
            let manager = zbus::Proxy::new(
                &connection,
                "org.freedesktop.systemd1",
                "/org/freedesktop/systemd1",
                "org.freedesktop.systemd1.Manager",
            )
            .await
            .map_err(io::Error::other)?;
            // Subscribe before submission so even a fast exec/failure is
            // observed. A queued job is not proof that its ELF was executed.
            let mut removed = manager
                .receive_signal("JobRemoved")
                .await
                .map_err(io::Error::other)?;
            manager
                .call::<_, _, ()>("Subscribe", &())
                .await
                .map_err(io::Error::other)?;
            let aux: Vec<(String, Properties)> = Vec::new();
            // Fail, never replace/restart an existing backend or active window unit.
            let Ok(job) = manager
                .call::<_, _, OwnedObjectPath>("StartTransientUnit", &(unit, "fail", values, aux))
                .await
            else {
                return Ok(false);
            };
            while let Some(message) = removed.next().await {
                if let Ok((_, path, _, result)) =
                    message
                        .body()
                        .deserialize::<(u32, OwnedObjectPath, String, String)>()
                    && path == job
                {
                    return Ok(result == "done");
                }
            }
            Ok(false)
        },
        async {
            async_io::Timer::after(Duration::from_secs(5)).await;
            Err(io::Error::other("user manager handoff timed out"))
        },
    ))
}
