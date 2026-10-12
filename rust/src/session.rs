//! Explicit application context shared by startup, accounts and desktop helpers.
//! Construction validates storage only; registration never opens a database.
use crate::{
    application::Application, native_driver::Driver, paths::AccountPaths, storage::Storage,
};
use std::{collections::BTreeMap, io, path::PathBuf};

/// Enabled only in the checked cutover batch after the full migration gate.
/// Keeping the implementation in source never grants working-profile access.
pub const CUTOVER_READY: bool = false;

pub struct Context {
    pub home: PathBuf,
    pub runtime: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub language: String,
}
impl Context {
    pub fn new(
        home: PathBuf,
        runtime: PathBuf,
        environment: BTreeMap<String, String>,
        language: String,
    ) -> io::Result<Self> {
        Storage::open(&home)?;
        let storage = Storage::open(&runtime)?;
        storage.ensure_dir("omagram/rec")?;
        // Only the session capabilities used by existing fixed-argv helpers.
        // Loader, language-runtime and arbitrary caller variables never survive.
        let mut environment: BTreeMap<_, _> = environment
            .into_iter()
            .filter(|(key, value)| {
                matches!(
                    key.as_str(),
                    "DISPLAY"
                        | "WAYLAND_DISPLAY"
                        | "DBUS_SESSION_BUS_ADDRESS"
                        | "PULSE_SERVER"
                        | "PIPEWIRE_REMOTE"
                        | "XAUTHORITY"
                        | "LANG"
                        | "LC_ALL"
                        | "LC_MESSAGES"
                        | "HYPRLAND_INSTANCE_SIGNATURE"
                ) && value.len() <= 4096
                    && !value.contains('\0')
            })
            .collect();
        environment.insert("HOME".into(), home.to_string_lossy().into_owned());
        environment.insert(
            "XDG_RUNTIME_DIR".into(),
            runtime.to_string_lossy().into_owned(),
        );
        Ok(Self {
            home,
            runtime,
            environment,
            language,
        })
    }
    pub fn configure_application(&self, app: &mut Application) -> io::Result<()> {
        let mut databases = vec![self.home.join(".local/share/omagram/database")];
        for row in crate::accounts::catalog_rows(&app.catalog) {
            let id = row["id"]
                .as_str()
                .ok_or_else(|| io::Error::other("invalid account catalog"))?;
            let paths = AccountPaths::new(&self.home.join(".local/share/omagram"), id)
                .map_err(io::Error::other)?;
            if !databases.contains(&paths.database) {
                databases.push(paths.database);
            }
        }
        app.upload_context = Some((self.home.clone(), databases.clone()));
        app.clipboard_context = Some(crate::clipboard::Context {
            runtime: self.runtime.clone(),
            databases,
            environment: self.environment.clone(),
        });
        app.recording_runtime = Some(self.runtime.clone());
        Ok(())
    }
    pub fn configure_desktop(&self, app: &mut Application, plugin: PathBuf) {
        app.settings.global_context = Some(crate::global_shortcuts::Context {
            plugin,
            environment: self.environment.clone(),
        });
    }
    /// Enable startup only in the final session owner, after acquiring its lock.
    /// Inert consumers deliberately do not call this method or contact keyring.
    pub fn configure_driver(&self, app: &mut Application, driver: &mut Driver) {
        app.keyring_environment = Some(self.environment.clone());
        driver.set_startup_context(crate::startup::Context {
            home: self.home.clone(),
            environment: self.environment.clone(),
            version: env!("CARGO_PKG_VERSION").into(),
            language: self.language.clone(),
        });
        driver.capture_context = Some(crate::capture::Context {
            home: self.home.clone(),
            runtime: self.runtime.clone(),
            environment: self.environment.clone(),
        });
    }
    /// Restore persisted order, active identity and metadata on one native owner.
    /// Registering creates pre-parameter clients; startup owns credentials later.
    pub fn register_catalog(&self, app: &Application, driver: &mut Driver) -> io::Result<()> {
        if !driver.runtime.engine.accounts.is_empty() {
            return Err(io::Error::other("account catalog already registered"));
        }
        for row in crate::accounts::catalog_rows(&app.catalog) {
            let id = row["id"]
                .as_str()
                .ok_or_else(|| io::Error::other("invalid account catalog"))?;
            let paths = AccountPaths::new(&self.home.join(".local/share/omagram"), id)
                .map_err(io::Error::other)?;
            driver.register(
                id.into(),
                row["name"].as_str().unwrap_or(id).into(),
                paths.media_roots(&self.runtime),
            )?;
            let meta = driver
                .runtime
                .metadata
                .get_mut(id)
                .ok_or_else(|| io::Error::other("account registration failed"))?;
            meta.phone = row["phone"].as_str().unwrap_or("").into();
            driver
                .runtime
                .engine
                .accounts
                .get_mut(id)
                .ok_or_else(|| io::Error::other("account registration failed"))?
                .state
                .me_id = crate::projection::integer(&row["meId"], 0);
        }
        let active = app.catalog["active"]
            .as_str()
            .filter(|id| driver.runtime.engine.accounts.contains_key(*id))
            .ok_or_else(|| io::Error::other("unknown active account"))?;
        driver.runtime.engine.active = active.into();
        Ok(())
    }
}
