use super::model::{Candidate, Metadata, MetadataValue, Provenance};
use unicode_normalization::UnicodeNormalization;
pub fn normalize(value: &str) -> String {
    value
        .nfkc()
        .flat_map(char::to_lowercase)
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}
fn choose(a: Option<MetadataValue>, b: Option<MetadataValue>) -> Option<MetadataValue> {
    match (a, b) {
        (Some(a), Some(b)) if a.source >= b.source => Some(a),
        (Some(_), Some(b)) => Some(b),
        (a, None) => a,
        (None, b) => b,
    }
}
pub fn merge(existing: Metadata, incoming: Metadata) -> Metadata {
    Metadata {
        title: choose(existing.title, incoming.title),
        album: choose(existing.album, incoming.album),
        artists: choose(existing.artists, incoming.artists),
        duration_ms: incoming.duration_ms.or(existing.duration_ms),
        provider_id: incoming.provider_id.or(existing.provider_id),
        cover: match (existing.cover, incoming.cover) {
            // Artwork is intentionally not governed by textual provenance.
            // A picture already embedded in the decrypted audio is the most
            // trustworthy representation of that exact file.
            (Some(a), Some(_)) if a.source == Provenance::ExistingTag => Some(a),
            (Some(a), Some(b)) if cover_rank(a.source) >= cover_rank(b.source) => Some(a),
            (Some(_), Some(b)) => Some(b),
            (a, None) => a,
            (None, b) => b,
        },
    }
}
fn cover_rank(source: Provenance) -> u8 {
    match source {
        Provenance::ExistingTag => 4,
        Provenance::Container => 3,
        Provenance::ExactProvider => 2,
        Provenance::VerifiedSearch => 1,
        Provenance::FilenameHint => 0,
    }
}
pub fn verified_search(
    title: &str,
    artists: &str,
    duration_ms: Option<u32>,
    candidates: &[Candidate],
) -> Option<Metadata> {
    let title = normalize(title);
    let artists = normalize(artists);
    let mut ranked = candidates
        .iter()
        .filter_map(|c| {
            if normalize(&c.title) != title || !normalize(&c.artists).contains(&artists) {
                return None;
            }
            let ok = duration_ms.map_or(true, |d| c.duration_ms.abs_diff(d) <= 2000);
            ok.then_some((
                c,
                100 - duration_ms.map_or(0, |d| c.duration_ms.abs_diff(d) / 100) as i32,
            ))
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    match ranked.as_slice() {
        [(c, score), ..]
            if *score >= 80 && ranked.get(1).map_or(true, |(_, next)| score - next >= 10) =>
        {
            Some(c.metadata.clone())
        }
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Provenance;
    #[test]
    fn precedence_and_ambiguity_are_conservative() {
        let a = Metadata {
            title: Some(MetadataValue::new("tag", Provenance::ExistingTag)),
            ..Default::default()
        };
        let b = Metadata {
            title: Some(MetadataValue::new("file", Provenance::FilenameHint)),
            ..Default::default()
        };
        assert_eq!(merge(a, b).title.unwrap().text, "tag");
        let c = Candidate {
            title: "Song".into(),
            artists: "Artist".into(),
            duration_ms: 1000,
            metadata: Metadata::default(),
        };
        assert!(verified_search("song", "artist", Some(1000), &[c.clone(), c]).is_none());
    }
    #[test]
    fn existing_art_is_never_replaced_by_container_or_network_art() {
        let existing = Metadata {
            cover: Some(crate::metadata::model::Cover {
                bytes: Some(vec![1]),
                url: None,
                source: Provenance::ExistingTag,
            }),
            ..Default::default()
        };
        let container = Metadata {
            cover: Some(crate::metadata::model::Cover {
                bytes: Some(vec![2]),
                url: None,
                source: Provenance::Container,
            }),
            ..Default::default()
        };
        assert_eq!(
            merge(existing, container).cover.unwrap().bytes,
            Some(vec![1])
        );
    }
}
