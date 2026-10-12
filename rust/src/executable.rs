//! Executable trust for fixed-argv helpers. Root tools resolve only under /usr;
//! owned application binaries have checked non-writable ancestor directories.
use std::{
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
pub fn checked(path: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(io::Error::other("helper path must be absolute"));
    }
    let real = fs::canonicalize(path)?;
    let meta = fs::metadata(&real)?;
    let uid = unsafe { libc::getuid() };
    let system = path.starts_with("/usr") || path.starts_with("/bin");
    let expected = if system { 0 } else { uid };
    if system && !real.starts_with("/usr") {
        return Err(io::Error::other("system helper resolves outside /usr"));
    }
    #[cfg(test)]
    if system && meta.uid() != expected && fixture_tool(&real) {
        return Ok(real);
    }
    if !meta.is_file()
        || meta.uid() != expected
        || meta.mode() & 0o022 != 0
        || meta.mode() & 0o111 == 0
    {
        return Err(io::Error::other(
            "helper is not an owned non-writable executable",
        ));
    }
    for parent in real.ancestors().skip(1) {
        let meta = fs::metadata(parent)?;
        let shared_temp = meta.uid() == 0 && meta.mode() & libc::S_ISVTX != 0;
        if !meta.is_dir()
            || (meta.uid() != 0 && meta.uid() != expected)
            || (meta.mode() & 0o022 != 0 && !(!system && shared_temp))
        {
            return Err(io::Error::other("unsafe helper ancestor"));
        }
    }
    Ok(real)
}

// Test-only namespace admission. Unprivileged bwrap cannot map host root, so
// helpers appear as overflow UID. A host-verified immutable hash manifest is
// required; release application code has no exception or environment override.
#[cfg(test)]
fn fixture_tool(path: &Path) -> bool {
    use sha2::Digest;
    use std::io::Read;
    if std::env::var("TELEBAR_ISOLATED_WAYLAND").as_deref() != Ok("1") {
        return false;
    }
    let result = || -> io::Result<bool> {
        let mapping = fs::read_to_string("/proc/self/uid_map")?;
        let values: Vec<_> = mapping.split_whitespace().collect();
        let uid = unsafe { libc::getuid() }.to_string();
        if values.len() != 3 || values[0] != uid || values[2] != "1" {
            return Ok(false);
        }
        let mut vfs: libc::statvfs = unsafe { std::mem::zeroed() };
        let usr = std::ffi::CString::new("/usr").expect("fixed path");
        if unsafe { libc::statvfs(usr.as_ptr(), &mut vfs) } != 0
            || vfs.f_flag & libc::ST_RDONLY == 0
        {
            return Ok(false);
        }
        let metadata = fs::metadata(path)?;
        if !metadata.is_file() || metadata.mode() & 0o022 != 0 || metadata.mode() & 0o111 == 0 {
            return Ok(false);
        }
        if !path.starts_with("/usr/bin") {
            return Ok(false);
        }
        let raw = fs::File::open("/test-helper-manifest.json")?;
        let mut bytes = Vec::new();
        raw.take(8193).read_to_end(&mut bytes)?;
        if bytes.len() > 8192 {
            return Ok(false);
        }
        let manifest: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        let mut file = fs::File::open(path)?;
        let mut hash = sha2::Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        let digest = format!("{:x}", hash.finalize());
        Ok(manifest[path
            .to_str()
            .ok_or_else(|| io::Error::other("invalid helper path"))?]
        .as_str()
            == Some(&digest))
    };
    result().unwrap_or(false)
}
