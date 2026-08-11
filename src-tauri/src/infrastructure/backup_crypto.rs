//! Audited age encryption and operating-system credential-store adapters for backups.
//!
//! Secret identity material stays behind [`SecretBytes`]. This module never formats,
//! logs, or includes a secret in an error message.

use std::{fmt, str};

use age::{
    scrypt,
    secrecy::{ExposeSecret, SecretString},
    x25519,
};

use crate::workspace_features::backup::{AgeBackend, KeychainIdentityProvider, SecretBytes};

const KEYRING_SERVICE: &str = "com.clearders.testpapers.desktop.backup";
const KEYRING_ACCOUNT_PREFIX: &str = "age-x25519-v1:";
const MAX_KEY_ID_LEN: usize = 128;

/// Production implementation of the backup encryption boundary.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AuditedAgeBackend;

impl AuditedAgeBackend {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl AgeBackend for AuditedAgeBackend {
    fn encrypt_with_passphrase(
        &self,
        plaintext: &[u8],
        passphrase: &SecretBytes,
    ) -> Result<Vec<u8>, String> {
        let recipient = scrypt::Recipient::new(to_age_secret(passphrase, "passphrase")?);
        age::encrypt(&recipient, plaintext).map_err(|error| error.to_string())
    }

    fn decrypt_with_passphrase(
        &self,
        ciphertext: &[u8],
        passphrase: &SecretBytes,
    ) -> Result<Vec<u8>, String> {
        let identity = scrypt::Identity::new(to_age_secret(passphrase, "passphrase")?);
        age::decrypt(&identity, ciphertext).map_err(|error| error.to_string())
    }

    fn encrypt_for_recipient(&self, plaintext: &[u8], recipient: &str) -> Result<Vec<u8>, String> {
        let recipient = recipient
            .parse::<x25519::Recipient>()
            .map_err(|_| "age recipient is invalid".to_owned())?;
        age::encrypt(&recipient, plaintext).map_err(|error| error.to_string())
    }

    fn decrypt_with_identity(
        &self,
        ciphertext: &[u8],
        identity: &SecretBytes,
    ) -> Result<Vec<u8>, String> {
        let identity = parse_identity(identity).map_err(|error| error.to_string())?;
        age::decrypt(&identity, ciphertext).map_err(|error| error.to_string())
    }
}

fn to_age_secret(secret: &SecretBytes, kind: &str) -> Result<SecretString, String> {
    let text = str::from_utf8(secret.expose())
        .map_err(|_| format!("backup {kind} must be valid UTF-8"))?;
    Ok(SecretString::from(text.to_owned()))
}

/// Minimal secure-store boundary. Keeping this injectable prevents unit tests from touching the
/// user's real OS credential store.
pub(crate) trait BackupIdentityStore: Send + Sync {
    fn load(&self, account: &str) -> Result<Option<Vec<u8>>, String>;
    fn save(&self, account: &str, secret: &[u8]) -> Result<(), String>;
}

/// Native Windows Credential Manager / macOS Keychain / Linux Secret Service storage.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct OsBackupIdentityStore;

impl BackupIdentityStore for OsBackupIdentityStore {
    fn load(&self, account: &str) -> Result<Option<Vec<u8>>, String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, account)
            .map_err(|error| format!("could not open the OS credential store: {error}"))?;
        match entry.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(format!("could not read the OS credential store: {error}")),
        }
    }

    fn save(&self, account: &str, secret: &[u8]) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, account)
            .map_err(|error| format!("could not open the OS credential store: {error}"))?;
        entry
            .set_secret(secret)
            .map_err(|error| format!("could not update the OS credential store: {error}"))
    }
}

/// X25519 identity lifecycle backed by an injectable credential store.
pub(crate) struct KeyringAgeIdentityProvider<S = OsBackupIdentityStore> {
    store: S,
    operation_lock: std::sync::Mutex<()>,
}

impl KeyringAgeIdentityProvider<OsBackupIdentityStore> {
    pub(crate) fn production() -> Self {
        Self::with_store(OsBackupIdentityStore)
    }
}

impl<S: BackupIdentityStore> KeyringAgeIdentityProvider<S> {
    pub(crate) fn with_store(store: S) -> Self {
        Self {
            store,
            operation_lock: std::sync::Mutex::new(()),
        }
    }

