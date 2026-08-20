use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum Provenance {
    #[default]
    FilenameHint,
    ExistingTag,
    VerifiedSearch,
    ExactProvider,
    Container,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataValue {
    pub text: String,
    pub source: Provenance,
    pub confidence: u8,
}
impl MetadataValue {
    pub fn new(text: impl Into<String>, source: Provenance) -> Self {
        Self {
            text: text.into(),
            source,
            confidence: 100,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Cover {
    pub bytes: Option<Vec<u8>>,
    pub url: Option<String>,
    pub source: Provenance,
}
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub title: Option<MetadataValue>,
    pub album: Option<MetadataValue>,
    pub artists: Option<MetadataValue>,
    pub duration_ms: Option<u32>,
    pub provider_id: Option<String>,
    pub cover: Option<Cover>,
}
impl Metadata {
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.album.is_none()
            && self.artists.is_none()
            && self.cover.is_none()
    }
    pub fn with_provenance(mut self, source: Provenance) -> Self {
        for value in [&mut self.title, &mut self.album, &mut self.artists]
            .into_iter()
            .flatten()
        {
            value.source = source;
        }
        if let Some(cover) = &mut self.cover {
            cover.source = source;
        }
        self
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub title: String,
    pub artists: String,
    pub duration_ms: u32,
    pub metadata: Metadata,
}
