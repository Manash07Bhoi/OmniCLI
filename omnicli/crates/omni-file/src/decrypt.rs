use std::{io::Read, path::Path, str::FromStr};

use age::{x25519, Decryptor};
use serde::Serialize;

use crate::error::FileError;

#[derive(Debug, Serialize)]
pub struct DecryptResult {
    pub source: String,
    pub output: String,
    pub bytes_written: u64,
}

/// Decrypt an age X25519-encrypted file using the given private identity key.
///
/// `identity_str` must be an `AGE-SECRET-KEY-…` string.
/// `dest` defaults to stripping the `.age` extension; falls back to `<source>.dec`.
pub fn decrypt_file(
    source: &Path,
    dest: Option<&Path>,
    identity_str: &str,
) -> Result<DecryptResult, FileError> {
    let dest = dest.map(|p| p.to_owned()).unwrap_or_else(|| {
        let name = source.to_string_lossy();
        if name.ends_with(".age") {
            // strip .age → restores original extension
            source.with_extension("").to_owned()
        } else {
            source.with_extension("dec").to_owned()
        }
    });

    let identity = x25519::Identity::from_str(identity_str)
        .map_err(|e| FileError::Encryption(format!("Invalid age identity key: {e}")))?;

    let file = std::fs::File::open(source)
        .map_err(|_| FileError::NotFound(source.display().to_string()))?;

    let decryptor = Decryptor::new(file)
        .map_err(|e| FileError::Encryption(format!("Failed to parse age header: {e}")))?;

    let mut plaintext = Vec::new();
    match decryptor {
        Decryptor::Recipients(d) => {
            let ids: &[&dyn age::Identity] = &[&identity];
            let mut reader = d
                .decrypt(ids.iter().copied())
                .map_err(|e| FileError::Encryption(format!("Decryption failed: {e}")))?;
            reader.read_to_end(&mut plaintext)?;
        }
        Decryptor::Passphrase(_) => {
            return Err(FileError::Encryption(
                "Passphrase-encrypted files are not supported. \
                 Use a recipient key (AGE-SECRET-KEY-…)."
                    .into(),
            ));
        }
    }

    std::fs::write(&dest, &plaintext)?;
    let bytes_written = plaintext.len() as u64;

    Ok(DecryptResult {
        source: source.display().to_string(),
        output: dest.display().to_string(),
        bytes_written,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encrypt::encrypt_file;
    use age::secrecy::ExposeSecret;
    use tempfile::tempdir;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("secret.txt");
        let enc = dir.path().join("secret.txt.age");
        let dec = dir.path().join("secret.txt.dec");

        let payload = b"super secret data 1234";
        std::fs::write(&src, payload).unwrap();

        // Generate a fresh X25519 key pair
        let identity = x25519::Identity::generate();
        let public_key = identity.to_public().to_string();
        // In age 0.10 the private key is wrapped in Secret<String> to prevent logging.
        let private_key = identity.to_string();
        let private_key_str = private_key.expose_secret();

        encrypt_file(&src, Some(&enc), &public_key).unwrap();
        assert!(enc.exists());
        assert_ne!(std::fs::read(&enc).unwrap(), payload);

        let result = decrypt_file(&enc, Some(&dec), private_key_str).unwrap();
        assert_eq!(std::fs::read(&dec).unwrap(), payload);
        assert_eq!(result.bytes_written, payload.len() as u64);
    }

    #[test]
    fn test_decrypt_invalid_key() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("file.age");
        std::fs::write(&src, b"not real age data").unwrap();

        let result = decrypt_file(&src, None, "AGE-SECRET-KEY-INVALID");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_not_found() {
        let dir = tempdir().unwrap();
        let identity = x25519::Identity::generate();
        let priv_key = identity.to_string();
        let result = decrypt_file(
            &dir.path().join("ghost.age"),
            None,
            priv_key.expose_secret(),
        );
        assert!(matches!(result, Err(FileError::NotFound(_))));
    }
}
