use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

pub fn load_ekeys(path: &Path) -> std::result::Result<HashMap<String, String>, String> {
    let mut bytes = fs::read(path).map_err(|e| e.to_string())?;
    umc_kgm::decrypt_db(&mut bytes).map_err(|_| "KGM database decryption failed".to_owned())?;
    let temp = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    fs::write(temp.path(), bytes).map_err(|e| e.to_string())?;
    let connection = rusqlite::Connection::open_with_flags(
        temp.path(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| format!("open decrypted KGM SQLite database: {e}"))?;
    let mut statement = connection.prepare("SELECT EncryptionKeyId, EncryptionKey FROM ShareFileItems WHERE EncryptionKey IS NOT NULL AND EncryptionKey != ''")
        .map_err(|e| format!("query KGM SQLite database: {e}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut result = HashMap::new();
    for row in rows {
        let (id, key) = row.map_err(|e| e.to_string())?;
        result.insert(id, key);
    }
    Ok(result)
}

#[cfg(target_os = "windows")]
pub fn windows_default_database() -> Option<PathBuf> {
    let path = std::env::var_os("APPDATA")
        .map(PathBuf::from)?
        .join("Kugou8/KGMusicV3.db");
    path.is_file().then_some(path)
}
#[cfg(not(target_os = "windows"))]
pub fn windows_default_database() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plaintext_sqlite_key_map_is_queried_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("keys.db");
        let connection = rusqlite::Connection::open(&db).unwrap();
        connection.execute_batch("CREATE TABLE ShareFileItems (EncryptionKeyId TEXT, EncryptionKey TEXT); INSERT INTO ShareFileItems VALUES ('hash', 'ekey');").unwrap();
        drop(connection);
        assert_eq!(load_ekeys(&db).unwrap().get("hash").unwrap(), "ekey");
    }
}
