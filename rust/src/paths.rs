use std::{
    fs, io,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

pub fn valid_account_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.contains("..")
        && id != "."
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

pub struct AccountPaths {
    pub root: PathBuf,
    pub database: PathBuf,
    pub files: PathBuf,
    pub sent: PathBuf,
}

impl AccountPaths {
    pub fn new(data: &Path, id: &str) -> Result<Self, &'static str> {
        if !valid_account_id(id) {
            return Err("invalid account id");
        }
        let root = if id == "default" {
            data.to_owned()
        } else {
            data.join("accounts").join(id)
        };
        Ok(Self {
            database: root.join("database"),
            files: root.join("files"),
            sent: root.join("sent"),
            root,
        })
    }

    pub fn media_roots(&self, runtime: &Path) -> Vec<String> {
        let mut roots = vec![self.files.to_string_lossy().into_owned()];
        roots.extend(
            [
                "stickers",
                "thumbnails",
                "profile_photos",
                "wallpapers",
                "stories",
            ]
            .map(|name| self.database.join(name).to_string_lossy().into_owned()),
        );
        roots.push(self.sent.to_string_lossy().into_owned());
        roots.push(runtime.join("omagram/rec").to_string_lossy().into_owned());
        roots
    }
}

fn owned(metadata: &fs::Metadata) -> bool {
    // SAFETY: getuid has no arguments and does not access caller memory.
    metadata.uid() == unsafe { libc::getuid() } && metadata.mode() & 0o022 == 0
}

pub fn library_file(path: &Path) -> io::Result<fs::File> {
    for dir in [path.parent().and_then(Path::parent), path.parent()] {
        let dir = dir.ok_or_else(|| io::Error::other("invalid TDLib path"))?;
        let meta = fs::symlink_metadata(dir)?;
        if !meta.is_dir() || !owned(&meta) {
            return Err(io::Error::other(
                "TDLib directory is not owned and non-writable",
            ));
        }
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || !owned(&meta) {
        return Err(io::Error::other(
            "TDLib is not a regular owned non-writable file",
        ));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_media_identities_stay_compatible() {
        for invalid in ["", ".", "..", "one..two", "a/b", "é", "/tmp", "a b"] {
            assert!(!valid_account_id(invalid), "{invalid}");
        }
        let p =
            AccountPaths::new(Path::new("/home/fixture/.local/share/omagram"), "default").unwrap();
        assert_eq!(
            p.database,
            Path::new("/home/fixture/.local/share/omagram/database")
        );
        assert_eq!(
            p.media_roots(Path::new("/run/user/123"))[6],
            "/home/fixture/.local/share/omagram/sent"
        );
        let p = AccountPaths::new(Path::new("/data"), "acc-2").unwrap();
        assert_eq!(p.database, Path::new("/data/accounts/acc-2/database"));
    }
}
