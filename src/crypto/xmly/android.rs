use lazy_static::lazy_static;

pub fn derive_table<const N: usize>(init: f64, step: f64) -> [usize; N] {
    debug_assert!(step > 0.0);
    debug_assert!(init > 0.0);

    let mut result = [0usize; N];

    let mut temp = init;
    let mut data = [0f64; N];
    for datum in data.iter_mut() {
        *datum = temp;
        temp = temp * step * (1.0 - temp);
    }

    let mut sorted = data;
    sorted.sort_unstable_by(|a, b| a.total_cmp(b));

    for (item, needle) in result.iter_mut().zip(data) {
        let idx = sorted
            .iter()
            .position(|&x| x == needle)
            .expect("could not find item");
        *item = idx;
        sorted[idx] = -f64::NAN; // values can not be negative, so...
    }

    result
}

lazy_static! {
    static ref TABLE_X2M: [usize; 0x400] = derive_table(0.615243, 3.837465);
    static ref TABLE_X3M: [usize; 0x400] = derive_table(0.726354, 3.948576);
}

pub enum FileType {
    X2M,
    X3M,
}

/// Decrypt the first 0x400 bytes.
pub fn decrypt_android(version: FileType, buffer: &mut [u8; 0x400]) {
    let (content_key, scramble_table) = match version {
        FileType::X2M => (*b"xmlyxmlyxmlyxmlyxmlyxmlyxmlyxmly", &*TABLE_X2M),
        FileType::X3M => (*b"3989d111aad5613940f4fc44b639b292", &*TABLE_X3M),
    };

    let src = *buffer;
    for (i, hdr) in buffer.iter_mut().enumerate() {
        *hdr = src[scramble_table[i % scramble_table.len()]] ^ content_key[i % content_key.len()];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn fixture(name: &str) -> [u8; 0x400] {
        let encoded = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .unwrap();
        base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .unwrap()
            .try_into()
            .unwrap()
    }

    #[test]
    fn decrypts_pinned_x2m_and_x3m_headers() {
        let mut x2m = fixture("x2m_hdr.bin.b64");
        decrypt_android(FileType::X2M, &mut x2m);
        assert_eq!(x2m, fixture("x2m_hdr_plain.bin.b64"));

        let mut x3m = fixture("x3m_hdr.bin.b64");
        decrypt_android(FileType::X3M, &mut x3m);
        assert_eq!(x3m, fixture("x3m_hdr_plain.bin.b64"));
    }
}
