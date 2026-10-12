//! Generated input ownership and a durable pre-dispatch journal. Telegram may
//! keep local paths for later playback: submitted/unknown inputs are never age
//! deleted. Finite admission bounds them instead; only proven unsent copies GC.
use crate::storage::Storage;
use serde_json::{Value, json};
use std::{
    fs::File,
    io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
pub const MAX_BYTES: u64 = 32 * 1024 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 4096;
const CHILD_ENTRIES: usize = 16;
const JOURNAL_RESERVE: u64 = 2 * 8192; // Current plus one atomic replacement.
pub const JOURNAL: &str = ".telebar-input-v1.json";
const LOCK: &str = ".telebar-input.lock";
#[derive(Clone)]
struct Entry {
    name: String,
    dev: u64,
    ino: u64,
}
struct Journal {
    version: u8,
    submitted: bool,
    files: Vec<Entry>,
}
fn child(path: &str) -> Option<(&str, &str)> {
    let (directory, name) = path.rsplit_once('/')?;
    let container = directory.rsplit('/').next()?;
    let (pid, serial) = container.strip_prefix("upload-")?.split_once('-')?;
    if pid.is_empty()
        || serial.is_empty()
        || !pid
            .bytes()
            .chain(serial.bytes())
            .all(|b| b.is_ascii_digit())
        || name == JOURNAL
    {
        return None;
    }
    Some((directory, name))
}
fn read_journal(store: &Storage, directory: &str) -> io::Result<Journal> {
    let bytes = store
        .read(&format!("{directory}/{JOURNAL}"), 8192)?
        .ok_or_else(|| io::Error::other("input journal is missing"))?;
    let value = crate::wire::bounded_json(&bytes, 5, 128, 4096)
        .map_err(|_| io::Error::other("invalid input journal"))?;
    let invalid = || io::Error::other("invalid input journal");
    if value.as_object().is_none_or(|v| v.len() != 3) {
        return Err(invalid());
    }
    let files = value["files"]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|e| {
            if e.as_object().is_none_or(|v| v.len() != 3) {
                return Err(invalid());
            }
            Ok(Entry {
                name: e["name"].as_str().ok_or_else(invalid)?.into(),
                dev: e["dev"].as_u64().ok_or_else(invalid)?,
                ino: e["ino"].as_u64().ok_or_else(invalid)?,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let journal = Journal {
        version: value["version"]
            .as_u64()
            .and_then(|v| u8::try_from(v).ok())
            .ok_or_else(invalid)?,
        submitted: value["submitted"].as_bool().ok_or_else(invalid)?,
        files,
    };
    if journal.version != 1
        || journal.files.is_empty()
        || journal.files.len() >= CHILD_ENTRIES
        || journal.files.iter().any(|e| {
            e.name.is_empty()
                || matches!(e.name.as_str(), "." | ".." | JOURNAL)
                || e.name.contains(['/', '\0'])
        })
    {
        return Err(io::Error::other("invalid input journal"));
    }
    let mut names = std::collections::HashSet::new();
    if journal.files.iter().any(|e| !names.insert(&e.name)) {
        return Err(io::Error::other("duplicate input journal entry"));
    }
    Ok(journal)
}
fn write_journal(store: &Storage, directory: &str, journal: &Journal) -> io::Result<()> {
    store.write(
        &format!("{directory}/{JOURNAL}"),
        &serde_json::to_vec(&json!({"version":journal.version,"submitted":journal.submitted,"files":journal.files.iter().map(|e|json!({"name":e.name,"dev":e.dev,"ino":e.ino})).collect::<Vec<Value>>()}))?,
        0o600,
    )
}
/// Called after exclusive file creation, before copying any bytes. A crash may
/// leave a partial file, but its inode and pre-dispatch state are already durable.
pub(crate) fn journal_copy(
    store: &Storage,
    directory: &str,
    name: &str,
    file: &File,
) -> io::Result<()> {
    let m = file.metadata()?;
    write_journal(
        store,
        directory,
        &Journal {
            version: 1,
            submitted: false,
            files: vec![Entry {
                name: name.into(),
                dev: m.dev(),
                ino: m.ino(),
            }],
        },
    )
}
fn exact(store: &Storage, path: &str, expected: &Entry) -> io::Result<bool> {
    match store.open_read(path) {
        Ok(file) => {
            let m = file.metadata()?;
            Ok(m.dev() == expected.dev
                && m.ino() == expected.ino
                && m.nlink() == 1
                && m.mode() & 0o022 == 0)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}
fn cleanup_journal(store: &Storage, directory: &str, journal: &Journal) -> io::Result<usize> {
    let mut removed = 0;
    for entry in &journal.files {
        let path = format!("{directory}/{}", entry.name);
        if store.remove_owned(&path, entry.dev, entry.ino)? {
            removed += 1;
        }
    }
    // A replacement or an unrelated added file is not ours. Leave it and the
    // journal intact; never recursively remove a container or traverse a link.
    let entries = store.entries(directory, CHILD_ENTRIES)?;
    if entries.len() == 1 && entries[0].0 == JOURNAL {
        let path = format!("{directory}/{JOURNAL}");
        let m = store.open_read(&path)?.metadata()?;
        store.remove_owned(&path, m.dev(), m.ino())?;
        store.remove_empty_directory(directory)?;
    }
    Ok(removed)
}
/// Retained lock serializes the complete reservation/write, not only counting.
/// Existing legacy inputs count toward the cap but have no deletion authority.
pub fn reserve(
    store: &Storage,
    directory: &str,
    bytes: u64,
    cancel: &AtomicBool,
    deadline: Instant,
) -> io::Result<File> {
    store.ensure_dir(directory)?;
    let lock = store.lock(&format!("{directory}/{LOCK}"), cancel, deadline)?;
    let reserved = bytes
        .checked_add(JOURNAL_RESERVE)
        .ok_or_else(|| io::Error::other("input budget exceeded"))?;
    let mut total = reserved;
    let mut count = 0usize;
    for (name, m) in store.entries(directory, MAX_ENTRIES)? {
        count += 1;
        if m.is_file() {
            total = total
                .checked_add(m.len())
                .ok_or_else(|| io::Error::other("input budget exceeded"))?;
        } else if m.is_dir() {
            let nested = format!("{directory}/{name}");
            for (_, m) in store.entries(&nested, CHILD_ENTRIES)? {
                if !m.is_file() {
                    return Err(io::Error::other(
                        "input inventory contains an unsupported entry",
                    ));
                }
                total = total
                    .checked_add(m.len())
                    .ok_or_else(|| io::Error::other("input budget exceeded"))?;
            }
        } else {
            return Err(io::Error::other(
                "input inventory contains an unsupported entry",
            ));
        }
    }
    if count >= MAX_ENTRIES - 1
        || total > MAX_BYTES
        || store.available_bytes(directory)? < reserved.saturating_add(512 * 1024 * 1024)
    {
        return Err(io::Error::other(
            "saved Telegram input budget exceeded; existing inputs were kept",
        ));
    }
    Ok(lock)
}
/// Startup only, after the single session owner lock and before admitting jobs.
/// The journal is changed to submitted durably *before* native dispatch. Missing,
/// malformed, submitted and inode-replaced journals/inputs remain untouched.
pub fn recover_unsent(store: &Storage, directory: &str) -> io::Result<usize> {
    store.ensure_dir(directory)?;
    let cancel = AtomicBool::new(false);
    let _lock = store.lock(
        &format!("{directory}/{LOCK}"),
        &cancel,
        Instant::now() + std::time::Duration::from_secs(2),
    )?;
    let mut removed = 0;
    for (name, m) in store.entries(directory, MAX_ENTRIES)? {
        if !m.is_dir() || child(&format!("{directory}/{name}/input")).is_none() {
            continue;
        }
        let path = format!("{directory}/{name}");
        if let Ok(journal) = read_journal(store, &path)
            && !journal.submitted
        {
            // Refuse the entire cleanup if any link/special/nested file makes
            // the inventory ambiguous. No partial cleanup through that entry.
            if let Ok(entries) = store.entries(&path, CHILD_ENTRIES)
                && entries.iter().all(|(_, m)| m.is_file())
            {
                removed += cleanup_journal(store, &path, &journal)?;
            }
        }
    }
    Ok(removed)
}
#[derive(Clone, Debug)]
pub struct Lease(Arc<Owned>);
struct Owned {
    home: PathBuf,
    store: Storage,
    paths: Vec<(String, Entry)>,
    submitted: AtomicBool,
}
impl std::fmt::Debug for Owned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputOwner")
            .field("count", &self.paths.len())
            .finish()
    }
}
impl Lease {
    pub fn new(home: PathBuf) -> io::Result<Self> {
        let store = Storage::open(&home)?;
        Ok(Self(Arc::new(Owned {
            home,
            store,
            paths: vec![],
            submitted: AtomicBool::new(false),
        })))
    }
    pub fn add(&mut self, path: String) -> io::Result<()> {
        let owned = Arc::get_mut(&mut self.0)
            .ok_or_else(|| io::Error::other("snapshot preparation requires unique ownership"))?;
        let file = owned.store.open_read(&path)?;
        let m = file.metadata()?;
        if m.nlink() != 1 || m.mode() & 0o022 != 0 {
            return Err(io::Error::other("input inode is not private"));
        }
        let entry = Entry {
            name: Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| io::Error::other("invalid input path"))?
                .into(),
            dev: m.dev(),
            ino: m.ino(),
        };
        if let Some((directory, _)) = child(&path) {
            let mut journal = read_journal(&owned.store, directory)?;
            if journal.submitted {
                return Err(io::Error::other("input already submitted"));
            }
            match journal.files.iter().find(|e| e.name == entry.name) {
                Some(e) if e.dev != entry.dev || e.ino != entry.ino => {
                    return Err(io::Error::other("input inode changed"));
                }
                Some(_) => {}
                None if journal.files.len() + 1 < CHILD_ENTRIES => {
                    journal.files.push(entry.clone());
                    write_journal(&owned.store, directory, &journal)?;
                }
                None => return Err(io::Error::other("too many generated inputs")),
            }
        }
        owned.paths.push((path, entry));
        Ok(())
    }
    /// A converter has consumed its scratch input, not the user's original.
    /// Remove only the captured inode and update its pre-dispatch journal.
    pub fn consumed(&mut self, path: &str) -> io::Result<()> {
        let owned = Arc::get_mut(&mut self.0)
            .ok_or_else(|| io::Error::other("input cleanup requires unique ownership"))?;
        let index = owned
            .paths
            .iter()
            .position(|(p, _)| p == path)
            .ok_or_else(|| io::Error::other("input is not owned"))?;
        let entry = &owned.paths[index].1;
        if !exact(&owned.store, path, entry)? {
            return Err(io::Error::other("input inode changed"));
        }
        if !owned.store.remove_owned(path, entry.dev, entry.ino)? {
            return Err(io::Error::other("input inode changed"));
        }
        if let Some((directory, _)) = child(path) {
            let mut journal = read_journal(&owned.store, directory)?;
            if journal.submitted {
                return Err(io::Error::other("input already submitted"));
            }
            journal.files.retain(|e| e.name != entry.name);
            write_journal(&owned.store, directory, &journal)?;
        }
        owned.paths.remove(index);
        Ok(())
    }
    pub fn path(&self, index: usize) -> &str {
        &self.0.paths[index].0
    }
    /// Disarm only the aggregate temporary owner after splitting album groups.
    /// This transfers cleanup ownership; it is NOT native submission/journaling.
    pub fn transferred(&self) {
        self.0.submitted.store(true, Ordering::Relaxed);
    }
    pub fn submitted(&self) -> io::Result<()> {
        // Validate what TDLib will resolve, not only what our retained fd sees.
        // A moved/replaced absolute home/path must refuse before native send.
        let visible = Storage::open(&self.0.home)?;
        if !visible.same_root(&self.0.store)? {
            return Err(io::Error::other("input root changed"));
        }
        let mut directories = std::collections::HashSet::new();
        for (path, entry) in &self.0.paths {
            // Every dispatched path must still be the exact owned inode.
            if !exact(&self.0.store, path, entry)? || !exact(&visible, path, entry)? {
                return Err(io::Error::other("input inode changed"));
            }
            if let Some((directory, _)) = child(path) {
                directories.insert(directory);
            }
        }
        for directory in directories {
            let mut journal = read_journal(&self.0.store, directory)?;
            journal.submitted = true;
            write_journal(&self.0.store, directory, &journal)?;
        }
        self.0.submitted.store(true, Ordering::Relaxed);
        Ok(())
    }
    pub fn rejected(&self, code: i128) {
        if (400..500).contains(&code) && code != 408 {
            self.0.submitted.store(false, Ordering::Relaxed);
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        if self.submitted.load(Ordering::Relaxed) {
            return;
        }
        let mut directories = std::collections::HashSet::new();
        for (path, entry) in &self.paths {
            let _ = self.store.remove_owned(path, entry.dev, entry.ino);
            if let Some((directory, _)) = child(path) {
                directories.insert(directory);
            } else if let Some(parent) = Path::new(path).parent().and_then(|p| p.to_str()) {
                let _ = self.store.remove_empty_directory(parent);
            }
        }
        for directory in directories {
            if let Ok(journal) = read_journal(&self.store, directory) {
                let all_absent = journal.files.iter().all(|entry| {
                    self.store
                        .open_read(&format!("{directory}/{}", entry.name))
                        .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
                });
                if all_absent {
                    let path = format!("{directory}/{JOURNAL}");
                    if let Ok(m) = self.store.open_read(&path).and_then(|f| f.metadata()) {
                        let _ = self.store.remove_owned(&path, m.dev(), m.ino());
                    }
                    let _ = self.store.remove_empty_directory(directory);
                }
            }
        }
    }
}
