//! `Secret`: a string that never prints.
//!
//! Every API key, broker token, OAuth code, password and TOTP value that
//! passes through Rust is held in this type. `Debug` and `Display` print
//! `[REDACTED]`, the buffer is zeroed on drop, and the plaintext is only
//! reachable through an explicit `expose()` call that is easy to grep for.

use serde::{Deserialize, Deserializer};
use std::fmt;
use zeroize::Zeroize;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Secret(value.into())
    }

    /// The plaintext. Call sites are the audit surface for secret handling.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl From<String> for Secret {
    fn from(s: String) -> Self {
        Secret(s)
    }
}

impl From<&str> for Secret {
    fn from(s: &str) -> Self {
        Secret(s.to_string())
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Secret)
    }
}

/// Raw key bytes with the same guarantees.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretBytes(Vec<u8>);

impl SecretBytes {
    pub fn new(bytes: Vec<u8>) -> Self {
        SecretBytes(bytes)
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_are_redacted() {
        let s = Secret::new("api-key-123");
        assert_eq!(format!("{:?}", s), "[REDACTED]");
        assert_eq!(format!("{}", s), "[REDACTED]");
        assert_eq!(s.expose(), "api-key-123");

        #[derive(Debug)]
        #[allow(dead_code)]
        struct Holder {
            token: Secret,
        }
        let h = Holder {
            token: Secret::new("tok"),
        };
        assert!(!format!("{:?}", h).contains("tok\""));
    }

    #[test]
    fn deserializes_transparently() {
        let s: Secret = serde_json::from_str("\"abc\"").unwrap();
        assert_eq!(s.expose(), "abc");
    }
}
