use aegis_common::AegisError;
use thiserror::Error;

/// Error type for vault operations.
#[derive(Error, Debug)]
pub enum VaultError {
    #[error("vault is sealed")]
    Sealed,

    #[error("secret not found: {0}")]
    SecretNotFound(String),

    #[error("access denied: {0}")]
    AccessDenied(String),

    #[error("encryption error: {0}")]
    Encryption(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid passphrase")]
    InvalidPassphrase,

    #[error("secret already exists: {0}")]
    AlreadyExists(String),

    #[error("vault error: {0}")]
    Other(String),
}

impl From<VaultError> for AegisError {
    fn from(err: VaultError) -> Self {
        AegisError::Vault(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_error_display() {
        let err = VaultError::Sealed;
        assert_eq!(err.to_string(), "vault is sealed");

        let err = VaultError::SecretNotFound("db_password".into());
        assert_eq!(err.to_string(), "secret not found: db_password");

        let err = VaultError::InvalidPassphrase;
        assert_eq!(err.to_string(), "invalid passphrase");
    }

    #[test]
    fn test_vault_error_to_aegis_error() {
        let vault_err = VaultError::Sealed;
        let aegis_err: AegisError = vault_err.into();
        assert!(matches!(aegis_err, AegisError::Vault(_)));
        assert_eq!(aegis_err.to_string(), "vault error: vault is sealed");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let vault_err = VaultError::from(io_err);
        assert!(matches!(vault_err, VaultError::Io(_)));
    }
}
