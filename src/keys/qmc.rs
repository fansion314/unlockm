use aes::{
    cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit},
    Aes128,
};
use md5::{Digest, Md5};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

/// Load an explicit key map. JSON object and UTF-8 `identifier=ekey` lines are
/// both accepted so the command line does not require a bespoke key export.
pub fn load_key_map(path: &Path) -> std::result::Result<HashMap<String, String>, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    if let Ok(object) = serde_json::from_str::<HashMap<String, String>>(&text) {
        return Ok(normalize(object));
    }
    let map = text
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, ekey)| (name.trim().to_owned(), ekey.trim().to_owned()))
        .filter(|(name, ekey)| !name.is_empty() && !ekey.is_empty())
        .collect::<HashMap<_, _>>();
    if map.is_empty() {
        return Err("key map is neither a JSON object nor identifier=ekey lines".into());
    }
    Ok(normalize(map))
}

/// Reads the string containers written by `go-mmkv` v0.1.4. MMKV prefixes its
/// payload with a little-endian size and an internal varint header. Encrypted
/// stores keep the same length prefix but AES-128-CFB encrypt the payload; the
/// companion `.crc` file provides the IV at byte offset 12.
pub fn load_mmkv(
    path: &Path,
    password: Option<&str>,
) -> std::result::Result<HashMap<String, String>, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let declared = bytes.get(..4).ok_or("MMKV is missing payload length")?;
    let declared = u32::from_le_bytes(declared.try_into().expect("fixed size")) as usize;
    let encrypted = bytes.get(4..4 + declared).ok_or("truncated MMKV payload")?;
    let payload = match password.filter(|value| !value.is_empty()) {
        Some(password) => decrypt_payload(encrypted, password, &crc_path(path)?)?,
        None => encrypted.to_vec(),
    };
    let mut cursor = 0;
    let mut result = HashMap::new();
    // The Go reader discards this MMKV metadata varint before key/value pairs.
    let (_, after_header) = varint(&payload, cursor)?;
    cursor = after_header;
    while cursor < payload.len() {
        let (key, after_key) = string(&payload, cursor)?;
        cursor = after_key;
        let (container_len, after_len) = varint(&payload, cursor)?;
        cursor = after_len;
        let end = cursor
            .checked_add(container_len as usize)
            .ok_or("MMKV container length overflow")?;
        let container = payload.get(cursor..end).ok_or("truncated MMKV container")?;
        cursor = end;
        let (value, consumed) = string(container, 0)?;
        if consumed > container.len() {
            return Err("MMKV string exceeded its container".into());
        }
        if !key.is_empty() && !value.is_empty() {
            result.insert(normalize_name(&key), value);
        }
    }
    if result.is_empty() {
        Err("MMKV contained no string key entries".into())
    } else {
        Ok(result)
    }
}

fn string(bytes: &[u8], cursor: usize) -> std::result::Result<(String, usize), String> {
    let (len, start) = varint(bytes, cursor)?;
    if len == 0 {
        return Ok((String::new(), start));
    }
    let end = start
        .checked_add(len as usize)
        .ok_or("MMKV string length overflow")?;
    let value = std::str::from_utf8(bytes.get(start..end).ok_or("truncated MMKV string")?)
        .map_err(|_| "MMKV key/value is not UTF-8")?;
    Ok((value.to_owned(), end))
}

fn crc_path(path: &Path) -> std::result::Result<Vec<u8>, String> {
    fs::read(PathBuf::from(format!("{}.crc", path.display())))
        .map_err(|error| format!("encrypted MMKV needs {}.crc: {error}", path.display()))
}

