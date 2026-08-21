use crate::crypto::common::{base64_decode, base64_decode_overwrite};
use crate::crypto::xmly::XmlyError;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::BlockDecryptMut;
use aes::{Aes192Dec, Aes256Dec};
use byteorder::{ByteOrder, BE};
use cbc::cipher::KeyIvInit;
use cbc::Decryptor;
use cipher::block_padding::Pkcs7;

type Aes256CbcDec = Decryptor<Aes256Dec>;
type Aes192CbcDec = Decryptor<Aes192Dec>;

fn parse_safe_sync_int(v: u32) -> u32 {
    ((v & 0x7f00_0000) >> 3)
        | ((v & 0x007f_0000) >> 2)
        | ((v & 0x0000_7f00) >> 1)
        | (v & 0x0000_007f)
}

fn from_unicode(buf: &[u8]) -> String {
    let data = buf
        .iter()
        .step_by(2)
        .map_while(|&b| match b {
            0 => None,
            b => Some(b),
        })
        .collect::<Vec<u8>>();
    String::from_utf8_lossy(&data[..]).to_string()
}

pub struct Header {
    pub data_start_offset: usize,
    pub encrypted_header_size: usize,
    stage1_iv: [u8; 0x10],
    stage2_key: [u8; 0x18],
    m4a_header: Vec<u8>,
}

impl Header {
    pub fn from_buffer(buffer: &[u8]) -> Result<Self, XmlyError> {
        if buffer.len() < 10 {
            Err(XmlyError::MetadataTooSmall(10))?;
        }
        if &buffer[0..3] != b"ID3" {
            Err(XmlyError::MetadataMissing)?;
        }

        let header_size = parse_safe_sync_int(BE::read_u32(&buffer[6..10]));
        let data_start_offset = 10 + header_size as usize;

        if buffer.len() < data_start_offset {
            Err(XmlyError::MetadataTooSmall(data_start_offset))?;
        }

        let mut encrypted_header_size = None;
        let mut stage1_iv = None;
        let mut stage2_key = None;
        let mut m4a_header = None;

        let mut offset = 10;
        while offset < data_start_offset {
            // Safety check
            if offset + 10 > buffer.len() {
                break;
            }

            let tag_name: &[u8; 4] = &buffer[offset..offset + 4].try_into().unwrap();
            offset += 4;

            let tag_value_size = BE::read_u32(&buffer[offset..offset + 4]) as usize;
            offset += 4;

            // flags, ignore
            offset += 2;

            if offset + tag_value_size > buffer.len() {
                Err(XmlyError::MetadataTooSmall(offset + tag_value_size))?;
            }
            let data = &buffer[offset + 3..offset + tag_value_size];
            offset += tag_value_size;

            match tag_name {
                b"TSIZ" => {
                    let data = from_unicode(data)
                        .parse::<usize>()
                        .map_err(|_| XmlyError::ParseHeaderSizeError)?;
                    encrypted_header_size = Some(data);
                }
                b"TSRC" | b"TENC" => {
                    let data = hex::decode(from_unicode(data))
                        .map_err(|_| XmlyError::ParseStage1IVError)?
                        .try_into()
                        .map_err(|_| XmlyError::ParseStage1IVError)?;
                    stage1_iv = Some(data)
                }
                b"TSSE" => {
                    let data = base64_decode(from_unicode(data))
                        .map_err(|_| XmlyError::ParseAudioHeaderError)?;
                    m4a_header = Some(data);
                }
                b"TRCK" => {
                    let data = from_unicode(data);
                    let tmp = data.as_bytes();

                    let mut key = *b"123456781234567812345678";
                    key[24 - tmp.len()..].copy_from_slice(tmp);
                    stage2_key = Some(key);
                }
                // ignore unknown tags
                _ => {}
            }
        }

        Ok(Self {
            data_start_offset,
            encrypted_header_size: encrypted_header_size.ok_or(XmlyError::ParseHeaderSizeError)?,
            stage1_iv: stage1_iv.ok_or(XmlyError::ParseStage1IVError)?,
            stage2_key: stage2_key.ok_or(XmlyError::ParseStage2KeyError)?,
            m4a_header: m4a_header.ok_or(XmlyError::ParseAudioHeaderError)?,
        })
    }

