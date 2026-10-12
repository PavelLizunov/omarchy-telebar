//! Same-user Unix socket and single-owner lock under a pinned private directory.
use std::{
    fs::{self, File},
    io,
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
            net::UnixListener,
        },
    },
    path::{Path, PathBuf},
};
pub struct Owner {
    pub listener: UnixListener,
    directory: File,
    lock: File,
    path: PathBuf,
    identity: (u64, u64),
}
impl Owner {
    pub fn acquire(directory: &Path, socket: &str, lock_name: &str) -> io::Result<Self> {
        for name in [socket, lock_name] {
            if name.is_empty() || name.contains(['/', '\0']) || matches!(name, "." | "..") {
                return Err(io::Error::other("invalid runtime entry"));
            }
        }
        // Storage pins and checks every ancestor, not just the last component.
        crate::storage::Storage::open(directory)?;
        let directory = File::options()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(directory)?;
        let meta = directory.metadata()?;
        if meta.mode() & 0o777 != 0o700 {
            return Err(io::Error::other("socket directory must be private"));
        }
        let anchored = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(anchored.join(lock_name))?;
        let meta = lock.metadata()?;
        // SAFETY: getuid has no caller memory or pointers.
        let uid = unsafe { libc::getuid() };
        if !meta.is_file() || meta.uid() != uid || meta.nlink() != 1 || meta.mode() & 0o777 != 0o600
        {
            return Err(io::Error::other(
                "runtime lock must be a private owned regular file",
            ));
        }
        // SAFETY: lock is a valid owned fd, retained until Owner is dropped.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let path = anchored.join(socket);
        match fs::symlink_metadata(&path) {
            Ok(meta) => {
                use std::os::unix::fs::FileTypeExt;
                if !meta.file_type().is_socket() || meta.uid() != uid || meta.nlink() != 1 {
                    return Err(io::Error::other(
                        "runtime socket path is not an owned socket",
                    ));
                }
                // A previous owner's socket is stale only after taking its lock.
                fs::remove_file(&path)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        // The owner is created before threads. umask is restored on all paths.
        // SAFETY: this startup-only call occurs before any worker is created.
        let mask = unsafe { libc::umask(0o177) };
        let result = UnixListener::bind(&path);
        // SAFETY: restore process-wide mask before creating workers.
        unsafe { libc::umask(mask) };
        let listener = result?;
        let meta = fs::symlink_metadata(&path)?;
        let mut owner = Self {
            listener,
            directory,
            lock,
            path,
            identity: (meta.dev(), meta.ino()),
        };
        if let Err(error) = fs::set_permissions(&owner.path, fs::Permissions::from_mode(0o600))
            .and_then(|_| owner.listener.set_nonblocking(true))
        {
            owner.remove_socket();
            return Err(error);
        }
        Ok(owner)
    }
    fn remove_socket(&mut self) {
        if fs::symlink_metadata(&self.path).is_ok_and(|m| (m.dev(), m.ino()) == self.identity) {
            let _ = fs::remove_file(&self.path);
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        // Directory and lock remain retained during identity-checked cleanup.
        self.remove_socket();
        let _ = (&self.directory, &self.lock);
    }
}