fn decrypt_payload(
    input: &[u8],
    password: &str,
    crc: &[u8],
) -> std::result::Result<Vec<u8>, String> {
    let iv: [u8; 16] = crc
        .get(12..28)
        .ok_or("MMKV crc is missing its 16-byte IV")?
        .try_into()
        .expect("fixed size");
    let mut key = [0_u8; 16];
    let password = password.as_bytes();
    let copy_len = password.len().min(key.len());
    key[..copy_len].copy_from_slice(&password[..copy_len]);
    let cipher = Aes128::new_from_slice(&key).map_err(|_| "invalid MMKV AES key")?;
    let mut result = input.to_vec();
    let mut feedback = iv;
    for chunk in result.chunks_mut(16) {
        let ciphertext = chunk.to_vec();
        let mut stream = GenericArray::clone_from_slice(&feedback);
        cipher.encrypt_block(&mut stream);
        for (byte, key_byte) in chunk.iter_mut().zip(stream.iter()) {
            *byte ^= *key_byte;
        }
        feedback[..chunk.len()].copy_from_slice(&ciphertext);
        if chunk.len() < feedback.len() {
            // CFB uses the last full ciphertext block as feedback; no later
            // block exists, so partial feedback does not need special state.
            break;
        }
    }
    Ok(result)
}

fn varint(bytes: &[u8], mut cursor: usize) -> std::result::Result<(u64, usize), String> {
    let mut value = 0_u64;
    for shift in (0..64).step_by(7) {
        let byte = *bytes.get(cursor).ok_or("truncated MMKV varint")?;
        cursor += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, cursor));
        }
    }
    Err("invalid MMKV varint".into())
}

pub fn lookup<'a>(
    map: &'a HashMap<String, String>,
    path: &Path,
    footer_filename: Option<&str>,
) -> Option<&'a str> {
    let full = normalize_name(&path.to_string_lossy());
    let basename = path
        .file_name()
        .map(|v| normalize_name(&v.to_string_lossy()));
    let footer = footer_filename.map(normalize_name);
    map.get(&full)
        .or_else(|| basename.as_ref().and_then(|v| map.get(v)))
        .or_else(|| footer.as_ref().and_then(|v| map.get(v)))
        .map(String::as_str)
}

fn normalize(map: HashMap<String, String>) -> HashMap<String, String> {
    map.into_iter()
        .map(|(key, value)| (normalize_name(&key), value))
        .collect()
}
fn normalize_name(value: &str) -> String {
    value.trim().replace('\\', "/").to_lowercase()
}

pub fn discover_macos_in(home: &Path) -> HashMap<String, String> {
    let base = home.join(
        "Library/Containers/com.tencent.QQMusicMac/Data/Library/Application Support/QQMusicMac",
    );
    let v8 = base.join("mmkv/MMKVStreamEncryptId");
    let mut result = HashMap::new();
    if v8.is_file() {
        match load_mmkv(&v8, None) {
            Ok(keys) => result.extend(keys),
            Err(error) => {
                tracing::warn!(source=%v8.display(), %error, "could not load QQMusic macOS v8 MMKV")
            }
        }
    }
    let v10 = base.join("iData");
    if v10.is_dir() {
        for plist in [
            home.join("Library/Preferences/com.tencent.QQMusicMac.plist"),
            home.join("Library/Containers/com.tencent.QQMusicMac/Data/Library/Preferences/com.tencent.QQMusicMac.plist"),
        ] {
            let Ok(data) = fs::read(&plist) else { continue };
            for udid in extract_v10_udids(&data) {
                match macos_v10_store(&v10, &udid, 1).and_then(|(path, password)| load_mmkv(&path, Some(&password))) {
                    Ok(keys) => result.extend(keys),
                    Err(error) => tracing::debug!(plist=%plist.display(), %error, "could not load QQMusic macOS v10 MMKV"),
                }
            }
        }
    }
    result
}

fn extract_v10_udids(data: &[u8]) -> Vec<String> {
    data.windows(3)
        .enumerate()
        .filter_map(|(index, marker)| {
            (marker == b"_\x10(")
                .then(|| data.get(index + 3..index + 43))
                .flatten()
                .filter(|value| value.iter().all(u8::is_ascii_hexdigit))
                .and_then(|value| std::str::from_utf8(value).ok())
                .map(str::to_owned)
        })
        .collect()
}

fn caesar(text: &str, shift: u8) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' => (byte - b'A' + shift) % 26 + b'A',
            b'a'..=b'z' => (byte - b'a' + shift) % 26 + b'a',
            b'0'..=b'9' => (byte - b'0' + shift) % 10 + b'0',
            _ => byte,
        })
        .map(char::from)
        .collect()
}