    /// Loads the existing identity or generates and saves a new X25519 identity, returning its
    /// public age recipient. This is the setup path for encrypted scheduled backups.
    pub(crate) fn load_or_create_recipient(
        &self,
        key_id: &str,
    ) -> Result<String, BackupCryptoError> {
        let account = account_for(key_id)?;
        let _guard = self
            .operation_lock
            .lock()
            .map_err(|_| BackupCryptoError::OperationUnavailable)?;

        match self.load_secret(&account)? {
            Some(secret) => recipient_from_secret(&secret),
            None => {
                let identity = x25519::Identity::generate();
                let recipient = identity.to_public().to_string();
                let secret = encode_identity(&identity)?;
                self.store
                    .save(&account, secret.expose())
                    .map_err(BackupCryptoError::CredentialStore)?;
                Ok(recipient)
            }
        }
    }

    /// Returns the public recipient for an existing key without creating replacement material.
    pub(crate) fn recipient(&self, key_id: &str) -> Result<String, BackupCryptoError> {
        let secret = self.require_identity(key_id)?;
        recipient_from_secret(&secret)
    }

    /// Returns the canonical age X25519 identity for an explicit recovery-key export flow.
    /// Callers must keep the result in secret-aware UI and storage paths.
    pub(crate) fn export_recovery_identity(
        &self,
        key_id: &str,
    ) -> Result<SecretBytes, BackupCryptoError> {
        let secret = self.require_identity(key_id)?;
        let identity = parse_identity(&secret)?;
        encode_identity(&identity)
    }

    /// Validates and saves an exported age X25519 identity, returning its public recipient.
    /// Existing material for `key_id` is deliberately replaced so a recovery import can restore
    /// access after the OS credential store has been lost.
    #[allow(dead_code)] // Reserved for the explicit recovery-key import flow after restore.
    pub(crate) fn import_recovery_identity(
        &self,
        key_id: &str,
        recovery_identity: &SecretBytes,
    ) -> Result<String, BackupCryptoError> {
        let account = account_for(key_id)?;
        let identity = parse_identity(recovery_identity)
            .map_err(|_| BackupCryptoError::InvalidRecoveryIdentity)?;
        let recipient = identity.to_public().to_string();
        let canonical = encode_identity(&identity)?;
        let _guard = self
            .operation_lock
            .lock()
            .map_err(|_| BackupCryptoError::OperationUnavailable)?;
        self.store
            .save(&account, canonical.expose())
            .map_err(BackupCryptoError::CredentialStore)?;
        Ok(recipient)
    }

    fn require_identity(&self, key_id: &str) -> Result<SecretBytes, BackupCryptoError> {
        let account = account_for(key_id)?;
        self.load_secret(&account)?
            .ok_or(BackupCryptoError::IdentityMissing)
    }

    fn load_secret(&self, account: &str) -> Result<Option<SecretBytes>, BackupCryptoError> {
        self.store
            .load(account)
            .map_err(BackupCryptoError::CredentialStore)?
            .map(|secret| {
                SecretBytes::new(secret).map_err(|_| BackupCryptoError::InvalidStoredIdentity)
            })
            .transpose()
    }
}

impl<S: BackupIdentityStore> KeychainIdentityProvider for KeyringAgeIdentityProvider<S> {
    fn identity(&self, key_id: &str) -> Result<SecretBytes, String> {
        let secret = self
            .require_identity(key_id)
            .map_err(|error| error.to_string())?;
        parse_identity(&secret).map_err(|error| error.to_string())?;
        Ok(secret)
    }
}

fn account_for(key_id: &str) -> Result<String, BackupCryptoError> {
    if key_id.is_empty()
        || key_id.len() > MAX_KEY_ID_LEN
        || !key_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(BackupCryptoError::InvalidKeyId);
    }
    Ok(format!("{KEYRING_ACCOUNT_PREFIX}{key_id}"))
}

fn parse_identity(secret: &SecretBytes) -> Result<x25519::Identity, BackupCryptoError> {
    let text =
        str::from_utf8(secret.expose()).map_err(|_| BackupCryptoError::InvalidStoredIdentity)?;
    text.trim()
        .parse::<x25519::Identity>()
        .map_err(|_| BackupCryptoError::InvalidStoredIdentity)
}

fn encode_identity(identity: &x25519::Identity) -> Result<SecretBytes, BackupCryptoError> {
    let encoded = identity.to_string();
    SecretBytes::new(encoded.expose_secret().as_bytes().to_vec())
        .map_err(|_| BackupCryptoError::InvalidStoredIdentity)
}

