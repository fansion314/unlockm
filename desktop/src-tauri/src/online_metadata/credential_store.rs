use crate::APP_IDENTIFIER;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const SERVICE: &str = APP_IDENTIFIER;
const ACCOUNT: &str = "online-metadata.spotify.default";
const VERSION: u8 = 1;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCredential {
    version: u8,
    refresh_token: String,
    issued_at_ms: u64,
    client_id_fingerprint: String,
}

impl std::fmt::Debug for StoredCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StoredCredential")
            .field("version", &self.version)
            .field("refresh_token", &"[REDACTED]")
            .field("issued_at_ms", &self.issued_at_ms)
            .field("client_id_fingerprint", &self.client_id_fingerprint)
            .finish()
    }
}

impl StoredCredential {
    pub fn new(refresh_token: String, issued_at_ms: u64, client_id: &str) -> Self {
        Self {
            version: VERSION,
            refresh_token,
            issued_at_ms,
            client_id_fingerprint: client_id_fingerprint(client_id),
        }
    }

    pub fn refresh_token(&self) -> &str {
        &self.refresh_token
    }
    pub fn issued_at_ms(&self) -> u64 {
        self.issued_at_ms
    }
    pub fn matches_client_id(&self, client_id: &str) -> bool {
        self.client_id_fingerprint == client_id_fingerprint(client_id)
    }
}

pub trait CredentialStore: Send + Sync {
    fn load(&self) -> Result<Option<StoredCredential>, CredentialError>;
    fn save(&self, credential: &StoredCredential) -> Result<(), CredentialError>;
    fn delete(&self) -> Result<(), CredentialError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    Unavailable,
    Invalid,
}

/// Native Keychain on macOS and Credential Manager on Windows through
/// `keyring`; failures deliberately have no file/database fallback.
#[derive(Default)]
pub struct SystemCredentialStore;

impl SystemCredentialStore {
    fn entry(&self) -> Result<keyring::Entry, CredentialError> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|_| CredentialError::Unavailable)
    }
}

impl CredentialStore for SystemCredentialStore {
    fn load(&self) -> Result<Option<StoredCredential>, CredentialError> {
        match self.entry()?.get_password() {
            Ok(raw) => {
                let credential: StoredCredential =
                    serde_json::from_str(&raw).map_err(|_| CredentialError::Invalid)?;
                if credential.version != VERSION || credential.refresh_token.is_empty() {
                    return Err(CredentialError::Invalid);
                }
                Ok(Some(credential))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(CredentialError::Unavailable),
        }
    }

    fn save(&self, credential: &StoredCredential) -> Result<(), CredentialError> {
        let value = serde_json::to_string(credential).map_err(|_| CredentialError::Invalid)?;
        self.entry()?
            .set_password(&value)
            .map_err(|_| CredentialError::Unavailable)
    }

    fn delete(&self) -> Result<(), CredentialError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(CredentialError::Unavailable),
        }
    }
}

pub fn system_store() -> Arc<dyn CredentialStore> {
    Arc::new(SystemCredentialStore)
}

fn client_id_fingerprint(client_id: &str) -> String {
    hex::encode(Sha256::digest(client_id.as_bytes()))
}

#[cfg(test)]
pub struct MemoryCredentialStore(pub std::sync::Mutex<Option<StoredCredential>>);
#[cfg(test)]
impl CredentialStore for MemoryCredentialStore {
    fn load(&self) -> Result<Option<StoredCredential>, CredentialError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, value: &StoredCredential) -> Result<(), CredentialError> {
        *self.0.lock().unwrap() = Some(value.clone());
        Ok(())
    }
    fn delete(&self) -> Result<(), CredentialError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(test)]
pub struct FailingCredentialStore;
#[cfg(test)]
impl CredentialStore for FailingCredentialStore {
    fn load(&self) -> Result<Option<StoredCredential>, CredentialError> {
        Err(CredentialError::Unavailable)
    }
    fn save(&self, _: &StoredCredential) -> Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }
    fn delete(&self) -> Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stored_secret_is_versioned_and_bound_to_the_public_client_id() {
        let value = StoredCredential::new("refresh-token".into(), 1, "client-a");
        assert!(value.matches_client_id("client-a"));
        assert!(!value.matches_client_id("client-b"));
        assert!(!serde_json::to_string(&value).unwrap().contains("client-a"));
        assert!(!format!("{value:?}").contains("refresh-token"));
    }
}