fn macos_v10_store(
    directory: &Path,
    udid: &str,
    id: u32,
) -> std::result::Result<(PathBuf, String), String> {
    let byte = udid.get(5..7).ok_or("macOS v10 UDID is too short")?;
    let value = u8::from_str_radix(byte, 16).map_err(|_| "macOS v10 UDID has invalid hex")?;
    let name_len = 5 + (u32::from(value) + id) % 4;
    let shifted = caesar(udid, (id + 3) as u8);
    let name = shifted
        .get(..name_len as usize)
        .ok_or("macOS v10 UDID is too short")?;
    let digest = Md5::digest(format!("{udid}{:04x}", id + 0xa546));
    Ok((
        directory.join(name),
        format!("{:x}", digest)[..16].to_owned(),
    ))
}

#[cfg(target_os = "macos")]
pub fn discover_macos() -> HashMap<String, String> {
    std::env::var_os("HOME")
        .map(|home| discover_macos_in(Path::new(&home)))
        .unwrap_or_default()
}
#[cfg(not(target_os = "macos"))]
pub fn discover_macos() -> HashMap<String, String> {
    HashMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }
    const PLAIN: &str = "1c000000ffffff0705776f726c64060568656c6c6f04746573740504756e6974";
    const ENCRYPTED: &str = "1c000000fc3d03a45bf75c60952c4ec027b9b7e11ffce55820a5984837a8a2d3";
    const CRC: &str = "8dfeb7bc0400000001000000ed5f0000e22c00003a7900007d090000";
    #[test]
    fn explicit_maps_and_plain_mmkv_normalize_identifiers() {
        let dir = tempfile::tempdir().unwrap();
        let map = dir.path().join("keys.json");
        fs::write(&map, r#"{"Foo\\Track.MFLAC":"ekey"}"#).unwrap();
        assert_eq!(
            load_key_map(&map).unwrap().get("foo/track.mflac").unwrap(),
            "ekey"
        );
        let mmkv = dir.path().join("mmkv");
        fs::write(&mmkv, hex(PLAIN)).unwrap();
        assert_eq!(
            load_mmkv(&mmkv, None).unwrap().get("world").unwrap(),
            "hello"
        );
    }
    #[test]
    fn upstream_plain_and_encrypted_mmkv_vectors_match_go_mmkv() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("plain");
        fs::write(&plain, hex(PLAIN)).unwrap();
        let encrypted = dir.path().join("encrypted");
        fs::write(&encrypted, hex(ENCRYPTED)).unwrap();
        fs::write(format!("{}.crc", encrypted.display()), hex(CRC)).unwrap();
        for map in [
            load_mmkv(&plain, None).unwrap(),
            load_mmkv(&encrypted, Some("unlock-music:key")).unwrap(),
        ] {
            assert_eq!(map.get("world"), Some(&"hello".to_owned()));
            assert_eq!(map.get("test"), Some(&"unit".to_owned()));
        }
    }
    #[test]
    fn macos_v8_discovery_and_v10_derivation_match_reference_algorithm() {
        let home = tempfile::tempdir().unwrap();
        let base = home.path().join(
            "Library/Containers/com.tencent.QQMusicMac/Data/Library/Application Support/QQMusicMac",
        );
        let v8 = base.join("mmkv/MMKVStreamEncryptId");
        fs::create_dir_all(v8.parent().unwrap()).unwrap();
        fs::write(&v8, hex(PLAIN)).unwrap();
        assert_eq!(
            discover_macos_in(home.path()).get("world"),
            Some(&"hello".to_owned())
        );
        let udid = "0123456789abcdef0123456789abcdef01234567";
        let (path, password) = macos_v10_store(Path::new("iData"), udid, 1).unwrap();
        assert_eq!(path, Path::new("iData").join("45678901"));
        assert_eq!(password.len(), 16);
        let plist = [b"prefix_\x10(".as_slice(), udid.as_bytes()].concat();
        assert_eq!(extract_v10_udids(&plist), vec![udid]);
    }
}
