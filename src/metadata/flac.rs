use super::model::Metadata;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

const STREAMINFO: u8 = 0;
const PADDING: u8 = 1;
const VORBIS_COMMENT: u8 = 4;
const PICTURE: u8 = 6;
const COVER_FRONT: u32 = 3;
const MAX_METADATA_BYTES: usize = 32 * 1024 * 1024;
const SHIFT_BUFFER: usize = 1024 * 1024;

struct Block {
    kind: u8,
    data: Vec<u8>,
}

pub(super) fn write_in_place(
    path: &Path,
    metadata: &Metadata,
    cover: Option<&[u8]>,
) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    let mut magic = [0_u8; 4];
    file.read_exact(&mut magic)
        .map_err(|error| error.to_string())?;
    if &magic != b"fLaC" {
        return Err("not a FLAC stream".into());
    }

    let (blocks, old_audio_start) = read_blocks(&mut file)?;
    let mut retained = Vec::new();
    let mut vendor = b"unlockm".to_vec();
    let mut comments = Vec::new();
    let mut pictures = Vec::new();
    for block in blocks {
        match block.kind {
            PADDING => {}
            VORBIS_COMMENT => {
                let parsed = parse_comments(&block.data)?;
                vendor = parsed.0;
                comments.extend(parsed.1);
            }
            PICTURE => {
                if cover.is_none() || picture_type(&block.data)? != COVER_FRONT {
                    pictures.push(block);
                }
            }
            _ => retained.push(block),
        }
    }
    if retained.first().map(|block| block.kind) != Some(STREAMINFO) {
        return Err("FLAC STREAMINFO must be the first metadata block".into());
    }

    comments.retain(|entry| {
        let key = entry.split(|byte| *byte == b'=').next().unwrap_or_default();
        !matches_ignore_ascii_case(key, b"TITLE")
            && !matches_ignore_ascii_case(key, b"ALBUM")
            && !matches_ignore_ascii_case(key, b"ARTIST")
    });
    if let Some(value) = &metadata.title {
        comments.push(format!("TITLE={}", value.text).into_bytes());
    }
    if let Some(value) = &metadata.album {
        comments.push(format!("ALBUM={}", value.text).into_bytes());
    }
    if let Some(value) = &metadata.artists {
        comments.push(format!("ARTIST={}", value.text).into_bytes());
    }

    retained.insert(
        1,
        Block {
            kind: VORBIS_COMMENT,
            data: encode_comments(&vendor, &comments)?,
        },
    );
    retained.extend(pictures);
    if let Some(bytes) = cover {
        retained.push(Block {
            kind: PICTURE,
            data: encode_cover(bytes)?,
        });
    }

    let old_metadata_len = old_audio_start - 4;
    let base_len = encoded_len(&retained)?;
    let target_len = if base_len <= old_metadata_len && old_metadata_len - base_len >= 4 {
        retained.push(Block {
            kind: PADDING,
            data: vec![0; old_metadata_len - base_len - 4],
        });
        old_metadata_len
    } else {
        base_len
    };
    let encoded = encode_blocks(&retained)?;
    debug_assert_eq!(encoded.len(), target_len);

    let new_audio_start = 4 + encoded.len();
    shift_audio(&mut file, old_audio_start as u64, new_audio_start as u64)?;
    file.seek(SeekFrom::Start(4))
        .map_err(|error| error.to_string())?;
    file.write_all(&encoded)
        .map_err(|error| error.to_string())?;
    file.flush().map_err(|error| error.to_string())
}

fn read_blocks(file: &mut File) -> Result<(Vec<Block>, usize), String> {
    let mut blocks = Vec::new();
    let mut total = 0_usize;
    loop {
        let mut header = [0_u8; 4];
        file.read_exact(&mut header)
            .map_err(|error| error.to_string())?;
        let last = header[0] & 0x80 != 0;
        let kind = header[0] & 0x7f;
        let length = u32::from_be_bytes([0, header[1], header[2], header[3]]) as usize;
        total = total
            .checked_add(4 + length)
            .filter(|total| *total <= MAX_METADATA_BYTES)
            .ok_or("FLAC metadata exceeds 32 MiB")?;
        let mut data = vec![0; length];
        file.read_exact(&mut data)
            .map_err(|error| error.to_string())?;
        blocks.push(Block { kind, data });
        if last {
            return Ok((blocks, 4 + total));
        }
    }
}

fn parse_comments(data: &[u8]) -> Result<(Vec<u8>, Vec<Vec<u8>>), String> {
    let mut cursor = 0_usize;
    let vendor_length = read_le_u32(data, &mut cursor)? as usize;
    let vendor = take(data, &mut cursor, vendor_length)?.to_vec();
    let count = read_le_u32(data, &mut cursor)? as usize;
    let mut comments = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        let length = read_le_u32(data, &mut cursor)? as usize;
        comments.push(take(data, &mut cursor, length)?.to_vec());
    }
    Ok((vendor, comments))
}

fn encode_comments(vendor: &[u8], comments: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    push_le_len(&mut output, vendor.len())?;
    output.extend_from_slice(vendor);
    push_le_len(&mut output, comments.len())?;
    for comment in comments {
        push_le_len(&mut output, comment.len())?;
        output.extend_from_slice(comment);
    }
    Ok(output)
}

