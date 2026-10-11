//! Parameter preparation over explicit storage and Secret Service context.
//! This worker never loads TDLib or opens an account database.
use crate::{
    keyring::{self, Keyring},
    storage::Storage,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io,
    path::PathBuf,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
#[derive(Clone, Debug)]
pub struct Context {
    pub home: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub version: String,
    pub language: String,
}
pub struct Prepared(pub Result<Option<Value>, String>);
impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Prepared { parameters: [redacted] }")
    }
}
impl Context {
    pub fn prepare_with(
        &self,
        account: &str,
        get: &mut dyn FnMut(&str) -> io::Result<Option<String>>,
        set: &mut dyn FnMut(&str, &str, &str) -> io::Result<bool>,
    ) -> Prepared {
        let result = (|| {
            if !crate::paths::valid_account_id(account) {
                return Err(io::Error::other("invalid account id"));
            }
            let home = Storage::open(&self.home)?;
            let id = get("api_id")?;
            let hash = get("api_hash")?;
            let (Some(id), Some(hash)) = (id, hash) else {
                return Ok(None);
            };
            if !keyring::valid_credentials(&id, &hash) {
                return Ok(None);
            }
            let key = keyring::database_key_with(account, &home, get, set)?;
            let api_id = id
                .parse::<i64>()
                .map_err(|_| io::Error::other("invalid API id"))?;
            keyring::parameters(
                &self.home,
                account,
                api_id,
                &hash,
                &key,
                &self.version,
                &self.language,
            )
            .map(Some)
        })();
        // Never serialize native/helper exceptions or Secret Service payloads.
        Prepared(result.map_err(|error| match error.to_string().as_str(){
            "the database key in the keyring is malformed"=>"the database key in the keyring is malformed".into(),
            "the database key is missing from the keyring"=>"the database key is missing from the keyring".into(),
            "the database key could not be stored in the keyring"=>"the database key could not be stored in the keyring".into(),
            _=>"the startup credentials could not be prepared; check the keyring and account storage".into(),
        }))
    }
    pub fn run(&self, account: &str, cancel: &AtomicBool) -> Prepared {
        let keyring = Keyring::until(
            self.environment.clone(),
            Instant::now() + Duration::from_secs(150),
        );
        self.prepare_with(
            account,
            &mut |key| keyring.get_cancel(key, cancel),
            &mut |key, label, value| keyring.set_cancel(key, label, value, cancel),
        )
    }
}
