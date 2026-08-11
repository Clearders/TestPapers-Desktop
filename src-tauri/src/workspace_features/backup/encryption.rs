//! Encryption boundary. Production adapters should use the audited `age` and OS-keychain crates;
//! no home-grown cipher or reversible fallback is permitted.

use std::fmt;

const AGE_HEADER_PREFIX: &[u8] = b"age-encryption.org/v1";

pub(crate) struct SecretBytes(Vec<u8>);

impl SecretBytes {
    pub(crate) fn new(bytes: Vec<u8>) -> Result<Self, BackupEncryptionError> {
        if bytes.is_empty() {
            return Err(BackupEncryptionError::MissingSecret);
        }
        Ok(Self(bytes))
    }

    pub(crate) fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretBytes([REDACTED])")
    }
}

pub(crate) trait AgeBackend: Send + Sync {
    fn encrypt_with_passphrase(
        &self,
        plaintext: &[u8],
        passphrase: &SecretBytes,
    ) -> Result<Vec<u8>, String>;

    fn decrypt_with_passphrase(
        &self,
        ciphertext: &[u8],
        passphrase: &SecretBytes,
    ) -> Result<Vec<u8>, String>;

    fn encrypt_for_recipient(&self, plaintext: &[u8], recipient: &str) -> Result<Vec<u8>, String>;

    fn decrypt_with_identity(
        &self,
        ciphertext: &[u8],
        identity: &SecretBytes,
    ) -> Result<Vec<u8>, String>;
}

pub(crate) trait KeychainIdentityProvider: Send + Sync {
    fn identity(&self, key_id: &str) -> Result<SecretBytes, String>;
}

pub(crate) enum BackupEncryption<'a> {
    Plaintext,
    Passphrase(&'a SecretBytes),
    Recipient { recipient: &'a str, key_id: &'a str },
}

pub(crate) enum UnlockMaterial<'a> {
    Passphrase(&'a SecretBytes),
    Identity(&'a SecretBytes),
    Keychain {
        key_id: &'a str,
        provider: &'a dyn KeychainIdentityProvider,
    },
}

impl BackupEncryption<'_> {
    pub(crate) fn encrypt(
        &self,
        plaintext_archive: &[u8],
        backend: &dyn AgeBackend,
    ) -> Result<Vec<u8>, BackupEncryptionError> {
        match self {
            Self::Plaintext => Ok(plaintext_archive.to_vec()),
            Self::Passphrase(passphrase) => backend
                .encrypt_with_passphrase(plaintext_archive, passphrase)
                .map_err(BackupEncryptionError::Backend),
            Self::Recipient { recipient, key_id } => {
                if recipient.trim().is_empty() || key_id.trim().is_empty() {
                    return Err(BackupEncryptionError::InvalidRecipientMetadata);
                }
                backend
                    .encrypt_for_recipient(plaintext_archive, recipient)
                    .map_err(BackupEncryptionError::Backend)
            }
        }
    }
}

impl UnlockMaterial<'_> {
    pub(crate) fn decrypt(
        &self,
        archive: &[u8],
        backend: &dyn AgeBackend,
    ) -> Result<Vec<u8>, BackupEncryptionError> {
        if !archive.starts_with(AGE_HEADER_PREFIX) {
            return Err(BackupEncryptionError::NotAgeArchive);
        }
        match self {
            Self::Passphrase(passphrase) => backend
                .decrypt_with_passphrase(archive, passphrase)
                .map_err(BackupEncryptionError::Backend),
            Self::Identity(identity) => backend
                .decrypt_with_identity(archive, identity)
                .map_err(BackupEncryptionError::Backend),
            Self::Keychain { key_id, provider } => {
                let identity = provider
                    .identity(key_id)
                    .map_err(BackupEncryptionError::Keychain)?;
                backend
                    .decrypt_with_identity(archive, &identity)
                    .map_err(BackupEncryptionError::Backend)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BackupEncryptionError {
    MissingSecret,
    InvalidRecipientMetadata,
    NotAgeArchive,
    Backend(String),
    Keychain(String),
}

impl fmt::Display for BackupEncryptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSecret => formatter.write_str("backup secret must not be empty"),
            Self::InvalidRecipientMetadata => {
                formatter.write_str("age recipient metadata is invalid")
            }
            Self::NotAgeArchive => formatter.write_str("encrypted backup has no age header"),
            Self::Backend(message) => write!(formatter, "backup encryption failed: {message}"),
            Self::Keychain(message) => {
                write!(formatter, "backup identity is unavailable: {message}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeAge;

    impl AgeBackend for FakeAge {
        fn encrypt_with_passphrase(
            &self,
            plaintext: &[u8],
            _: &SecretBytes,
        ) -> Result<Vec<u8>, String> {
            let mut result = AGE_HEADER_PREFIX.to_vec();
            result.extend(plaintext.iter().map(|byte| byte ^ 0x55));
            Ok(result)
        }

        fn decrypt_with_passphrase(
            &self,
            ciphertext: &[u8],
            _: &SecretBytes,
        ) -> Result<Vec<u8>, String> {
            Ok(ciphertext[AGE_HEADER_PREFIX.len()..]
                .iter()
                .map(|byte| byte ^ 0x55)
                .collect())
        }

        fn encrypt_for_recipient(&self, plaintext: &[u8], _: &str) -> Result<Vec<u8>, String> {
            self.encrypt_with_passphrase(plaintext, &SecretBytes(vec![1]))
        }

        fn decrypt_with_identity(
            &self,
            ciphertext: &[u8],
            _: &SecretBytes,
        ) -> Result<Vec<u8>, String> {
            self.decrypt_with_passphrase(ciphertext, &SecretBytes(vec![1]))
        }
    }

    #[test]
    fn passphrase_boundary_round_trips_without_exposing_secret() {
        let secret = SecretBytes::new(b"correct horse".to_vec()).unwrap();
        assert_eq!(format!("{secret:?}"), "SecretBytes([REDACTED])");
        let encrypted = BackupEncryption::Passphrase(&secret)
            .encrypt(b"backup", &FakeAge)
            .unwrap();
        let decrypted = UnlockMaterial::Passphrase(&secret)
            .decrypt(&encrypted, &FakeAge)
            .unwrap();
        assert_eq!(decrypted, b"backup");
    }
}