fn recipient_from_secret(secret: &SecretBytes) -> Result<String, BackupCryptoError> {
    Ok(parse_identity(secret)?.to_public().to_string())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BackupCryptoError {
    InvalidKeyId,
    IdentityMissing,
    InvalidStoredIdentity,
    InvalidRecoveryIdentity,
    CredentialStore(String),
    OperationUnavailable,
}

impl fmt::Display for BackupCryptoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKeyId => formatter.write_str("backup key ID is invalid"),
            Self::IdentityMissing => {
                formatter.write_str("backup identity is not in the OS credential store")
            }
            Self::InvalidStoredIdentity => {
                formatter.write_str("backup identity in the OS credential store is invalid")
            }
            Self::InvalidRecoveryIdentity => {
                formatter.write_str("recovery identity is not a valid age X25519 identity")
            }
            Self::CredentialStore(message) => write!(formatter, "{message}"),
            Self::OperationUnavailable => {
                formatter.write_str("backup identity operation is temporarily unavailable")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use super::*;
    use crate::workspace_features::backup::{BackupEncryption, UnlockMaterial};

    #[derive(Default)]
    struct MemoryIdentityStore {
        secrets: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl BackupIdentityStore for MemoryIdentityStore {
        fn load(&self, account: &str) -> Result<Option<Vec<u8>>, String> {
            Ok(self.secrets.lock().unwrap().get(account).cloned())
        }

        fn save(&self, account: &str, secret: &[u8]) -> Result<(), String> {
            self.secrets
                .lock()
                .unwrap()
                .insert(account.to_owned(), secret.to_vec());
            Ok(())
        }
    }

    #[test]
    fn age_passphrase_round_trip_uses_real_age_format() {
        let backend = AuditedAgeBackend::new();
        let passphrase = SecretBytes::new(b"correct horse battery staple".to_vec()).unwrap();

        let encrypted = BackupEncryption::Passphrase(&passphrase)
            .encrypt(b"consistent backup", &backend)
            .unwrap();
        assert!(encrypted.starts_with(b"age-encryption.org/v1"));
        assert_ne!(encrypted, b"consistent backup");

        let decrypted = UnlockMaterial::Passphrase(&passphrase)
            .decrypt(&encrypted, &backend)
            .unwrap();
        assert_eq!(decrypted, b"consistent backup");
    }

    #[test]
    fn generated_keychain_identity_round_trips_without_real_keychain() {
        let backend = AuditedAgeBackend::new();
        let provider = KeyringAgeIdentityProvider::with_store(MemoryIdentityStore::default());
        let recipient = provider.load_or_create_recipient("workspace-0198").unwrap();

        let encrypted = BackupEncryption::Recipient {
            recipient: &recipient,
            key_id: "workspace-0198",
        }
        .encrypt(b"scheduled backup", &backend)
        .unwrap();
        let decrypted = UnlockMaterial::Keychain {
            key_id: "workspace-0198",
            provider: &provider,
        }
        .decrypt(&encrypted, &backend)
        .unwrap();

        assert_eq!(decrypted, b"scheduled backup");
        assert_eq!(provider.recipient("workspace-0198").unwrap(), recipient);
    }

    #[test]
    fn exported_identity_can_recover_into_a_fresh_store() {
        let original = KeyringAgeIdentityProvider::with_store(MemoryIdentityStore::default());
        let recipient = original.load_or_create_recipient("primary").unwrap();
        let recovery = original.export_recovery_identity("primary").unwrap();
        assert_eq!(format!("{recovery:?}"), "SecretBytes([REDACTED])");

        let recovered = KeyringAgeIdentityProvider::with_store(MemoryIdentityStore::default());
        let imported_recipient = recovered
            .import_recovery_identity("primary", &recovery)
            .unwrap();

        assert_eq!(imported_recipient, recipient);
        assert_eq!(recovered.recipient("primary").unwrap(), recipient);
    }

    #[test]
    fn malformed_identifiers_and_recovery_material_are_rejected() {
        let provider = KeyringAgeIdentityProvider::with_store(MemoryIdentityStore::default());
        assert_eq!(
            provider.load_or_create_recipient("../outside"),
            Err(BackupCryptoError::InvalidKeyId)
        );
        let invalid = SecretBytes::new(b"not-an-age-identity".to_vec()).unwrap();
        assert_eq!(
            provider.import_recovery_identity("primary", &invalid),
            Err(BackupCryptoError::InvalidRecoveryIdentity)
        );
    }
}