fn encode_cover(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mime = cover_mime(bytes);
    let mut output = Vec::new();
    output.extend_from_slice(&COVER_FRONT.to_be_bytes());
    push_be_len(&mut output, mime.len())?;
    output.extend_from_slice(mime);
    output.extend_from_slice(&0_u32.to_be_bytes()); // description
    output.extend_from_slice(&0_u32.to_be_bytes()); // width
    output.extend_from_slice(&0_u32.to_be_bytes()); // height
    output.extend_from_slice(&0_u32.to_be_bytes()); // color depth
    output.extend_from_slice(&0_u32.to_be_bytes()); // indexed colors
    push_be_len(&mut output, bytes.len())?;
    output.extend_from_slice(bytes);
    Ok(output)
}

fn cover_mime(bytes: &[u8]) -> &'static [u8] {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        b"image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        b"image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        b"image/gif"
    } else if bytes.starts_with(b"BM") {
        b"image/bmp"
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        b"image/webp"
    } else {
        b"application/octet-stream"
    }
}

fn picture_type(data: &[u8]) -> Result<u32, String> {
    data.get(..4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_be_bytes)
        .ok_or_else(|| "truncated FLAC picture block".into())
}

fn encoded_len(blocks: &[Block]) -> Result<usize, String> {
    blocks.iter().try_fold(0_usize, |total, block| {
        if block.data.len() > 0x00ff_ffff {
            return Err("FLAC metadata block exceeds 24-bit length".into());
        }
        total
            .checked_add(4 + block.data.len())
            .ok_or_else(|| "FLAC metadata length overflow".into())
    })
}

fn encode_blocks(blocks: &[Block]) -> Result<Vec<u8>, String> {
    let mut output = Vec::with_capacity(encoded_len(blocks)?);
    for (index, block) in blocks.iter().enumerate() {
        let mut kind = block.kind;
        if index + 1 == blocks.len() {
            kind |= 0x80;
        }
        output.push(kind);
        let length = block.data.len() as u32;
        let bytes = length.to_be_bytes();
        output.extend_from_slice(&bytes[1..]);
        output.extend_from_slice(&block.data);
    }
    Ok(output)
}

fn shift_audio(file: &mut File, from: u64, to: u64) -> Result<(), String> {
    if from == to {
        return Ok(());
    }
    let length = file.metadata().map_err(|error| error.to_string())?.len();
    let mut buffer = vec![0_u8; SHIFT_BUFFER];
    if to > from {
        let delta = to - from;
        file.set_len(length + delta)
            .map_err(|error| error.to_string())?;
        let mut end = length;
        while end > from {
            let start = from.max(end.saturating_sub(buffer.len() as u64));
            let count = (end - start) as usize;
            file.seek(SeekFrom::Start(start))
                .map_err(|error| error.to_string())?;
            file.read_exact(&mut buffer[..count])
                .map_err(|error| error.to_string())?;
            file.seek(SeekFrom::Start(start + delta))
                .map_err(|error| error.to_string())?;
            file.write_all(&buffer[..count])
                .map_err(|error| error.to_string())?;
            end = start;
        }
    } else {
        let delta = from - to;
        let mut start = from;
        while start < length {
            let count = ((length - start) as usize).min(buffer.len());
            file.seek(SeekFrom::Start(start))
                .map_err(|error| error.to_string())?;
            file.read_exact(&mut buffer[..count])
                .map_err(|error| error.to_string())?;
            file.seek(SeekFrom::Start(start - delta))
                .map_err(|error| error.to_string())?;
            file.write_all(&buffer[..count])
                .map_err(|error| error.to_string())?;
            start += count as u64;
        }
        file.set_len(length - delta)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn read_le_u32(data: &[u8], cursor: &mut usize) -> Result<u32, String> {
    let bytes: [u8; 4] = take(data, cursor, 4)?
        .try_into()
        .map_err(|_| "truncated Vorbis comment integer")?;
    Ok(u32::from_le_bytes(bytes))
}

fn take<'a>(data: &'a [u8], cursor: &mut usize, length: usize) -> Result<&'a [u8], String> {
    let end = cursor
        .checked_add(length)
        .ok_or("metadata cursor overflow")?;
    let bytes = data.get(*cursor..end).ok_or("truncated FLAC metadata")?;
    *cursor = end;
    Ok(bytes)
}

fn push_le_len(output: &mut Vec<u8>, length: usize) -> Result<(), String> {
    let length = u32::try_from(length).map_err(|_| "Vorbis field is too large")?;
    output.extend_from_slice(&length.to_le_bytes());
    Ok(())
}

fn push_be_len(output: &mut Vec<u8>, length: usize) -> Result<(), String> {
    let length = u32::try_from(length).map_err(|_| "FLAC picture field is too large")?;
    output.extend_from_slice(&length.to_be_bytes());
    Ok(())
}

fn matches_ignore_ascii_case(left: &[u8], right: &[u8]) -> bool {
    left.eq_ignore_ascii_case(right)
}