    const STAGE1_KEY: [u8; 32] = *b"ximalayaximalayaximalayaximalaya";
    fn decrypt_stage1<'a>(&self, buffer: &'a mut [u8]) -> Result<&'a [u8], XmlyError> {
        let key = GenericArray::from(Self::STAGE1_KEY);
        let iv = GenericArray::from(self.stage1_iv);
        let aes = Aes256CbcDec::new(&key, &iv);

        let len = aes
            .decrypt_padded_mut::<Pkcs7>(buffer)
            .map_err(XmlyError::DecryptStage1Error)?
            .len();
        base64_decode_overwrite(&mut buffer[..len]).map_err(XmlyError::DecryptStage1B64Error)
    }

    fn decrypt_stage2<'a>(&self, buffer: &'a mut [u8]) -> Result<&'a [u8], XmlyError> {
        let key = &self.stage2_key[..24];
        let iv = &self.stage2_key[..16];
        let aes = Aes192CbcDec::new_from_slices(key, iv).map_err(XmlyError::InitStage2Error)?;

        let len = aes
            .decrypt_padded_mut::<Pkcs7>(buffer)
            .map_err(XmlyError::DecryptStage2Error)?
            .len();
        base64_decode_overwrite(&mut buffer[..len]).map_err(XmlyError::DecryptStage2B64Error)
    }

    pub fn decrypt<'a>(&self, buffer: &'a mut [u8]) -> Result<&'a [u8], XmlyError> {
        let len = self.decrypt_stage1(&mut buffer[..])?.len();
        self.decrypt_stage2(&mut buffer[..len])
    }

    pub fn copy_m4a_header(&self) -> Vec<u8> {
        self.m4a_header.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::BlockEncryptMut;
    use base64::Engine;
    use cbc::Encryptor;

    fn encrypt_padded<C>(cipher: C, input: &[u8]) -> Vec<u8>
    where
        C: BlockEncryptMut,
    {
        let mut buffer = input.to_vec();
        let length = buffer.len();
        buffer.resize(length + 16, 0);
        cipher
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, length)
            .unwrap()
            .to_vec()
    }

    fn text_frame(name: &[u8; 4], value: &str) -> Vec<u8> {
        let mut data = vec![0_u8; 3];
        for byte in value.bytes() {
            data.extend_from_slice(&[byte, 0]);
        }
        let mut frame = name.to_vec();
        frame.extend_from_slice(&(data.len() as u32).to_be_bytes());
        frame.extend_from_slice(&[0, 0]);
        frame.extend_from_slice(&data);
        frame
    }

    fn syncsafe(value: usize) -> [u8; 4] {
        [
            ((value >> 21) & 0x7f) as u8,
            ((value >> 14) & 0x7f) as u8,
            ((value >> 7) & 0x7f) as u8,
            (value & 0x7f) as u8,
        ]
    }

    #[test]
    fn hand_built_pc_header_decrypts_bounded_prefix_and_copies_m4a_header() {
        type Aes256CbcEnc = Encryptor<aes::Aes256Enc>;
        type Aes192CbcEnc = Encryptor<aes::Aes192Enc>;

        let iv = [0x11_u8; 16];
        let track_key = *b"abcdefghijklmnopqrstuvwx";
        let plain = b"deterministic ximalaya audio prefix";
        let stage2_input = base64::engine::general_purpose::STANDARD.encode(plain);
        let stage2 = encrypt_padded(
            Aes192CbcEnc::new_from_slices(&track_key, &track_key[..16]).unwrap(),
            stage2_input.as_bytes(),
        );
        let stage1_input = base64::engine::general_purpose::STANDARD.encode(stage2);
        let encrypted = encrypt_padded(
            Aes256CbcEnc::new_from_slices(&Header::STAGE1_KEY, &iv).unwrap(),
            stage1_input.as_bytes(),
        );
        let m4a_header = b"\0\0\0\x20ftypM4A ";

        let mut frames = Vec::new();
        frames.extend(text_frame(b"TSIZ", &encrypted.len().to_string()));
        frames.extend(text_frame(b"TSRC", &hex::encode(iv)));
        frames.extend(text_frame(
            b"TSSE",
            &base64::engine::general_purpose::STANDARD.encode(m4a_header),
        ));
        frames.extend(text_frame(
            b"TRCK",
            std::str::from_utf8(&track_key).unwrap(),
        ));
        let mut header_bytes = b"ID3\x04\0\0".to_vec();
        header_bytes.extend_from_slice(&syncsafe(frames.len()));
        header_bytes.extend_from_slice(&frames);

        let header = Header::from_buffer(&header_bytes).unwrap();
        assert_eq!(header.data_start_offset, header_bytes.len());
        assert_eq!(header.encrypted_header_size, encrypted.len());
        assert_eq!(header.copy_m4a_header(), m4a_header);
        let mut encrypted = encrypted;
        let decrypted = header.decrypt(&mut encrypted).unwrap();
        assert_eq!(decrypted, plain);
        let mut output = header.copy_m4a_header();
        output.extend_from_slice(decrypted);
        assert_eq!(crate::formats::extension(&output).as_deref(), Some("m4a"));
    }
}
