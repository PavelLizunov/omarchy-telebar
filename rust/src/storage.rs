//! Descriptor-anchored user storage. Every descendant is opened without symlink
//! traversal; writes are atomic and durable. No implicit real-home discovery.
use std::{
    ffi::CString,
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
static SERIAL: AtomicU64 = AtomicU64::new(1);
fn cstring(value: &str) -> io::Result<CString> {
    CString::new(value).map_err(|_| io::Error::other("invalid storage path"))
}
fn owned(file: &File, directory: bool) -> io::Result<()> {
    let m = file.metadata()?;
    // SAFETY: getuid has no pointers and no caller-owned state.
    if m.uid() != unsafe { libc::getuid() }
        || if directory {
            !m.is_dir() || m.mode() & 0o022 != 0
        } else {
            !m.is_file()
        }
    {
        return Err(io::Error::other("storage is not owned or safe"));
    }
    Ok(())
}
fn open_at(directory: &File, name: &str, flags: i32, mode: u32) -> io::Result<File> {
    let name = cstring(name)?;
    // SAFETY: directory is an owned open fd, name is NUL terminated; successful
    // openat returns a new descriptor whose unique owner is the resulting File.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            mode,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fd is newly returned and not otherwise wrapped or retained.
    Ok(unsafe { File::from_raw_fd(fd) })
}
pub struct Storage {
    root: File,
}
impl Storage {
    pub fn open(root: &Path) -> io::Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        if !root.is_absolute() {
            return Err(io::Error::other("storage root must be absolute"));
        }
        let mut directory = File::options()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open("/")?;
        // O_NOFOLLOW on only the leaf is insufficient: pin every ancestor.
        for part in root.components().skip(1) {
            let std::path::Component::Normal(part) = part else {
                return Err(io::Error::other("invalid storage root"));
            };
            let name = part
                .to_str()
                .ok_or_else(|| io::Error::other("invalid storage root"))?;
            directory = open_at(&directory, name, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
            let m = directory.metadata()?;
            // Root-owned sticky temporary parents are safe for disposable
            // owned fixtures; other shared writable ancestors are rejected.
            let uid = unsafe { libc::getuid() };
            if (m.uid() != 0 && m.uid() != uid)
                || (m.mode() & 0o022 != 0 && !(m.uid() == 0 && m.mode() & libc::S_ISVTX != 0))
            {
                return Err(io::Error::other("unsafe storage ancestor"));
            }
        }
        owned(&directory, true)?;
        Ok(Self { root: directory })
    }
    fn parts<'a>(&self, path: &'a str) -> io::Result<Vec<&'a str>> {
        let parts: Vec<_> = path.split('/').collect();
        if parts.is_empty()
            || parts
                .iter()
                .any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains('\0'))
        {
            return Err(io::Error::other("invalid relative storage path"));
        }
        Ok(parts)
    }
    fn directory(&self, parts: &[&str], create: bool) -> io::Result<File> {
        let mut dir = self.root.try_clone()?;
        for part in parts {
            let opened = open_at(&dir, part, libc::O_RDONLY | libc::O_DIRECTORY, 0);
            let next = match opened {
                Err(e) if create && e.kind() == io::ErrorKind::NotFound => {
                    let name = cstring(part)?;
                    // SAFETY: valid parent fd and NUL-terminated single component.
                    let result = unsafe { libc::mkdirat(dir.as_raw_fd(), name.as_ptr(), 0o700) };
                    if result != 0
                        && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists
                    {
                        return Err(io::Error::last_os_error());
                    }
                    open_at(&dir, part, libc::O_RDONLY | libc::O_DIRECTORY, 0)?
                }
                result => result?,
            };
            owned(&next, true)?;
            dir = next;
        }
        Ok(dir)
    }
    pub fn ensure_dir(&self, path: &str) -> io::Result<()> {
        let parts = self.parts(path)?;
        self.directory(&parts, true)?;
        Ok(())
    }
    pub fn create_directory(&self, path: &str, mode: u32) -> io::Result<File> {
        let parts = self.parts(path)?;
        let parent = self.directory(&parts[..parts.len() - 1], true)?;
        let name = cstring(parts[parts.len() - 1])?;
        // SAFETY: checked single component and pinned parent.
        if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), mode & !0o022) } != 0
            && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists
        {
            return Err(io::Error::last_os_error());
        }
        let file = open_at(
            &parent,
            parts[parts.len() - 1],
            libc::O_RDONLY | libc::O_DIRECTORY,
            0,
        )?;
        owned(&file, true)?;
        Ok(file)
    }
    pub fn open_read(&self, path: &str) -> io::Result<File> {
        let parts = self.parts(path)?;
        let parent = self.directory(&parts[..parts.len() - 1], false)?;
        let file = open_at(
            &parent,
            parts[parts.len() - 1],
            libc::O_RDONLY | libc::O_NONBLOCK,
            0,
        )?;
        owned(&file, false)?;
        Ok(file)
    }
    /// Unlink one owned regular entry under a descriptor-pinned private root.
    /// Missing entries are already removed; symlinks and special files fail.
    pub fn remove_file(&self, path: &str) -> io::Result<()> {
        let parts = self.parts(path)?;
        let parent = match self.directory(&parts[..parts.len() - 1], false) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            result => result?,
        };
        let name = parts[parts.len() - 1];
        let file = match open_at(&parent, name, libc::O_RDONLY | libc::O_NONBLOCK, 0) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            result => result?,
        };
        owned(&file, false)?;
        let name = cstring(name)?;
        // SAFETY: one checked entry in a retained private parent; unlinkat
        // removes the entry, never traverses a replacement symlink.
        if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } != 0 {
            return Err(io::Error::last_os_error());
        }
        parent.sync_all()
    }
    /// Remove only an empty owned directory; never recursively delete contents.
    pub fn remove_empty_directory(&self, path: &str) -> io::Result<()> {
        let parts = self.parts(path)?;
        let parent = self.directory(&parts[..parts.len() - 1], false)?;
        let directory = open_at(
            &parent,
            parts[parts.len() - 1],
            libc::O_RDONLY | libc::O_DIRECTORY,
            0,
        )?;
        owned(&directory, true)?;
        let name = cstring(parts[parts.len() - 1])?;
        // SAFETY: checked single component and pinned parent; AT_REMOVEDIR
        // refuses nonempty directories and never follows a replacement link.
        if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
            return Err(io::Error::last_os_error());
        }
        parent.sync_all()
    }
    pub fn read(&self, path: &str, limit: usize) -> io::Result<Option<Vec<u8>>> {
        let parts = self.parts(path)?;
        let dir = match self.directory(&parts[..parts.len() - 1], false) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            r => r?,
        };
        let mut file = match open_at(
            &dir,
            parts[parts.len() - 1],
            libc::O_RDONLY | libc::O_NONBLOCK,
            0,
        ) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            r => r?,
        };
        owned(&file, false)?;
        if file.metadata()?.len() > limit as u64 {
            return Err(io::Error::other("storage file exceeds limit"));
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(limit.saturating_add(1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            return Err(io::Error::other("storage file exceeds limit"));
        }
        Ok(Some(bytes))
    }
    /// Exclusive task-owned output in a descriptor-pinned private parent.
    pub fn create_file(&self, path: &str) -> io::Result<File> {
        let parts = self.parts(path)?;
        let dir = self.directory(&parts[..parts.len() - 1], true)?;
        let file = open_at(
            &dir,
            parts[parts.len() - 1],
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL,
            0o600,
        )?;
        owned(&file, false)?;
        dir.sync_all()?;
        Ok(file)
    }
    pub fn write(&self, path: &str, bytes: &[u8], mode: u32) -> io::Result<()> {
        let parts = self.parts(path)?;
        let dir = self.directory(&parts[..parts.len() - 1], true)?;
        let name = parts[parts.len() - 1];
        let mode = match open_at(&dir, name, libc::O_RDONLY | libc::O_NONBLOCK, 0) {
            Ok(file) => {
                owned(&file, false)?;
                file.metadata()?.mode() & 0o777 & !0o022
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => mode & 0o777 & !0o022,
            Err(e) => return Err(e),
        };
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let tmp = format!(".{name}.{}.{serial}.tmp", std::process::id());
        let mut file = open_at(
            &dir,
            &tmp,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            mode,
        )?;
        let result = (|| {
            file.write_all(bytes)?;
            // SAFETY: valid owned file descriptor; mode contains permission bits.
            if unsafe { libc::fchmod(file.as_raw_fd(), mode) } != 0 {
                return Err(io::Error::last_os_error());
            }
            file.sync_all()?;
            let temp = cstring(&tmp)?;
            let target = cstring(name)?;
            // SAFETY: both names are checked single components in the same open
            // directory. renameat replaces the entry, never follows its symlink.
            if unsafe {
                libc::renameat(
                    dir.as_raw_fd(),
                    temp.as_ptr(),
                    dir.as_raw_fd(),
                    target.as_ptr(),
                )
            } != 0
            {
                return Err(io::Error::last_os_error());
            }
            dir.sync_all()
        })();
        if result.is_err() {
            let temp = cstring(&tmp)?;
            // SAFETY: exact task-owned temporary entry in retained parent fd.
            unsafe {
                libc::unlinkat(dir.as_raw_fd(), temp.as_ptr(), 0);
            }
        }
        result
    }
    /// Copy explicitly selected bytes into a private stable TDLib input. The
    /// caller retains successful snapshots across uncertain native outcomes.
    pub fn snapshot_upload(
        &self,
        relative: &str,
        source: &mut File,
        size: u64,
        original_name: &str,
        cancel: &std::sync::atomic::AtomicBool,
        deadline: std::time::Instant,
    ) -> io::Result<String> {
        let before = source.metadata()?;
        if !before.is_file() || before.len() != size || size > 2 * 1024 * 1024 * 1024 {
            return Err(io::Error::other("invalid upload snapshot"));
        }
        let dir = self.directory(&self.parts(relative)?, true)?;
        if original_name.is_empty()
            || original_name.contains(['/', '\0'])
            || matches!(original_name, "." | "..")
        {
            return Err(io::Error::other("invalid upload name"));
        }
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let container = format!("upload-{}-{serial}", std::process::id());
        let component = cstring(&container)?;
        // SAFETY: exact new task-owned directory in checked parent.
        if unsafe { libc::mkdirat(dir.as_raw_fd(), component.as_ptr(), 0o700) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let parent = dir;
        let dir = open_at(&parent, &container, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
        let name = original_name;
        let mut output = open_at(
            &dir,
            name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            0o600,
        )?;
        let result = (|| {
            let mut left = size;
            let mut bytes = vec![0; 1024 * 1024];
            while left > 0 {
                if cancel.load(Ordering::Relaxed) || std::time::Instant::now() >= deadline {
                    return Err(io::Error::other("upload cancelled"));
                }
                let count = source.read(&mut bytes[..left.min(1024 * 1024) as usize])?;
                if count == 0 {
                    return Err(io::Error::other("upload changed"));
                }
                output.write_all(&bytes[..count])?;
                left -= count as u64;
            }
            let after = source.metadata()?;
            if after.len() != size
                || after.mtime() != before.mtime()
                || after.mtime_nsec() != before.mtime_nsec()
                || after.ctime() != before.ctime()
                || after.ctime_nsec() != before.ctime_nsec()
            {
                return Err(io::Error::other("upload changed"));
            }
            output.sync_all()?;
            dir.sync_all()
        })(); // Only our exclusively created failed copy is removed.
        if let Err(error) = result {
            let name = cstring(name)?;
            unsafe {
                libc::unlinkat(dir.as_raw_fd(), name.as_ptr(), 0);
                libc::unlinkat(parent.as_raw_fd(), component.as_ptr(), libc::AT_REMOVEDIR);
            }
            return Err(error);
        }
        Ok(format!("{relative}/{container}/{name}"))
    }
    /// Finite no-follow regular-file inventory relative to a pinned directory.
    pub fn files(&self, path: &str, limit: usize) -> io::Result<Vec<(String, std::fs::Metadata)>> {
        let parts = self.parts(path)?;
        let dir = match self.directory(&parts, false) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            result => result?,
        };
        let mut entries = Vec::new();
        for (index, entry) in
            std::fs::read_dir(format!("/proc/self/fd/{}", dir.as_raw_fd()))?.enumerate()
        {
            if index >= limit {
                return Err(io::Error::other("storage directory exceeds limit"));
            }
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let file = match open_at(&dir, &name, libc::O_RDONLY | libc::O_NONBLOCK, 0) {
                Ok(file) => file,
                Err(_) => continue,
            };
            if owned(&file, false).is_ok() {
                entries.push((name, file.metadata()?));
            }
        }
        Ok(entries)
    }
    pub fn nonempty_directory(&self, path: &str) -> io::Result<bool> {
        let parts = self.parts(path)?;
        let dir = match self.directory(&parts, false) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            r => r?,
        };
        // Linux /proc anchors inspection to the open, checked directory inode;
        // no database bytes are read or copied by this existence check.
        match std::fs::read_dir(format!("/proc/self/fd/{}", dir.as_raw_fd()))?.next() {
            Some(entry) => {
                entry?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::{
            fs::MetadataExt,
            fs::{PermissionsExt, symlink},
        },
    };
    #[test]
    fn atomic_storage_rejects_symlinks_and_bounds_reads() {
        let path = std::env::temp_dir().join(format!("telebar-storage-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let store = Storage::open(&path).unwrap();
        store
            .write("config/settings.json", b"first", 0o600)
            .unwrap();
        assert_eq!(
            store.read("config/settings.json", 5).unwrap(),
            Some(b"first".to_vec())
        );
        assert!(store.read("config/settings.json", 4).is_err());
        store
            .write("config/settings.json", b"second", 0o644)
            .unwrap();
        assert_eq!(
            fs::metadata(path.join("config/settings.json"))
                .unwrap()
                .mode()
                & 0o777,
            0o600
        );
        symlink("config", path.join("escape")).unwrap();
        assert!(store.read("escape/settings.json", 100).is_err());
        assert!(Storage::open(&path.join("escape")).is_err());
        assert!(Storage::open(&path.join("escape/child")).is_err());
        fs::create_dir(path.join("shared")).unwrap();
        fs::set_permissions(path.join("shared"), fs::Permissions::from_mode(0o777)).unwrap();
        fs::create_dir(path.join("shared/leaf")).unwrap();
        assert!(Storage::open(&path.join("shared/leaf")).is_err());
        symlink("settings.json", path.join("config/link")).unwrap();
        assert!(store.write("config/link", b"bad", 0o600).is_err());
        assert!(store.read("../config/settings.json", 100).is_err());
        assert!(store.read("/config/settings.json", 100).is_err());
        assert!(!store.nonempty_directory("missing").unwrap());
        assert!(store.nonempty_directory("config").unwrap());
        fs::remove_dir_all(path).unwrap();
    }
}
