//! Existing Secret Service identity. This owner is injected with an explicit
//! home store and environment; pure tests do not contact the real keyring.
use crate::{
    paths::{AccountPaths, valid_account_id},
    process,
    storage::Storage,
};
use base64::{
    Engine,
    alphabet::STANDARD as ALPHABET,
    engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD},
};
fn encode_base64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
    path::Path,
    time::{Duration, Instant},
};
const TIMEOUT: Duration = Duration::from_secs(60);

pub fn valid_credentials(id: &str, hash: &str) -> bool {
    !id.is_empty()
        && id.len() <= 10
        && !id.starts_with('0')
        && id.bytes().all(|c| c.is_ascii_digit())
        && hash.len() == 32
        && hash
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
pub fn valid_key(key: &str) -> bool {
    // The compatible stored decoder accepts nonzero trailing bits. Preserve the
    // existing key representation; never replace a stored key to normalize it.
    GeneralPurpose::new(
        &ALPHABET,
        GeneralPurposeConfig::new().with_decode_allow_trailing_bits(true),
    )
    .decode(key)
    .is_ok_and(|bytes| bytes.len() == 32)
}
pub fn classify_lookup(output: process::Output) -> io::Result<Option<String>> {
    if output.timed_out || output.truncated {
        return Err(io::Error::other("the keyring did not answer"));
    }
    if output.code == Some(0) && !output.stdout.is_empty() {
        return String::from_utf8(output.stdout)
            .map(Some)
            .map_err(|_| io::Error::other("the keyring returned invalid text"));
    }
    if matches!(output.code, Some(0 | 1)) && output.stdout.is_empty() {
        return Ok(None);
    }
    Err(io::Error::other("the keyring refused the lookup"))
}
pub struct CredentialSave {
    environment: BTreeMap<String, String>,
    id: String,
    hash: String,
}
impl std::fmt::Debug for CredentialSave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CredentialSave { credentials: [redacted] }")
    }
}
impl CredentialSave {
    pub fn prepare(
        args: &Value,
        environment: BTreeMap<String, String>,
    ) -> crate::commands::Result<Self> {
        let id = if crate::wire::is_integer(&args["apiId"]) {
            args["apiId"].to_string()
        } else {
            args["apiId"].as_str().unwrap_or("").into()
        };
        let hash = args["apiHash"].as_str().unwrap_or("");
        if !valid_credentials(&id, hash) {
            return Err(
                "the API id is a number and the API hash is 32 hexadecimal characters".into(),
            );
        }
        Ok(Self {
            environment,
            id,
            hash: hash.into(),
        })
    }
    pub fn save_with(
        &self,
        store: &mut dyn FnMut(&str, &str, &str) -> io::Result<bool>,
    ) -> crate::commands::Result<crate::commands::Action> {
        if !store("api_id", "Omagram API id", &self.id).unwrap_or(false)
            || !store("api_hash", "Omagram API hash", &self.hash).unwrap_or(false)
        {
            return Err("the keyring did not accept the credentials".into());
        }
        Ok(crate::commands::Action::CredentialsSaved)
    }
    pub fn run(
        self,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> crate::commands::Result<crate::commands::Action> {
        let keyring = Keyring::until(
            self.environment.clone(),
            Instant::now() + Duration::from_secs(150),
        );
        self.save_with(&mut |key, label, value| keyring.set_cancel(key, label, value, cancel))
    }
}
pub struct Keyring {
    environment: BTreeMap<String, String>,
    deadline: Option<Instant>,
}
impl Keyring {
    pub fn new(environment: BTreeMap<String, String>) -> Self {
        Self {
            environment,
            deadline: None,
        }
    }
    pub fn until(environment: BTreeMap<String, String>, deadline: Instant) -> Self {
        Self {
            environment,
            deadline: Some(deadline),
        }
    }
    fn budget(&self) -> io::Result<Duration> {
        let budget = self.deadline.map_or(TIMEOUT, |deadline| {
            deadline
                .saturating_duration_since(Instant::now())
                .min(TIMEOUT)
        });
        if budget.is_zero() {
            return Err(io::Error::other("the keyring did not answer"));
        }
        Ok(budget)
    }
    pub fn get(&self, key: &str) -> io::Result<Option<String>> {
        self.get_cancel(key, &std::sync::atomic::AtomicBool::new(false))
    }
    pub fn get_cancel(
        &self,
        key: &str,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> io::Result<Option<String>> {
        let output = process::run_cancel(
            "/usr/bin/secret-tool",
            &["lookup", "service", "omagram", "key", key],
            &self.environment,
            &[],
            self.budget()?,
            4096,
            cancel,
        )?;
        classify_lookup(output)
    }
    pub fn set(&self, key: &str, label: &str, value: &str) -> io::Result<bool> {
        self.set_cancel(
            key,
            label,
            value,
            &std::sync::atomic::AtomicBool::new(false),
        )
    }
    pub fn set_cancel(
        &self,
        key: &str,
        label: &str,
        value: &str,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> io::Result<bool> {
        let label = format!("--label={label}");
        Ok(process::run_cancel(
            "/usr/bin/secret-tool",
            &["store", &label, "service", "omagram", "key", key],
            &self.environment,
            value.as_bytes(),
            self.budget()?,
            4096,
            cancel,
        )?
        .ok())
    }
    pub fn credentials(&self) -> io::Result<Option<(i64, String)>> {
        let id = self.get("api_id")?;
        let hash = self.get("api_hash")?;
        match (id, hash) {
            (Some(id), Some(hash)) if valid_credentials(&id, &hash) => {
                Ok(id.parse().ok().map(|id| (id, hash)))
            }
            _ => Ok(None),
        }
    }
    pub fn save_credentials(&self, id: &str, hash: &str) -> io::Result<bool> {
        if !valid_credentials(id, hash) {
            return Ok(false);
        }
        Ok(self.set("api_id", "Omagram API id", id)?
            && self.set("api_hash", "Omagram API hash", hash)?)
    }
    pub fn database_key(&self, id: &str, home: &Storage) -> io::Result<String> {
        database_key_with(
            id,
            home,
            &mut |key| self.get(key),
            &mut |key, label, value| self.set(key, label, value),
        )
    }
}
pub fn database_key_with(
    id: &str,
    home: &Storage,
    get: &mut dyn FnMut(&str) -> io::Result<Option<String>>,
    set: &mut dyn FnMut(&str, &str, &str) -> io::Result<bool>,
) -> io::Result<String> {
    if !valid_account_id(id) {
        return Err(io::Error::other("invalid account id"));
    }
    let name = if id == "default" {
        "database_key".into()
    } else {
        format!("database_key_{id}")
    };
    if let Some(key) = get(&name)? {
        if !valid_key(&key) {
            return Err(io::Error::other(
                "the database key in the keyring is malformed",
            ));
        }
        return Ok(key);
    }
    let directory = if id == "default" {
        ".local/share/omagram/database".into()
    } else {
        format!(".local/share/omagram/accounts/{id}/database")
    };
    if home.nonempty_directory(&directory)? {
        return Err(io::Error::other(
            "the database key is missing from the keyring",
        ));
    }
    let mut entropy = [0u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut entropy)?;
    let key = encode_base64(&entropy);
    let label = if id == "default" {
        "Omagram database key".into()
    } else {
        format!("Omagram database key ({id})")
    };
    if !set(&name, &label, &key)? {
        return Err(io::Error::other(
            "the database key could not be stored in the keyring",
        ));
    }
    Ok(key)
}
pub fn parameters(
    home: &Path,
    id: &str,
    api_id: i64,
    api_hash: &str,
    key: &str,
    version: &str,
    language: &str,
) -> io::Result<Value> {
    let paths =
        AccountPaths::new(&home.join(".local/share/omagram"), id).map_err(io::Error::other)?;
    Ok(
        json!({"@type":"setTdlibParameters","use_test_dc":false,"database_directory":paths.database,"files_directory":paths.files,"database_encryption_key":key,"use_file_database":true,"use_chat_info_database":true,"use_message_database":true,"use_secret_chats":true,"api_id":api_id,"api_hash":api_hash,"system_language_code":language,"device_model":"Omarchy","system_version":"","application_version":version}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn output(code: i32, bytes: &[u8]) -> process::Output {
        process::Output {
            code: Some(code),
            stdout: bytes.into(),
            stderr: Vec::new(),
            timed_out: false,
            truncated: false,
        }
    }
    #[test]
    fn absence_is_not_a_transient_failure_or_permission_to_replace_key() {
        assert!(classify_lookup(output(1, b"")).unwrap().is_none());
        assert_eq!(
            classify_lookup(output(0, b"synthetic")).unwrap(),
            Some("synthetic".into())
        );
        assert!(classify_lookup(output(2, b"")).is_err());
        let mut timeout = output(0, b"");
        timeout.timed_out = true;
        assert!(classify_lookup(timeout).is_err());
        assert!(valid_credentials("123", "0123456789abcdef0123456789abcdef"));
        assert!(!valid_credentials(
            "0123",
            "0123456789abcdef0123456789abcdef"
        ));
        assert!(valid_key(&encode_base64(&[42; 32])));
        assert!(!valid_key("bad"));
    }
    #[test]
    fn parameter_identity_is_unchanged() {
        let p = parameters(
            Path::new("/fixture/home"),
            "default",
            123,
            "synthetic",
            "synthetic",
            "fixture",
            "ru-RU",
        )
        .unwrap();
        assert_eq!(
            p["database_directory"],
            "/fixture/home/.local/share/omagram/database"
        );
        assert_eq!(p["device_model"], "Omarchy");
        assert_eq!(p["use_secret_chats"], true);
    }
}
