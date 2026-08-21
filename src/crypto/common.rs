//! Small byte-order helpers shared by locally ported crypto modules.
//! Attribution: `lib_um_crypto_rust` revision `5dad838c33c3e4aab198d995160d3fbb9968470f`, MIT OR Apache-2.0.

pub(crate) fn read_be_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().expect("four-byte integer"))
}

pub(crate) fn read_le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().expect("four-byte integer"))
}

pub(crate) fn md5(buffer: impl AsRef<[u8]>) -> [u8; 16] {
    use md5::{Digest, Md5};
    Md5::digest(buffer).into()
}

const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    base64::engine::GeneralPurposeConfig::new()
        .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
);

pub(crate) fn base64_decode(data: impl AsRef<[u8]>) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    BASE64_ENGINE.decode(data)
}

pub(crate) fn base64_decode_overwrite(data: &mut [u8]) -> Result<&[u8], base64::DecodeError> {
    let decoded = base64_decode(&*data)?;
    let len = decoded.len();
    data[..len].copy_from_slice(&decoded);
    Ok(&data[..len])
}
