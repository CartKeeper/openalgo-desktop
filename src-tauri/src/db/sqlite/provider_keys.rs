//! Provider API key storage (encrypted)

use crate::error::Result;
use crate::security::SecurityManager;
use rusqlite::Connection;

/// Save an encrypted provider API key
pub fn save_provider_key(
    conn: &Connection,
    provider: &str,
    api_key: &str,
    security: &SecurityManager,
) -> Result<()> {
    let (encrypted, nonce) = security.encrypt(api_key)?;
    conn.execute(
        "INSERT OR REPLACE INTO provider_api_keys (provider, api_key_encrypted, api_key_nonce, updated_at)
         VALUES (?1, ?2, ?3, datetime('now'))",
        rusqlite::params![provider, encrypted, nonce],
    )?;
    Ok(())
}

/// Get a decrypted provider API key
pub fn get_provider_key(
    conn: &Connection,
    provider: &str,
    security: &SecurityManager,
) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT api_key_encrypted, api_key_nonce FROM provider_api_keys WHERE provider = ?1",
    )?;

    let result = stmt.query_row([provider], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    });

    match result {
        Ok((encrypted, nonce)) => {
            let key = security.decrypt(&encrypted, &nonce)?;
            Ok(Some(key))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Delete a provider API key
pub fn delete_provider_key(conn: &Connection, provider: &str) -> Result<bool> {
    let rows = conn.execute(
        "DELETE FROM provider_api_keys WHERE provider = ?1",
        [provider],
    )?;
    Ok(rows > 0)
}

/// Get list of configured provider names
pub fn get_configured_providers(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT provider FROM provider_api_keys")?;
    let providers = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<Vec<String>, _>>()?;
    Ok(providers)
}
