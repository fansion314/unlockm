//! Bounded standard-audio probing shared by registry and decrypted staging.

pub fn is_standard_audio(header: &[u8]) -> bool {
    um_audio::detect_audio_type(header).is_ok_and(|kind| kind != um_audio::AudioType::Unknown)
}

pub fn extension(header: &[u8]) -> Option<String> {
    um_audio::detect_audio_type(header)
        .ok()
        .and_then(|kind| (kind != um_audio::AudioType::Unknown).then(|| kind.as_str().to_owned()))
}
