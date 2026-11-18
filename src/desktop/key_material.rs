//! Database-key helpers shared by the Windows desktop shell.
//! The key itself is never written here. Callers seal it before it touches disk.

/// What to do when the sealed key file and the database may or may not exist.
#[derive(Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// `db-key.bin` exists. Unseal it and do not mint a new key.
    Unseal,
    /// Neither file exists. Mint a key, seal it, then create the database.
    Create,
    /// The database is present and the sealed key is not. Do not mint a key
    /// and do not replace the database. A copied database file stays unreadable.
    RefuseMissingKey,
}

#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn key_action(key_exists: bool, db_exists: bool) -> KeyAction {
    if key_exists {
        KeyAction::Unseal
    } else if db_exists {
        KeyAction::RefuseMissingKey
    } else {
        KeyAction::Create
    }
}

/// base64url without padding, matching the key the previous Windows app stored.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn encode_key_material(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | (bytes[i + 2] as u32);
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 6) & 63) as usize] as char);
        out.push(ALPHABET[(n & 63) as usize] as char);
        i += 3;
    }
    if i < bytes.len() {
        let rem = bytes.len() - i;
        let mut n = (bytes[i] as u32) << 16;
        if rem == 2 {
            n |= (bytes[i + 1] as u32) << 8;
        }
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        if rem == 2 {
            out.push(ALPHABET[((n >> 6) & 63) as usize] as char);
        }
    }
    out
}

/// True when `blob` contains the raw secret. A sealed key must not.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn blob_contains_secret(blob: &[u8], secret: &str) -> bool {
    let secret = secret.as_bytes();
    !secret.is_empty() && blob.windows(secret.len()).any(|window| window == secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_database_without_a_sealed_key_is_refused() {
        assert_eq!(key_action(false, true), KeyAction::RefuseMissingKey);
        assert_eq!(key_action(false, false), KeyAction::Create);
        assert_eq!(key_action(true, true), KeyAction::Unseal);
        assert_eq!(key_action(true, false), KeyAction::Unseal);
    }

    #[test]
    fn base64url_matches_known_vectors() {
        assert_eq!(encode_key_material(&[0x4d, 0x61, 0x6e]), "TWFu");
        assert_eq!(encode_key_material(&[0x4d, 0x61]), "TWE");
        assert_eq!(encode_key_material(&[0x4d]), "TQ");
        let key = encode_key_material(&[0u8; 32]);
        assert_eq!(key.len(), 43);
        assert!(key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert!(!blob_contains_secret(b"sealed-blob", &key));
        assert!(blob_contains_secret(key.as_bytes(), &key));
    }
}
