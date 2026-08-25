pub mod musicbrainz;
pub mod qq;
pub mod spotify;
pub mod theaudiodb;

use serde::{Deserialize, Serialize};

/// User-selected online search candidates are intentionally separate from the
/// automatic metadata merge model. They contain no provider payload, token or
/// URL, and can only be applied to an editor draft by the desktop host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OnlineMetadataProviderKind {
    MusicBrainz,
    Spotify,
}
impl OnlineMetadataProviderKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MusicBrainz => "musicBrainz",
            Self::Spotify => "spotify",
        }
    }
}

/// Safe provider descriptors let the desktop order no-login sources ahead of
/// optional account-backed sources without receiving a URL or auth material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineMetadataProviderCapability {
    pub provider: OnlineMetadataProviderKind,
    pub primary_order: u8,
    pub requires_authorization: bool,
    pub artwork_source: Option<&'static str>,
    pub attribution: &'static str,
}
pub const ONLINE_METADATA_PROVIDER_CAPABILITIES: [OnlineMetadataProviderCapability; 2] = [
    OnlineMetadataProviderCapability {
        provider: OnlineMetadataProviderKind::MusicBrainz,
        primary_order: 0,
        requires_authorization: false,
        artwork_source: Some("TheAudioDB"),
        attribution: "MusicBrainz; artwork on demand from TheAudioDB",
    },
    OnlineMetadataProviderCapability {
        provider: OnlineMetadataProviderKind::Spotify,
        primary_order: 1,
        requires_authorization: true,
        artwork_source: Some("Spotify"),
        attribution: "Spotify",
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineMetadataCandidate {
    pub provider: OnlineMetadataProviderKind,
    /// UI capability metadata, never a credential or remote URL.
    pub requires_authorization: bool,
    pub artwork_source: Option<&'static str>,
    pub attribution: &'static str,
    pub item_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: u32,
    /// An opaque, short-lived desktop-owned handle. Providers never receive
    /// this field and the webview never receives an image URL.
    pub cover_handle: Option<String>,
    /// Rust-only artwork lookup hint; this is an identifier, never a URL.
    #[serde(skip_serializing)]
    pub release_group_id: Option<String>,
    /// MusicBrainz artist-credit joinphrases are useful only for the
    /// Rust-owned artwork fallback. The editor gets clean individual names.
    #[serde(skip_serializing)]
    pub credited_artist: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnlineMetadataError {
    NotConfigured,
    CredentialUnavailable,
    ReauthorizationRequired,
    AuthorizationBusy,
    AuthorizationCancelled,
    AuthorizationFailed,
    InvalidInput,
    RateLimited { retry_after_secs: u64 },
    RequestTimeout,
    Unavailable,
    MalformedResponse,
}
impl OnlineMetadataError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::CredentialUnavailable => "credential_unavailable",
            Self::ReauthorizationRequired => "reauthorization_required",
            Self::AuthorizationBusy => "authorization_busy",
            Self::AuthorizationCancelled => "authorization_cancelled",
            Self::AuthorizationFailed => "authorization_failed",
            Self::InvalidInput => "invalid_input",
            Self::RateLimited { .. } => "rate_limited",
            Self::RequestTimeout => "request_timeout",
            Self::Unavailable => "provider_unavailable",
            Self::MalformedResponse => "provider_malformed_response",
        }
    }
}

/// Minimal extension seam. Protocol-specific OAuth/token implementation stays
/// in its adapter; the desktop consumes only this safe candidate/error shape.
pub trait OnlineMetadataProvider {
    fn kind(&self) -> OnlineMetadataProviderKind;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_musicbrainz_capability_is_the_default_and_carries_attribution() {
        let musicbrainz = ONLINE_METADATA_PROVIDER_CAPABILITIES
            .iter()
            .find(|capability| capability.provider == OnlineMetadataProviderKind::MusicBrainz)
            .unwrap();
        assert_eq!(musicbrainz.primary_order, 0);
        assert!(!musicbrainz.requires_authorization);
        assert_eq!(musicbrainz.artwork_source, Some("TheAudioDB"));
        assert!(musicbrainz.attribution.contains("MusicBrainz"));
        assert!(musicbrainz.attribution.contains("TheAudioDB"));
    }
}
