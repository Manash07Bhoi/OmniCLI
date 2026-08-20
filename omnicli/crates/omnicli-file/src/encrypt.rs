use std::{fs, io::Write, path::Path, str::FromStr};

use age::{x25519, Encryptor};
use anyhow::Result;
use serde::Serialize;

use crate::error::FileError;

#[derive(Debug, Serialize)]
pub struct EncryptResult {
    pub source: String,
    pub output: String,
    pub recipient: String,
    pub bytes_written: u64,
}

/// Encrypt `source` with an age X25519 public key and write the ciphertext to `dest`.
/// `dest` defaults to `<source>.age` if not specified.
pub fn encrypt_file(
    source: &Path,
    dest: Option<&Path>,
    recipient_key: &str,
) -> Result<EncryptResult, FileError> {
    let dest = dest
        .map(|p| p.to_owned())
        .unwrap_or_else(|| source.with_extension("age"));

    let recipient = x25519::Recipient::from_str(recipient_key)
        .map_err(|e| FileError::Encryption(format!("Invalid age public key: {e}")))?;

    let plaintext =
        fs::read(source).map_err(|_| FileError::NotFound(source.display().to_string()))?;

    let encryptor = Encryptor::with_recipients(vec![Box::new(recipient)])
        .ok_or_else(|| FileError::Encryption("no recipients provided".into()))?;

    let mut ciphertext = Vec::new();
    {
        let mut writer = encryptor
            .wrap_output(&mut ciphertext)
            .map_err(|e| FileError::Encryption(e.to_string()))?;
        writer.write_all(&plaintext)?;
        writer
            .finish()
            .map_err(|e| FileError::Encryption(e.to_string()))?;
    }

    fs::write(&dest, &ciphertext)?;
    let bytes_written = ciphertext.len() as u64;

    Ok(EncryptResult {
        source: source.display().to_string(),
        output: dest.display().to_string(),
        recipient: recipient_key.to_owned(),
        bytes_written,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use age::x25519;
    use tempfile::tempdir;

    #[test]
    fn test_encrypt_file() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("secret.txt");
        let out = dir.path().join("secret.txt.age");
        fs::write(&src, b"secret data").unwrap();

        // Generate a fresh X25519 key pair for the test
        let identity = x25519::Identity::generate();
        let public_key = identity.to_public();
        let recipient_str = public_key.to_string();

        let result = encrypt_file(&src, Some(&out), &recipient_str).unwrap();
        assert!(out.exists());
        assert!(result.bytes_written > 0);
        assert_ne!(fs::read(&out).unwrap(), b"secret data");
    }

    #[test]
    fn test_encrypt_invalid_key() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("file.txt");
        fs::write(&src, b"data").unwrap();

        let result = encrypt_file(&src, None, "not-a-valid-key");
        assert!(matches!(result, Err(FileError::Encryption(_))));
    }
}
