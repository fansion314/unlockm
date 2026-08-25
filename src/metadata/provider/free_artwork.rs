//! Composite free artwork source for MusicBrainz candidates.
use super::{
    OnlineMetadataError, coverartarchive::CoverArtArchiveClient, theaudiodb::TheAudioDbClient,
};
use url::Url;

#[derive(Clone)]
pub struct FreeArtworkClient {
    cover_art_archive: CoverArtArchiveClient,
    the_audio_db: TheAudioDbClient,
}

impl FreeArtworkClient {
    pub fn new() -> Result<Self, OnlineMetadataError> {
        Ok(Self {
            cover_art_archive: CoverArtArchiveClient::new()?,
            the_audio_db: TheAudioDbClient::new()?,
        })
    }

    #[cfg(test)]
    fn with_clients(
        cover_art_archive: CoverArtArchiveClient,
        the_audio_db: TheAudioDbClient,
    ) -> Self {
        Self {
            cover_art_archive,
            the_audio_db,
        }
    }

    pub fn cover_url(
        &self,
        release_group_id: Option<&str>,
        artist: &str,
        album: &str,
    ) -> Result<Option<Url>, OnlineMetadataError> {
        match self.cover_art_archive.cover_url(release_group_id) {
            Ok(Some(url)) => return Ok(Some(url)),
            Ok(None) => {}
            Err(error @ OnlineMetadataError::InvalidInput) => return Err(error),
            Err(_) => {
                // A bounded CAA outage is recoverable if TheAudioDB has the
                // same album. No provider URL crosses this adapter boundary.
            }
        }
        self.the_audio_db.cover_url(release_group_id, artist, album)
    }

    pub fn fetch_cover(&self, url: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
        if self.cover_art_archive.supports(url) {
            self.cover_art_archive.fetch_cover(url)
        } else {
            self.the_audio_db.fetch_cover(url)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    fn one_response(status: &'static str, headers: String, body: &'static str) -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        });
        Url::parse(&format!("http://{address}/")).unwrap()
    }

    fn two_responses(first: &'static str, second: &'static str) -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            for body in [first, second] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 4096];
                let _ = stream.read(&mut request);
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Url::parse(&format!("http://{address}/")).unwrap()
    }

    #[test]
    fn caa_match_does_not_dispatch_theaudiodb() {
        let target = "http://127.0.0.1:9/caa.jpg";
        let caa = CoverArtArchiveClient::with_test_root(one_response(
            "307 Temporary Redirect",
            format!("Location: {target}\r\n"),
            "",
        ));
        let tadb_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let tadb_address = tadb_listener.local_addr().unwrap();
        tadb_listener.set_nonblocking(true).unwrap();
        let client = FreeArtworkClient::with_clients(
            caa,
            TheAudioDbClient::with_test_api(
                Url::parse(&format!("http://{tadb_address}/")).unwrap(),
            ),
        );
        assert_eq!(
            client
                .cover_url(
                    Some("11111111-1111-1111-1111-111111111111"),
                    "Artist",
                    "Album",
                )
                .unwrap()
                .unwrap()
                .as_str(),
            target
        );
        assert!(tadb_listener.accept().is_err());
    }

    #[test]
    fn caa_404_uses_theaudiodb_null_then_raw_text_fallback() {
        let caa =
            CoverArtArchiveClient::with_test_root(one_response("404 Not Found", String::new(), ""));
        let tadb = TheAudioDbClient::with_test_api(two_responses(
            r#"{"album":null}"#,
            r#"{"album":[{"strArtist":"Daft Punk","strAlbum":"Random Access Memories","strAlbumThumb":"https://r2.theaudiodb.com/ram.jpg"}]}"#,
        ));
        let client = FreeArtworkClient::with_clients(caa, tadb);
        assert_eq!(
            client
                .cover_url(
                    Some("11111111-1111-1111-1111-111111111111"),
                    "Daft Punk",
                    "Random Access Memories",
                )
                .unwrap()
                .unwrap()
                .as_str(),
            "https://r2.theaudiodb.com/ram.jpg"
        );
    }

    #[test]
    fn fetch_dispatches_non_caa_url_to_theaudiodb_validator() {
        let bytes = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let expected = bytes.clone();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            );
            let _ = stream.write_all(&bytes);
        });
        let image = Url::parse(&format!("http://{address}/cover.png")).unwrap();
        let caa = CoverArtArchiveClient::new().unwrap();
        let tadb = TheAudioDbClient::with_test_image(image.clone());
        assert_eq!(
            FreeArtworkClient::with_clients(caa, tadb)
                .fetch_cover(&image)
                .unwrap(),
            expected
        );
    }

    #[test]
    fn no_source_is_a_clean_none() {
        let client = FreeArtworkClient::with_clients(
            CoverArtArchiveClient::with_test_root(one_response("404 Not Found", String::new(), "")),
            TheAudioDbClient::with_test_api(two_responses(
                r#"{"album":null}"#,
                r#"{"album":null}"#,
            )),
        );
        assert!(
            client
                .cover_url(
                    Some("11111111-1111-1111-1111-111111111111"),
                    "Missing Artist",
                    "Missing Album",
                )
                .unwrap()
                .is_none()
        );
    }
}
