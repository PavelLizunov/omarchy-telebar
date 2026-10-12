//! Generated input ownership. Never deletes a user's selected file; pre-native
//! failures clean exact snapshots. Submitted/unknown TDLib inputs are retained.
use crate::storage::Storage;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Clone, Debug)]
pub struct Lease(Arc<Owned>);
#[derive(Debug)]
struct Owned {
    home: PathBuf,
    paths: Vec<String>,
    submitted: AtomicBool,
}
impl Lease {
    pub fn new(home: PathBuf) -> Self {
        Self(Arc::new(Owned {
            home,
            paths: vec![],
            submitted: AtomicBool::new(false),
        }))
    }
    pub fn add(&mut self, path: String) {
        Arc::get_mut(&mut self.0)
            .expect("snapshot preparation owns unique lease")
            .paths
            .push(path);
    }
    pub fn path(&self, index: usize) -> &str {
        &self.0.paths[index]
    }
    pub fn submitted(&self) {
        self.0.submitted.store(true, Ordering::Relaxed);
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
        if let Ok(home) = Storage::open(&self.home) {
            for path in &self.paths {
                let _ = home.remove_file(path);
                if let Some(parent) = std::path::Path::new(path).parent().and_then(|p| p.to_str()) {
                    let _ = home.remove_empty_directory(parent);
                }
            }
        }
    }
}
