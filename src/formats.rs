//! Format selection and bounded streaming transforms.  Format-specific keys do
//! not leak into the application runner.
use crate::{
    error::{AppError, Result},
    keys::KeySources,
};
pub mod adapter;
pub mod audio;
pub mod registry;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ncm,
    QmcV1,
    QmcV2,
    Kgm,
    Kwm,
    Xiami,
    Ximalaya,
    Tm,
}
#[derive(Debug, Clone)]
pub enum Classification {
    Passthrough,
    Encrypted(Kind),
    Unknown,
}
pub enum PreparedClassification {
    Passthrough,
    Encrypted(adapter::PreparedDecrypt),
    Unknown,
}

pub fn supported_extensions() -> Vec<String> {
    registry::all_extensions()
        .into_iter()
        .map(str::to_owned)
        .collect()
}
fn lower_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase()
}
pub fn candidate(path: &Path) -> Option<Kind> {
    registry::candidates(path)
        .first()
        .map(|adapter| adapter.kind())
}

pub fn is_standard_audio(header: &[u8]) -> bool {
    audio::is_standard_audio(header)
}
pub fn extension(header: &[u8]) -> Option<String> {
    audio::extension(header)
}
pub fn classify(path: &Path) -> Result<Classification> {
    Ok(match prepare(path)? {
        PreparedClassification::Passthrough => Classification::Passthrough,
        PreparedClassification::Encrypted(prepared) => Classification::Encrypted(prepared.kind()),
        PreparedClassification::Unknown => Classification::Unknown,
    })
}
pub fn prepare(path: &Path) -> Result<PreparedClassification> {
    let mut f = File::open(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut h = vec![0; 8192];
    let n = f.read(&mut h).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    h.truncate(n);
    if is_standard_audio(&h) {
        return Ok(PreparedClassification::Passthrough);
    }
    // `.xm` is shared by Xiami and Ximalaya.  Xiami has a short, definitive
    // header, so only use the Ximalaya adapter when that positive check fails.
    for adapter in registry::candidates(path) {
        if adapter.validates(path, &h) {
            let mut prepared = adapter.prepare(path);
            if prepared.kind() == Kind::QmcV2 {
                prepared.set_metadata_hints(qmc_metadata_hints(path)?);
            }
            return Ok(PreparedClassification::Encrypted(prepared));
        }
    }
    if candidate(path).is_some() {
        return Err(AppError::InvalidEncrypted(path.to_path_buf()));
    }
    Ok(PreparedClassification::Unknown)
}

pub(crate) fn ncm_metadata_evidence(path: &Path) -> Result<adapter::NcmMetadataEvidence> {
    let mut input = File::open(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let ncm = crate::crypto::ncm::header::NcmFile::new_from_readable(&mut input)
        .map_err(|_| AppError::InvalidEncrypted(path.to_path_buf()))?;
    Ok(adapter::NcmMetadataEvidence {
        json: ncm
            .metadata_json()
            .map_err(|_| AppError::InvalidEncrypted(path.to_path_buf()))?,
        image1: ncm.image1,
    })
}

fn qmc_metadata_hints(path: &Path) -> Result<adapter::MetadataHints> {
    use crate::crypto::qmc::footer::{self, Data};
    let mut input = File::open(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let len = input
        .metadata()
        .map_err(|source| AppError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    if len < footer::INITIAL_DETECTION_LEN as u64 {
        return Ok(adapter::MetadataHints::default());
    }
    input
        .seek(SeekFrom::End(-(footer::INITIAL_DETECTION_LEN as i64)))
        .map_err(|source| AppError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let mut footer_bytes = vec![0; footer::INITIAL_DETECTION_LEN];
    input
        .read_exact(&mut footer_bytes)
        .map_err(|source| AppError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let Some(footer) = footer::from_byte_slice(&footer_bytes).ok().flatten() else {
        return Ok(adapter::MetadataHints::default());
    };
    let (resource_id, media_mid) = match footer.data {
        Data::AndroidQTag(tag) => (Some(tag.resource_id.to_string()), None),
        Data::AndroidSTag(tag) => (Some(tag.resource_id.to_string()), Some(tag.media_mid)),
        Data::PCv2MusicEx(tag) => (None, Some(tag.mid)),
        Data::PCv1Legacy(_) => (None, None),
    };
    Ok(adapter::MetadataHints {
        resource_id,
        media_mid,
    })
}

/// Bounded preflight output prediction. Known suffixes are exact where their
/// payload container is fixed; callers fall back to a private header decrypt
/// for NCM/ambiguous formats before publishing.
pub fn predicted_extension(path: &Path, kind: Kind) -> Option<&'static str> {
    let name = lower_name(path);
    match kind {
        Kind::QmcV1
            if name.ends_with(".qmc0")
                || name.ends_with(".qmc2")
                || name.ends_with(".qmc3")
                || name.ends_with(".qmc4")
                || name.ends_with(".qmc6")
                || name.ends_with(".qmc8") =>
        {
            None
        }
        Kind::QmcV2 if name.ends_with("qmcflac") => Some("flac"),
        Kind::QmcV2 if name.ends_with("qmcogg") => Some("ogg"),
        Kind::Kgm if name.ends_with(".kgm.flac") || name.ends_with(".vpr.flac") => Some("flac"),
        _ => None,
    }
}

const PROBE_BYTES: usize = 128 * 1024;

fn read_probe(reader: &mut impl Read, path: &Path, limit: usize) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(limit);
    reader
        .take(limit as u64)
        .read_to_end(&mut output)
        .map_err(|source| AppError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(output)
}

fn probe_detect(bytes: &[u8], input: &Path) -> Result<String> {
    extension(bytes).ok_or_else(|| AppError::InvalidEncrypted(input.to_path_buf()))
}

/// Resolve the native output extension from at most a bounded decrypted
/// prefix. This is the cheap planning operation used before collision grouping;
/// it never creates a full decrypted stage.
pub(crate) fn probe_extension(
    kind: Kind,
    input: &Path,
    qmc_ekey: Option<&str>,
    keys: &KeySources,
) -> Result<String> {
    match kind {
        Kind::Ncm => {
            let mut file = File::open(input).map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
            let ncm = crate::crypto::ncm::header::NcmFile::new_from_readable(&mut file)
                .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
            file.seek(SeekFrom::Start(ncm.audio_data_offset as u64))
                .map_err(|source| AppError::Io {
                    path: input.to_path_buf(),
                    source,
                })?;
            let mut prefix = read_probe(&mut file, input, PROBE_BYTES)?;
            ncm.decrypt(&mut prefix, 0);
            probe_detect(&prefix, input)
        }
        Kind::QmcV1 => {
            let mut file = File::open(input).map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
            let mut prefix = read_probe(&mut file, input, PROBE_BYTES)?;
            if let Some(ekey) = keys.qmc_ekey(input, None) {
                crate::crypto::qmc::QmcV2Cipher::new_from_ekey(ekey)
                    .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?
                    .decrypt(&mut prefix, 0);
            } else {
                crate::crypto::qmc::v1::decrypt(&mut prefix, 0);
            }
            probe_detect(&prefix, input)
        }
        Kind::QmcV2 => {
            use crate::crypto::qmc::footer;
            let mut file = File::open(input).map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
            let total = file
                .metadata()
                .map_err(|source| AppError::Io {
                    path: input.to_path_buf(),
                    source,
                })?
                .len();
            if total < footer::INITIAL_DETECTION_LEN as u64 {
                return Err(AppError::InvalidEncrypted(input.to_path_buf()));
            }
            file.seek(SeekFrom::End(-(footer::INITIAL_DETECTION_LEN as i64)))
                .map_err(|source| AppError::Io {
                    path: input.to_path_buf(),
                    source,
                })?;
            let mut footer_bytes = vec![0; footer::INITIAL_DETECTION_LEN];
            file.read_exact(&mut footer_bytes)
                .map_err(|source| AppError::Io {
                    path: input.to_path_buf(),
                    source,
                })?;
            let parsed = footer::from_byte_slice(&footer_bytes).ok().flatten();
            let footer_size = parsed.as_ref().map_or(0, |metadata| metadata.size) as u64;
            let footer_filename = parsed.as_ref().and_then(|metadata| match &metadata.data {
                footer::Data::PCv2MusicEx(tag) => Some(tag.media_filename.clone()),
                _ => None,
            });
            let ekey = parsed
                .and_then(|metadata| metadata.ekey)
                .or_else(|| qmc_ekey.map(str::to_owned))
                .or_else(|| {
                    keys.qmc_ekey(input, footer_filename.as_deref())
                        .map(str::to_owned)
                })
                .ok_or_else(|| AppError::InvalidEncrypted(input.to_path_buf()))?;
            let cipher = crate::crypto::qmc::QmcV2Cipher::new_from_ekey(ekey)
                .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
            file.seek(SeekFrom::Start(0))
                .map_err(|source| AppError::Io {
                    path: input.to_path_buf(),
                    source,
                })?;
            let payload = total
                .checked_sub(footer_size)
                .ok_or_else(|| AppError::InvalidEncrypted(input.to_path_buf()))?;
            let mut prefix = read_probe(&mut file.take(payload), input, PROBE_BYTES)?;
            cipher.decrypt(&mut prefix, 0);
            probe_detect(&prefix, input)
        }
        Kind::Kgm => {
            let mut file = File::open(input).map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
            let header = crate::crypto::kgm::header::Header::from_reader(&mut file)
                .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
            let ekey = (header.crypto_version == 5)
                .then(|| keys.kgm_ekey(&header.audio_hash))
                .flatten()
                .map(str::to_owned);
            let cipher = crate::crypto::kgm::Decipher::new_v5(&header, ekey)
                .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
            file.seek(SeekFrom::Start(header.offset_to_data as u64))
                .map_err(|source| AppError::Io {
                    path: input.to_path_buf(),
                    source,
                })?;
            let mut prefix = read_probe(&mut file, input, PROBE_BYTES)?;
            cipher.decrypt(&mut prefix, 0);
            probe_detect(&prefix, input)
        }
        Kind::Kwm => {
            let mut file = File::open(input).map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
            let mut raw = [0_u8; 0x40];
            file.read_exact(&mut raw)
                .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
            let header = crate::crypto::kuwo::Header::from_bytes(raw)
                .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
            let cipher = crate::crypto::kuwo::Decipher::new(
                &header,
                qmc_ekey.or_else(|| keys.qmc_ekey(input, None)),
            )
            .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
            file.seek(SeekFrom::Start(
                crate::crypto::kuwo::DATA_START_OFFSET as u64,
            ))
            .map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
            let mut prefix = read_probe(&mut file, input, PROBE_BYTES)?;
            cipher.decrypt(&mut prefix, 0);
            probe_detect(&prefix, input)
        }
        Kind::Xiami => {
            let mut file = File::open(input).map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
            let mut raw = [0_u8; 0x10];
            file.read_exact(&mut raw)
                .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
            let cipher = crate::crypto::xiami::XiamiFile::from_header(&raw)
                .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
            let copied = cipher.copy_len.min(PROBE_BYTES);
            let mut prefix = read_probe(&mut file, input, copied)?;
            if prefix.len() < PROBE_BYTES && copied == cipher.copy_len {
                let mut encrypted = read_probe(&mut file, input, PROBE_BYTES - prefix.len())?;
                cipher.decrypt(&mut encrypted);
                prefix.extend_from_slice(&encrypted);
            }
            probe_detect(&prefix, input)
        }
        Kind::Ximalaya => {
            let mut file = File::open(input).map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
            let name = lower_name(input);
            let mut prefix = Vec::new();
            if name.ends_with(".x2m") || name.ends_with(".x3m") {
                let mut header = [0_u8; 0x400];
                file.read_exact(&mut header)
                    .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
                let kind = if name.ends_with(".x2m") {
                    crate::crypto::xmly::android::FileType::X2M
                } else {
                    crate::crypto::xmly::android::FileType::X3M
                };
                crate::crypto::xmly::android::decrypt_android(kind, &mut header);
                prefix.extend_from_slice(&header);
                prefix.extend(read_probe(&mut file, input, PROBE_BYTES - prefix.len())?);
            } else {
                let mut header = vec![0_u8; 8192];
                file.read_exact(&mut header)
                    .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
                let parsed = crate::crypto::xmly::pc::Header::from_buffer(&header)
                    .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
                if parsed.encrypted_header_size > PROBE_BYTES {
                    return Err(AppError::InvalidEncrypted(input.to_path_buf()));
                }
                prefix.extend_from_slice(&parsed.copy_m4a_header());
                file.seek(SeekFrom::Start(parsed.data_start_offset as u64))
                    .map_err(|source| AppError::Io {
                        path: input.to_path_buf(),
                        source,
                    })?;
                let mut encrypted = vec![0_u8; parsed.encrypted_header_size];
                file.read_exact(&mut encrypted)
                    .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
                prefix.extend_from_slice(
                    parsed
                        .decrypt(&mut encrypted)
                        .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?,
                );
            }
            probe_detect(&prefix, input)
        }
        Kind::Tm => {
            let mut file = File::open(input).map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
            let mut encrypted_header = [0_u8; 8];
            file.read_exact(&mut encrypted_header)
                .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
            if &encrypted_header[..4] != b"QQMU" {
                return Err(AppError::InvalidEncrypted(input.to_path_buf()));
            }
            let mut prefix = vec![0, 0, 0, 0x20, b'f', b't', b'y', b'p'];
            prefix.extend(read_probe(&mut file, input, PROBE_BYTES - prefix.len())?);
            probe_detect(&prefix, input)
        }
    }
}

/// Write a transformed payload using a fixed-size caller-provided buffer.
pub fn decrypt_to(
    kind: Kind,
    input: &Path,
    output: &mut File,
    buffer_size: usize,
    qmc_ekey: Option<&str>,
    keys: &KeySources,
) -> Result<String> {
    decrypt_to_expected(kind, input, output, buffer_size, qmc_ekey, keys, None)
}

pub(crate) fn decrypt_to_expected(
    kind: Kind,
    input: &Path,
    output: &mut File,
    buffer_size: usize,
    qmc_ekey: Option<&str>,
    keys: &KeySources,
    expected_extension: Option<&str>,
) -> Result<String> {
    match kind {
        Kind::Ncm => decrypt_ncm(input, output, buffer_size, expected_extension),
        Kind::QmcV1 => decrypt_qmc_v1(input, output, buffer_size, keys, expected_extension),
        Kind::QmcV2 => decrypt_qmc_v2(
            input,
            output,
            buffer_size,
            qmc_ekey,
            keys,
            expected_extension,
        ),
        Kind::Kgm => decrypt_kgm(input, output, buffer_size, keys, expected_extension),
        Kind::Kwm => decrypt_kwm(
            input,
            output,
            buffer_size,
            qmc_ekey.or_else(|| keys.qmc_ekey(input, None)),
            expected_extension,
        ),
        Kind::Xiami => decrypt_xiami(input, output, buffer_size, expected_extension),
        Kind::Ximalaya => decrypt_ximalaya(input, output, buffer_size, expected_extension),
        Kind::Tm => decrypt_tm(input, output, expected_extension),
    }
}
fn decrypt_tm(input: &Path, output: &mut File, expected_extension: Option<&str>) -> Result<String> {
    let mut f = File::open(input).map_err(|source| AppError::Io {
        path: input.to_path_buf(),
        source,
    })?;
    let mut header = [0_u8; 8];
    f.read_exact(&mut header)
        .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
    if &header[..4] != b"QQMU" {
        return Err(AppError::InvalidEncrypted(input.to_path_buf()));
    }
    output
        .write_all(&[0, 0, 0, 0x20, b'f', b't', b'y', b'p'])
        .map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
    std::io::copy(&mut f, output).map_err(|source| AppError::Io {
        path: input.to_path_buf(),
        source,
    })?;
    finish_written(output, expected_extension)
}
fn decrypt_kgm(
    input: &Path,
    output: &mut File,
    buffer_size: usize,
    keys: &KeySources,
    expected_extension: Option<&str>,
) -> Result<String> {
    use crate::crypto::kgm::Decipher;
    let mut f = File::open(input).map_err(|source| AppError::Io {
        path: input.to_path_buf(),
        source,
    })?;
    let header = crate::crypto::kgm::header::Header::from_reader(&mut f)
        .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
    let ekey = (header.crypto_version == 5)
        .then(|| keys.kgm_ekey(&header.audio_hash))
        .flatten()
        .map(str::to_owned);
    let decipher = Decipher::new_v5(&header, ekey)
        .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
    f.seek(SeekFrom::Start(header.offset_to_data as u64))
        .map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
    transform_stream(&mut f, output, buffer_size, |bytes, offset| {
        decipher.decrypt(bytes, offset)
    })?;
    finish_written(output, expected_extension)
}
fn decrypt_kwm(
    input: &Path,
    output: &mut File,
    buffer_size: usize,
    ekey: Option<&str>,
    expected_extension: Option<&str>,
) -> Result<String> {
    let mut f = File::open(input).map_err(|source| AppError::Io {
        path: input.to_path_buf(),
        source,
    })?;
    let mut raw = [0_u8; 0x40];
    f.read_exact(&mut raw)
        .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
    let header = crate::crypto::kuwo::Header::from_bytes(raw)
        .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
    let decipher = crate::crypto::kuwo::Decipher::new(&header, ekey)
        .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
    f.seek(SeekFrom::Start(
        crate::crypto::kuwo::DATA_START_OFFSET as u64,
    ))
    .map_err(|source| AppError::Io {
        path: input.to_path_buf(),
        source,
    })?;
    transform_stream(&mut f, output, buffer_size, |bytes, offset| {
        decipher.decrypt(bytes, offset)
    })?;
    finish_written(output, expected_extension)
}
fn decrypt_xiami(
    input: &Path,
    output: &mut File,
    buffer_size: usize,
    expected_extension: Option<&str>,
) -> Result<String> {
    let mut f = File::open(input).map_err(|source| AppError::Io {
        path: input.to_path_buf(),
        source,
    })?;
    let mut raw = [0_u8; 0x10];
    f.read_exact(&mut raw)
        .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
    let decipher = crate::crypto::xiami::XiamiFile::from_header(&raw)
        .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
    let mut remaining = decipher.copy_len;
    let mut buffer = vec![0; buffer_size];
    while remaining > 0 {
        let want = remaining.min(buffer.len());
        let n = f.read(&mut buffer[..want]).map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        if n == 0 {
            return Err(AppError::InvalidEncrypted(input.to_path_buf()));
        };
        output
            .write_all(&buffer[..n])
            .map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
        remaining -= n;
    }
    let mut offset = 0;
    loop {
        let n = f.read(&mut buffer).map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        if n == 0 {
            break;
        }
        decipher.decrypt(&mut buffer[..n]);
        output
            .write_all(&buffer[..n])
            .map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
        offset += n;
    }
    let _ = offset;
    finish_written(output, expected_extension)
}
fn decrypt_ximalaya(
    input: &Path,
    output: &mut File,
    buffer_size: usize,
    expected_extension: Option<&str>,
) -> Result<String> {
    let mut f = File::open(input).map_err(|source| AppError::Io {
        path: input.to_path_buf(),
        source,
    })?;
    let name = lower_name(input);
    if name.ends_with(".x2m") || name.ends_with(".x3m") {
        let mut header = [0_u8; 0x400];
        f.read_exact(&mut header)
            .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
        let kind = if name.ends_with(".x2m") {
            crate::crypto::xmly::android::FileType::X2M
        } else {
            crate::crypto::xmly::android::FileType::X3M
        };
        crate::crypto::xmly::android::decrypt_android(kind, &mut header);
        output.write_all(&header).map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        std::io::copy(&mut f, output).map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
    } else {
        let mut header = vec![0_u8; 8192];
        f.read_exact(&mut header)
            .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
        let parsed = crate::crypto::xmly::pc::Header::from_buffer(&header)
            .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
        output
            .write_all(parsed.copy_m4a_header().as_slice())
            .map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
        f.seek(SeekFrom::Start(parsed.data_start_offset as u64))
            .map_err(|source| AppError::Io {
                path: input.to_path_buf(),
                source,
            })?;
        let mut encrypted = vec![0_u8; parsed.encrypted_header_size];
        f.read_exact(&mut encrypted)
            .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
        let plain = parsed
            .decrypt(&mut encrypted)
            .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
        output.write_all(plain).map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        let _ = buffer_size;
        std::io::copy(&mut f, output).map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
    }
    finish_written(output, expected_extension)
}
fn transform_stream<F: FnMut(&mut [u8], usize)>(
    input: &mut File,
    output: &mut File,
    buffer_size: usize,
    mut transform: F,
) -> Result<()> {
    let mut offset = 0;
    let mut buffer = vec![0; buffer_size];
    loop {
        let n = input.read(&mut buffer).map_err(|source| AppError::Io {
            path: Path::new("input").to_path_buf(),
            source,
        })?;
        if n == 0 {
            break;
        }
        transform(&mut buffer[..n], offset);
        output
            .write_all(&buffer[..n])
            .map_err(|source| AppError::Io {
                path: Path::new("output").to_path_buf(),
                source,
            })?;
        offset += n;
    }
    Ok(())
}
fn decrypt_qmc_v2(
    input: &Path,
    output: &mut File,
    buffer_size: usize,
    override_ekey: Option<&str>,
    keys: &KeySources,
    expected_extension: Option<&str>,
) -> Result<String> {
    use crate::crypto::qmc::{QmcV2Cipher, footer};
    let mut f = File::open(input).map_err(|source| AppError::Io {
        path: input.to_path_buf(),
        source,
    })?;
    let total = f
        .metadata()
        .map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?
        .len();
    if total < footer::INITIAL_DETECTION_LEN as u64 {
        return Err(AppError::InvalidEncrypted(input.to_path_buf()));
    }
    f.seek(SeekFrom::End(-(footer::INITIAL_DETECTION_LEN as i64)))
        .map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
    let mut footer_bytes = vec![0; footer::INITIAL_DETECTION_LEN];
    f.read_exact(&mut footer_bytes)
        .map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
    let parsed = footer::from_byte_slice(&footer_bytes).ok().flatten();
    let footer_size = parsed.as_ref().map_or(0, |metadata| metadata.size) as u64;
    let footer_filename = parsed.as_ref().and_then(|metadata| match &metadata.data {
        footer::Data::PCv2MusicEx(tag) => Some(tag.media_filename.clone()),
        _ => None,
    });
    let ekey = parsed
        .and_then(|metadata| metadata.ekey)
        .or_else(|| override_ekey.map(str::to_owned))
        .or_else(|| {
            keys.qmc_ekey(input, footer_filename.as_deref())
                .map(str::to_owned)
        })
        .ok_or_else(|| AppError::InvalidEncrypted(input.to_path_buf()))?;
    let cipher = QmcV2Cipher::new_from_ekey(ekey)
        .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
    f.seek(SeekFrom::Start(0)).map_err(|source| AppError::Io {
        path: input.to_path_buf(),
        source,
    })?;
    let mut remaining = total
        .checked_sub(footer_size)
        .ok_or_else(|| AppError::InvalidEncrypted(input.to_path_buf()))?;
    let mut offset = 0usize;
    let mut buf = vec![0; buffer_size];
    while remaining > 0 {
        let length = usize::try_from(remaining.min(buf.len() as u64)).expect("bounded by buffer");
        let n = f.read(&mut buf[..length]).map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        if n == 0 {
            return Err(AppError::InvalidEncrypted(input.to_path_buf()));
        }
        cipher.decrypt(&mut buf[..n], offset);
        output.write_all(&buf[..n]).map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        remaining -= n as u64;
        offset += n;
    }
    finish_written(output, expected_extension)
}
fn finish_written(output: &mut File, expected_extension: Option<&str>) -> Result<String> {
    match expected_extension {
        Some(extension) => Ok(extension.to_owned()),
        None => sniff_written(output),
    }
}

fn sniff_written(output: &mut File) -> Result<String> {
    output.flush().map_err(|source| AppError::Io {
        path: Path::new("output").to_path_buf(),
        source,
    })?;
    output
        .seek(SeekFrom::Start(0))
        .map_err(|source| AppError::Io {
            path: Path::new("output").to_path_buf(),
            source,
        })?;
    let mut h = vec![0; 128 * 1024];
    let n = output.read(&mut h).map_err(|source| AppError::Io {
        path: Path::new("output").to_path_buf(),
        source,
    })?;
    h.truncate(n);
    extension(&h)
        .ok_or_else(|| AppError::InvalidEncrypted(Path::new("decrypted payload").to_path_buf()))
}
fn decrypt_ncm(
    input: &Path,
    output: &mut File,
    buffer_size: usize,
    expected_extension: Option<&str>,
) -> Result<String> {
    let mut input_file = File::open(input).map_err(|source| AppError::Io {
        path: input.to_path_buf(),
        source,
    })?;
    let ncm = crate::crypto::ncm::header::NcmFile::new_from_readable(&mut input_file)
        .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
    input_file
        .seek(SeekFrom::Start(ncm.audio_data_offset as u64))
        .map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
    let mut offset = 0usize;
    let mut buf = vec![0; buffer_size];
    loop {
        let n = input_file.read(&mut buf).map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        if n == 0 {
            break;
        }
        ncm.decrypt(&mut buf[..n], offset);
        output.write_all(&buf[..n]).map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        offset += n;
    }
    finish_written(output, expected_extension)
}
fn decrypt_qmc_v1(
    input: &Path,
    output: &mut File,
    buffer_size: usize,
    keys: &KeySources,
    expected_extension: Option<&str>,
) -> Result<String> {
    let mut f = File::open(input).map_err(|source| AppError::Io {
        path: input.to_path_buf(),
        source,
    })?;
    let keyed = keys
        .qmc_ekey(input, None)
        .map(crate::crypto::qmc::QmcV2Cipher::new_from_ekey)
        .transpose()
        .map_err(|_| AppError::InvalidEncrypted(input.to_path_buf()))?;
    let mut offset = 0;
    let mut buf = vec![0; buffer_size];
    loop {
        let n = f.read(&mut buf).map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        if n == 0 {
            break;
        }
        if let Some(cipher) = &keyed {
            cipher.decrypt(&mut buf[..n], offset);
        } else {
            crate::crypto::qmc::v1::decrypt(&mut buf[..n], offset);
        }
        output.write_all(&buf[..n]).map_err(|source| AppError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        offset += n;
    }
    finish_written(output, expected_extension)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extension_order_covers_multidot() {
        assert_eq!(candidate(Path::new("x.kgm.flac")), Some(Kind::Kgm));
    }
    #[test]
    fn qq_alias_families_are_registered_case_insensitively() {
        for name in ["song.BKCFLAC", "song.666C6163", "song.mggl", "song.MFLAC0"] {
            assert!(matches!(
                candidate(Path::new(name)),
                Some(Kind::QmcV1 | Kind::QmcV2)
            ));
        }
        assert!(supported_extensions().iter().any(|ext| ext == ".bkcflac"));
        assert!(supported_extensions().iter().any(|ext| ext == ".mflacm"));
    }
    #[test]
    fn every_qmc_suffix_routes_through_the_registry_adapter() {
        for suffix in registry::candidates(Path::new("track.qmc0"))[0].suffixes() {
            assert_eq!(
                candidate(Path::new(&format!("track{suffix}"))),
                Some(Kind::QmcV1)
            );
        }
        for suffix in registry::candidates(Path::new("track.mflac"))[0].suffixes() {
            assert_eq!(
                candidate(Path::new(&format!("track{suffix}"))),
                Some(Kind::QmcV2)
            );
        }
    }
    #[test]
    fn transform_stream_keeps_absolute_offsets_at_tiny_boundaries() {
        let fixture: Vec<u8> = (0..16_513).map(|n| (n % 251) as u8).collect();
        for size in [1, 7, 16, 4096] {
            let dir = tempfile::tempdir().unwrap();
            let input_path = dir.path().join("input");
            let output_path = dir.path().join("output");
            std::fs::write(&input_path, &fixture).unwrap();
            let mut input = File::open(input_path).unwrap();
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&output_path)
                .unwrap();
            transform_stream(&mut input, &mut output, size, |chunk, offset| {
                for (index, byte) in chunk.iter_mut().enumerate() {
                    *byte ^= ((offset + index) % 251) as u8;
                }
            })
            .unwrap();
            assert_eq!(
                std::fs::read(output_path).unwrap(),
                vec![0; fixture.len()],
                "buffer={size}"
            );
        }
    }
    #[test]
    fn planned_extension_avoids_completed_stage_reread() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("planned.qmc0");
        let mut encrypted = b"fLaC".to_vec();
        encrypted.resize(4096, 0);
        crate::crypto::qmc::v1::decrypt(&mut encrypted, 0);
        std::fs::write(&source, encrypted).unwrap();
        let prepared = match prepare(&source).unwrap() {
            PreparedClassification::Encrypted(prepared) => prepared,
            _ => panic!("fixture must be encrypted"),
        };
        let extension = prepared
            .probe_extension(None, &KeySources::default())
            .unwrap();
        assert_eq!(extension, "flac");
        // `File::create` is write-only. A completed-output sniff would fail;
        // execution must trust the already validated bounded-prefix result.
        let mut output = File::create(directory.path().join("stage")).unwrap();
        assert_eq!(
            prepared
                .decrypt_to_expected(&mut output, 7, None, &KeySources::default(), &extension)
                .unwrap(),
            "flac"
        );
    }
    #[test]
    fn registry_requires_positive_validation_but_allows_disguised_audio() {
        let dir = tempfile::tempdir().unwrap();
        let valid = dir.path().join("valid.tm2");
        std::fs::write(&valid, b"QQMU\0\0\0\0payload").unwrap();
        assert!(matches!(
            prepare(&valid).unwrap(),
            PreparedClassification::Encrypted(prepared) if prepared.kind() == Kind::Tm
        ));

        let invalid = dir.path().join("invalid.tm2");
        std::fs::write(&invalid, b"not-encrypted").unwrap();
        assert!(prepare(&invalid).is_err());
        let truncated = dir.path().join("truncated.kgm.flac");
        std::fs::write(&truncated, b"short").unwrap();
        assert!(prepare(&truncated).is_err());

        let disguised = dir.path().join("playable.ncm");
        let mut flac = b"fLaC".to_vec();
        flac.resize(64, 0);
        std::fs::write(&disguised, flac).unwrap();
        assert!(matches!(
            prepare(&disguised).unwrap(),
            PreparedClassification::Passthrough
        ));
    }
    #[test]
    fn tm_bounded_probe_rejects_header_only_junk_and_accepts_m4a_brand() {
        let directory = tempfile::tempdir().unwrap();
        let invalid = directory.path().join("invalid.tm6");
        std::fs::write(&invalid, b"QQMU\0\0\0\0junk payload").unwrap();
        let invalid = match prepare(&invalid).unwrap() {
            PreparedClassification::Encrypted(prepared) => prepared,
            _ => panic!("TM header must classify as encrypted"),
        };
        assert!(
            invalid
                .probe_extension(None, &KeySources::default())
                .is_err()
        );

        let valid = directory.path().join("valid.tm6");
        let mut bytes = b"QQMU\0\0\0\0M4A ".to_vec();
        bytes.resize(4096, 0);
        std::fs::write(&valid, bytes).unwrap();
        let valid = match prepare(&valid).unwrap() {
            PreparedClassification::Encrypted(prepared) => prepared,
            _ => panic!("TM header must classify as encrypted"),
        };
        assert_eq!(
            valid.probe_extension(None, &KeySources::default()).unwrap(),
            "m4a"
        );
    }
    #[test]
    fn shared_xm_keeps_xiami_before_ximalaya_for_positive_validation() {
        let candidates = registry::candidates(Path::new("shared.xm"));
        assert_eq!(candidates[0].kind(), Kind::Xiami);
        assert_eq!(candidates[1].kind(), Kind::Ximalaya);
    }
    #[test]
    fn qmc_v2_pc_footer_fixture_trims_and_ekey_paths_are_bounded() {
        use crate::crypto::qmc::{QmcV2Cipher, ekey, footer};
        use sha2::{Digest, Sha256};
        let dir = tempfile::tempdir().unwrap();
        // MIT/Apache-2.0 upstream qmc footer fixture, stored as base64 so this
        // repository is self-contained while preserving attribution in notices.
        let fixture = pc_footer_fixture();
        let metadata = footer::from_byte_slice(&fixture).unwrap().unwrap();
        assert_eq!(fixture.len(), 720);
        assert_eq!(metadata.size, 0x2c4);
        let ekey = metadata.ekey.unwrap();
        let raw_key = ekey::decrypt(&ekey).unwrap();
        assert_eq!(
            ekey::decrypt(ekey.trim_end_matches('=').as_bytes()).unwrap(),
            raw_key,
            "unpadded v1 EKey must use indifferent base64 padding"
        );
        let v2_ekey = ekey::encode_v2_for_test(ekey.as_bytes());
        assert_eq!(
            ekey::decrypt(v2_ekey).unwrap(),
            raw_key,
            "QQMusic EncV2 wrapper must round-trip the pinned v1 EKey"
        );
        let mut plain = b"fLaC".to_vec();
        plain.extend((0..8_192).map(|n| (n % 251) as u8));
        plain.resize(8_224, 0);
        let mut encrypted = plain.clone();
        QmcV2Cipher::new(&raw_key)
            .unwrap()
            .decrypt(&mut encrypted, 0);
        encrypted.extend_from_slice(&fixture[fixture.len() - metadata.size..]);
        let embedded = dir.path().join("embedded.mflac");
        std::fs::write(&embedded, encrypted).unwrap();

        for size in [1, 7, 16, 4096] {
            let output = dir.path().join(format!("embedded-{size}"));
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&output)
                .unwrap();
            assert_eq!(
                decrypt_to(
                    Kind::QmcV2,
                    &embedded,
                    &mut file,
                    size,
                    None,
                    &KeySources::default()
                )
                .unwrap(),
                "flac"
            );
            let output = std::fs::read(output).unwrap();
            assert_eq!(output, plain, "buffer={size}");
            assert_eq!(
                hex::encode(Sha256::digest(&output)),
                "ead056c44472778892f0ff35b7f76e2ca47e9db1afd4885ba30ebe6be4146ac5"
            );
        }

        let override_input = dir.path().join("override.mflac");
        let mut override_ciphertext = plain.clone();
        QmcV2Cipher::new(&raw_key)
            .unwrap()
            .decrypt(&mut override_ciphertext, 0);
        std::fs::write(&override_input, override_ciphertext).unwrap();
        let mut output = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(dir.path().join("override-output"))
            .unwrap();
        assert_eq!(
            decrypt_to(
                Kind::QmcV2,
                &override_input,
                &mut output,
                7,
                Some(&ekey),
                &KeySources::default()
            )
            .unwrap(),
            "flac"
        );
        assert_eq!(
            std::fs::read(dir.path().join("override-output")).unwrap(),
            plain
        );
        let mut missing = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(dir.path().join("missing-output"))
            .unwrap();
        assert!(
            decrypt_to(
                Kind::QmcV2,
                &override_input,
                &mut missing,
                7,
                None,
                &KeySources::default()
            )
            .is_err()
        );
        let mut invalid = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(dir.path().join("invalid-output"))
            .unwrap();
        assert!(
            decrypt_to(
                Kind::QmcV2,
                &override_input,
                &mut invalid,
                7,
                Some("invalid"),
                &KeySources::default()
            )
            .is_err()
        );
    }

    #[test]
    fn qmc_footer_identity_hints_cover_qtag_stag_and_pc_v2() {
        use base64::Engine;
        fn source_with_footer(dir: &Path, payload: &str, marker: &[u8]) -> std::path::PathBuf {
            let mut bytes = vec![0; footer::INITIAL_DETECTION_LEN - payload.len() - 8];
            bytes.extend_from_slice(payload.as_bytes());
            bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            bytes.extend_from_slice(marker);
            let path = dir.join(format!("{}.mflac", payload.len()));
            std::fs::write(&path, bytes).unwrap();
            path
        }
        use crate::crypto::qmc::footer;
        let dir = tempfile::tempdir().unwrap();
        let qtag = source_with_footer(dir.path(), "YWJj,123,2", b"QTag");
        assert_eq!(
            qmc_metadata_hints(&qtag).unwrap(),
            adapter::MetadataHints {
                resource_id: Some("123".into()),
                media_mid: None,
            }
        );
        let stag = source_with_footer(dir.path(), "456,2,media-mid", b"STag");
        assert_eq!(
            qmc_metadata_hints(&stag).unwrap(),
            adapter::MetadataHints {
                resource_id: Some("456".into()),
                media_mid: Some("media-mid".into()),
            }
        );
        let encoded = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/qmc_pc_enc_v2.bin.b64"),
        )
        .unwrap();
        let footer_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .unwrap();
        let mut pc_bytes = vec![0_u8; footer::INITIAL_DETECTION_LEN - footer_bytes.len()];
        pc_bytes.extend_from_slice(&footer_bytes);
        assert!(matches!(
            footer::from_byte_slice(&pc_bytes).unwrap().unwrap().data,
            footer::Data::PCv2MusicEx(_)
        ));
        let pc = dir.path().join("pc-v2.mflac");
        std::fs::write(&pc, pc_bytes).unwrap();
        assert_eq!(
            qmc_metadata_hints(&pc).unwrap(),
            adapter::MetadataHints {
                resource_id: None,
                media_mid: Some("AaBbCcDdEeFfGg".into()),
            }
        );
    }

    #[test]
    fn ncm_upstream_vector_has_a_checked_header_offset_and_metadata_hint() {
        // MIT/Apache-2.0 upstream `ncm_test1.bin` (1056 B), retained as text;
        // it intentionally contains only a 15-byte audio fragment, so it is a
        // parser vector rather than a false claim of a probeable media fixture.
        let input = decode_base64(
            "Q1RFTkZEQU0BaZAAAAAsztXraer7FFUNRb9h3Rcdk3FHHuHd2vTV6E8cugAgwwLp/imS4YFFbxjHLRHyvFu83CIz+Wi0sCg4P2NsiGY1+eexcA7uVay47YtIFyU65l61gHiKzdzh7z0w7JwqxsdRrj0RtWSIntZ3ZvYrUp76+WP23icQRYKsLSCElUwPeq6LkW0QLmMc6sr5FJfYs+hWAwAAUlVQQwgGGksnDA1EF0MODAcKBRpKWS9VVyU2UDRXOhs7UDklNw4BOUhbTAUJBytVIgBIMTJMFwcNJBo5ESZaFSkkUAIHIFYtM1oMECkmIihMGg0LBigNLBsJLgsrKAITDy8sDiUbTCtSMzYJNy0LJScsDA0CUTVaGjUtLztUKg1MKxYbDCYhC1IlUjZbUVBWAQoZFFtXMgcGUCVVVFMwLDJSFQBQNzkoDhUPNkgRJDNWByIgKRQoBhYBVEgLFyAlNgIBFgcnFgw3ICokISJQVg40UjcUKRAyIDYtUiECCTsLARcAMzYNASErF0w5AjNRAiAHN0xaJTkJUSVQVEwZMjcIBjJVG1MLC0wIFjNUJxECGjAoVyVQOy4oNFE7NhFTElYJISssFC0kWgJTFiEpFgJTAioxLTFWOxdRUg9WUFYAF1cvCBpSJRItSAUCNw82MAUUWjdaUFUWCVM7GyQaGiFQBjcuVwk5FxMmMQRSUFUQU1JXAgQCLwFaAiovGUwMVQBQLSkGFhEgDQFTIjIbNAAKBgIVDyVaGiIRDiEyMSI5VloJFjUWMCkPMCcsVVISSBYPDDYMJScoNQoaBgkWORsIECcSLCxbSAFIM0gOG1QbAjYlIEgxCwYNFREXASQgJzEwIg8iCBcPEwwMGhISUUw7Uzs3ORYlBVQQMg1IDFBbIAsZIhkNCygsCwU7UQlUCgYPVwhRUCQpFlczNhUEUFsvUVYSCS8nMFErN1ckNVsMDg0NUyolBjMiKVEkCFE6Bg4WMTYrEDQwCAovGTJaBDlSNhomFA4LKkw5VhoBLxQzM1YWJSkzDgstGhEpCBUCK0wBDAcPFUw2NglME1MRERICDxFWK1shKQtRCQI7EQIkFhIECjcLIloLAgoCNjYqKRoUFBMbLBExKkwoESsbMFE1EVMNCQ85DyUXAlsaOSZbUTsuBC4hNgQ7EVYKLi0aJyEQB1MRNwcvUVcrJFcBUzVSDy8tSFJWKwAXGjkAEgkCBwJSLCBMOUxIUA0PFUwAKw83SBkXMjUxKAsNLTIQMlYzKDsrVRIEB1AkAAgBECYwLDQQIQ0lAi5TESsZNjEuWlIzNSAUUzAEBQg0EwJUIgVaASATFDkqJRs3SFdTKiI6ADRTJBJGV2xHAQwAAAAFAAAAaW1nIzFJTUFHRSMyLmTDX6yvG3QMVRTuwFBF",
        );
        let ncm = crate::crypto::ncm::header::NcmFile::new(&input).unwrap();
        assert_eq!(input.len(), 1056);
        assert_eq!(ncm.audio_data_offset, 1041);
        assert!(ncm.metadata_json().unwrap().starts_with(b"{\"musicId\""));
        let mut audio = input[ncm.audio_data_offset..].to_vec();
        ncm.decrypt(&mut audio, 0);
        assert_eq!(&audio[..3], b"ID3");
    }

    #[test]
    fn kgm_v5_uses_only_database_resolved_ekeys() {
        use crate::crypto::qmc::QmcV2Cipher;
        let footer = crate::crypto::qmc::footer::from_byte_slice(&pc_footer_fixture())
            .unwrap()
            .unwrap();
        let ekey = footer.ekey.unwrap();
        let cipher = QmcV2Cipher::new_from_ekey(&ekey).unwrap();
        let audio_hash = "0123456789abcdef0123456789abcdef";
        let mut header = Vec::new();
        header.extend(crate::crypto::kgm::header::KGM_HEADER);
        header.extend(104_u32.to_le_bytes());
        header.extend(5_u32.to_le_bytes());
        header.extend(0_i32.to_le_bytes());
        let mut test = crate::crypto::kgm::header::KGM_TEST_DATA;
        cipher.decrypt(&mut test, 0);
        header.extend(test);
        header.extend([0; 16]);
        header.extend([0; 8]);
        header.extend(32_u32.to_le_bytes());
        header.extend(audio_hash.as_bytes());
        assert_eq!(header.len(), 104);
        let mut plain = b"fLaC".to_vec();
        plain.resize(128, 0);
        let mut encrypted = plain.clone();
        cipher.decrypt(&mut encrypted, 0);
        header.extend(encrypted);
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("v5.kgm");
        std::fs::write(&source, header).unwrap();
        let keys =
            KeySources::with_kgm_ekeys([(audio_hash.to_owned(), ekey)].into_iter().collect());
        let output_path = dir.path().join("out");
        let mut output = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&output_path)
            .unwrap();
        assert_eq!(
            decrypt_to(Kind::Kgm, &source, &mut output, 7, None, &keys).unwrap(),
            "flac"
        );
        assert_eq!(std::fs::read(&output_path).unwrap(), plain);
        let mut missing = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(dir.path().join("missing"))
            .unwrap();
        assert!(
            decrypt_to(
                Kind::Kgm,
                &source,
                &mut missing,
                7,
                None,
                &KeySources::default()
            )
            .is_err()
        );
    }

    fn pc_footer_fixture() -> Vec<u8> {
        let mut fixture = decode_base64(
            "W2R1bW15X2RhdGFdTlVaNmIwbGFkMmJmb0tJT2hNbG5tdER0dGdXZ28ycWVBbVVJY1Q3ckFoNEVtWk9mT1B5bnZyK3grcmlNUHRRVGpPZDd4eU9iTUozTDNiYzd0ZXZlbmtoUHBWUlpRMG1ORVBLTWpLZnhMTVJCdkRrOTFzWWt0OTA4cGpIV1lFOHJWbStzbVYvNlp6SFJTR3dHQ0FIMS9JT0IyU25KNkdRVk1oWjJLRUdRbFBKUnJ6blpaVjFYRHZJSjU0c050OXd2UWllUVJ6amVaYjh6Z2hmWVRQbU9UYVExWk5FTFY0TThsUGVGV29lNWp2VUl5YWtHYjdHZ0ltMEpHR0V4REpzdDVWVTMrRE1ITzhxMzVJdFZJQkJyKy9JRjlhSTdLQ0taRnk1T3V5cVlhejZ3Ti9uZHpiTDU1R2xYODVkd1lMNnBpQkgwNElaeGozWnpDcW5peXNzUjQzRzhWR2QyRHh3NFp5THBuZ2pQcnAvVThvanp5WGU1a09BZjFlZ1BzaXo2bU8rVzBBdGlkKzZqcFp5YjZmUDRRZ2RGc2tjcFZIYWFXMTc3Ry9WWWdkMzRGT2tVb0pxMG5NM1ArbTJ3V3AxWURrZElyc05aTmdOa1VNdUZ5N1lucGplUmQ4S29rRWpRODQyaFNkZXgxbGdKTDkyZCtwa1Q4RGEzajFHdlkwbkhFNjBmT0xpZVg4SU9oYk1KZWVxa3AybmpmaGZDS1FFZk5ELzZJeEpSMkFmWGJSajB5bndtcWFSUGQ3d2g2MEtZNUhHanFucDRqWFZSS0hWUGxLcDVCL3UyNEwrTER6YUNTTEJ4QVhzYjZKWlN2bkVvMFAyMDJ6ckZGVGVtL0VEdFNuNytqdjdkUnBPZkxTdjgrTkdBZ1JyVFpCMUlNQlhjWVBhUnAwZzgvajFmcXlEL001aGNRY3ZwV0lXcldBeW7AAgAA",
        );
        // `ekey_pc_enc_v1.bin` contains a 12-byte detector prefix before the
        // actual footer.  These two bytes preserve that upstream fixture's
        // literal payload after text transport.
        fixture[148..150].copy_from_slice(&[0x2b, 0x5a]);
        fixture
    }

    fn decode_base64(value: &str) -> Vec<u8> {
        fn sextet(byte: u8) -> u8 {
            match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => panic!("invalid fixture base64"),
            }
        }
        let input = value.as_bytes();
        assert_eq!(input.len() % 4, 0);
        let mut out = Vec::with_capacity(input.len() / 4 * 3);
        for chunk in input.as_chunks::<4>().0 {
            let a = sextet(chunk[0]);
            let b = sextet(chunk[1]);
            let c = if chunk[2] == b'=' {
                0
            } else {
                sextet(chunk[2])
            };
            let d = if chunk[3] == b'=' {
                0
            } else {
                sextet(chunk[3])
            };
            out.extend([(a << 2) | (b >> 4), ((b & 0x0f) << 4) | (c >> 2)]);
            if chunk[2] != b'=' {
                out.push(((c & 0x03) << 6) | d);
            }
        }
        out
    }
}
