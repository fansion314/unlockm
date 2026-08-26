//! TheAudioDB is queried only while an explicit cover request is in flight.
use super::{OnlineMetadataError, musicbrainz::read_capped};
use reqwest::{StatusCode, blocking::Client, redirect::Policy};
use serde::Deserialize;
use std::time::Duration;
use unicode_normalization::UnicodeNormalization;
use url::{Host, Url};

pub const API_HOST: &str = "www.theaudiodb.com";
pub const IMAGE_HOST: &str = "r2.theaudiodb.com";
pub const PUBLIC_KEY: &str = "123";
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_LOOKUP_SCALARS: usize = 200;
const MAX_LOOKUP_URL_BYTES: usize = 4096;

#[derive(Clone)]
pub struct TheAudioDbClient {
    client: Client,
    api: Url,
    #[cfg(test)]
    local_transport: bool,
}
impl TheAudioDbClient {
    pub fn new() -> Result<Self, OnlineMetadataError> {
        Ok(Self {
            client: Client::builder()
                .redirect(Policy::none())
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(request_error)?,
            api: Url::parse(&format!("https://{API_HOST}/api/v1/json/{PUBLIC_KEY}/"))
                .expect("constant URL"),
            #[cfg(test)]
            local_transport: false,
        })
    }
    #[cfg(test)]
    pub(super) fn with_test_api(api: Url) -> Self {
        Self {
            client: Client::builder()
                .redirect(Policy::none())
                .timeout(Duration::from_secs(1))
                .build()
                .unwrap(),
            api,
            local_transport: true,
        }
    }
    #[cfg(test)]
    pub(super) fn with_test_image(image: Url) -> Self {
        Self::with_test_api(image)
    }
    #[cfg(test)]
    fn production_client_with_api(api: Url) -> Self {
        Self {
            client: Client::builder()
                .redirect(Policy::none())
                .timeout(Duration::from_secs(1))
                .build()
                .unwrap(),
            api,
            // Deliberately retain production URL validation. The test server
            // below must never observe a request for a spoofed API host.
            local_transport: false,
        }
    }
    pub fn cover_url(
        &self,
        group: Option<&str>,
        artist: &str,
        album: &str,
    ) -> Result<Option<Url>, OnlineMetadataError> {
        if let Some(group) = group.filter(|value| super::musicbrainz::is_mbid(value))
            && let Some(url) = self.lookup("album-mb.php", &[("i", group)])?
        {
            return Ok(Some(url));
        }
        let artist_query = bounded_lookup_value(artist)?;
        let album_query = bounded_lookup_value(album)?;
        if artist_query.is_empty() || album_query.is_empty() {
            return Ok(None);
        }
        let artist_key = normalize(&artist_query);
        let album_key = normalize(&album_query);
        let exact = self
            .lookup_all(
                "searchalbum.php",
                &[("s", &artist_query), ("a", &album_query)],
            )?
            .into_iter()
            .filter(|entry| {
                normalize(&entry.artist) == artist_key && normalize(&entry.album) == album_key
            })
            .collect::<Vec<_>>();
        Ok((exact.len() == 1)
            .then(|| exact.into_iter().next().unwrap().image())
            .flatten())
    }
    pub fn fetch_cover(&self, url: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
        #[cfg(test)]
        let url = if self.local_transport {
            url.clone()
        } else {
            trusted(url, IMAGE_HOST)?
        };
        #[cfg(not(test))]
        let url = trusted(url, IMAGE_HOST)?;
        let response = self.client.get(url).send().map_err(request_error)?;
        if !response.status().is_success() {
            return Err(OnlineMetadataError::Unavailable);
        }
        let bytes = read_capped(response, crate::metadata::tags::MAX_COVER_BYTES)?;
        crate::metadata::tags::validate_cover(&bytes)
            .map_err(|_| OnlineMetadataError::MalformedResponse)?;
        Ok(bytes)
    }
    fn lookup(
        &self,
        path: &str,
        pairs: &[(&str, &str)],
    ) -> Result<Option<Url>, OnlineMetadataError> {
        Ok(self
            .lookup_all(path, pairs)?
            .into_iter()
            .next()
            .and_then(Album::image))
    }
    fn lookup_all(
        &self,
        path: &str,
        pairs: &[(&str, &str)],
    ) -> Result<Vec<Album>, OnlineMetadataError> {
        #[cfg(test)]
        let mut url = if self.local_transport {
            self.api.clone()
        } else {
            trusted(&self.api, API_HOST)?
        };
        #[cfg(not(test))]
        let mut url = trusted(&self.api, API_HOST)?;
        url.set_path(&format!("/api/v1/json/{PUBLIC_KEY}/{path}"));
        url.query_pairs_mut().extend_pairs(pairs.iter().copied());
        if url.as_str().len() > MAX_LOOKUP_URL_BYTES {
            return Err(OnlineMetadataError::InvalidInput);
        }
        let response = self
            .client
            .get(url)
            .header("accept", "application/json")
            .send()
            .map_err(request_error)?;
        match response.status() {
            StatusCode::OK => {
                serde_json::from_slice::<Response>(&read_capped(response, MAX_RESPONSE_BYTES)?)
                    .map(|body| body.album.unwrap_or_default())
                    .map_err(|_| OnlineMetadataError::MalformedResponse)
            }
            _ => Err(OnlineMetadataError::Unavailable),
        }
    }
}
#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    album: Option<Vec<Album>>,
}
#[derive(Deserialize)]
struct Album {
    #[serde(rename = "strArtist", default)]
    artist: String,
    #[serde(rename = "strAlbum", default)]
    album: String,
    #[serde(rename = "strAlbumThumbHQ")]
    hq: Option<String>,
    #[serde(rename = "strAlbumThumb")]
    thumb: Option<String>,
}
impl Album {
    fn image(self) -> Option<Url> {
        self.hq.into_iter().chain(self.thumb).find_map(|value| {
            Url::parse(&value)
                .ok()
                .and_then(|url| trusted(&url, IMAGE_HOST).ok())
        })
    }
}
pub fn normalize(value: &str) -> String {
    value
        .nfkc()
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}
fn bounded_lookup_value(value: &str) -> Result<String, OnlineMetadataError> {
    let value = value.trim();
    if value.chars().count() > MAX_LOOKUP_SCALARS {
        return Err(OnlineMetadataError::InvalidInput);
    }
    Ok(value.to_owned())
}
fn trusted(url: &Url, host: &str) -> Result<Url, OnlineMetadataError> {
    (url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.host(), Some(Host::Domain(actual)) if actual.eq_ignore_ascii_case(host)))
    .then_some(url.clone())
    .ok_or(OnlineMetadataError::InvalidInput)
}
fn request_error(error: reqwest::Error) -> OnlineMetadataError {
    if error.is_timeout() {
        OnlineMetadataError::RequestTimeout
    } else {
        OnlineMetadataError::Unavailable
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
    fn api_response(body: &'static str) -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let count = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..count]);
            assert!(request.contains("album-mb.php") || request.contains("searchalbum.php"));
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        });
        Url::parse(&format!("http://{address}/")).unwrap()
    }
    fn api_responses(bodies: [&'static str; 2]) -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            for body in bodies {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 4096];
                let count = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]);
                assert!(request.contains("album-mb.php") || request.contains("searchalbum.php"));
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Url::parse(&format!("http://{address}/")).unwrap()
    }
    fn api_request(body: &'static str) -> (Url, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sent, received) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let count = stream.read(&mut request).unwrap();
            let _ = sent.send(String::from_utf8_lossy(&request[..count]).into_owned());
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        });
        (Url::parse(&format!("http://{address}/")).unwrap(), received)
    }
    fn api_redirect() -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1/next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
        });
        Url::parse(&format!("http://{address}/")).unwrap()
    }
    #[test]
    fn normalized_fallback_and_exact_host_are_strict() {
        assert_eq!(normalize("Beyoncé — Album!"), "beyoncéalbum");
        assert!(
            trusted(
                &Url::parse("https://r2.theaudiodb.com/a.jpg").unwrap(),
                IMAGE_HOST
            )
            .is_ok()
        );
        assert!(
            trusted(
                &Url::parse("https://www.theaudiodb.com/a.jpg").unwrap(),
                IMAGE_HOST
            )
            .is_err()
        );
    }
    #[test]
    fn local_album_mbid_response_prefers_exact_hq_artwork() {
        let client = TheAudioDbClient::with_test_api(api_response(
            r#"{"album":[{"strArtist":"A","strAlbum":"B","strAlbumThumbHQ":"https://r2.theaudiodb.com/hq.jpg","strAlbumThumb":"https://r2.theaudiodb.com/fallback.jpg"}]}"#,
        ));
        assert_eq!(
            client
                .cover_url(
                    Some("11111111-1111-1111-1111-111111111111"),
                    "ignored",
                    "ignored"
                )
                .unwrap()
                .unwrap()
                .as_str(),
            "https://r2.theaudiodb.com/hq.jpg"
        );
    }
    #[test]
    fn real_null_mbid_response_falls_back_to_one_exact_normalized_album() {
        let client = TheAudioDbClient::with_test_api(api_responses([
            r#"{"album":null}"#,
            r#"{"album":[{"strArtist":"Beyoncé","strAlbum":"The Album!","strAlbumThumb":"https://r2.theaudiodb.com/a.jpg"}]}"#,
        ]));
        assert_eq!(
            client
                .cover_url(
                    Some("11111111-1111-1111-1111-111111111111"),
                    "Beyoncé",
                    "The Album!"
                )
                .unwrap()
                .unwrap()
                .as_str(),
            "https://r2.theaudiodb.com/a.jpg"
        );
    }
    #[test]
    fn ambiguous_or_empty_fallback_returns_no_cover() {
        let ambiguous = r#"{"album":[{"strArtist":"A","strAlbum":"B","strAlbumThumb":"https://r2.theaudiodb.com/1.jpg"},{"strArtist":"A","strAlbum":"B","strAlbumThumb":"https://r2.theaudiodb.com/2.jpg"}]}"#;
        let client = TheAudioDbClient::with_test_api(api_response(ambiguous));
        assert!(client.cover_url(None, "A", "B").unwrap().is_none());
        let client = TheAudioDbClient::with_test_api(api_response(r#"{"album":null}"#));
        assert!(client.cover_url(None, "A", "B").unwrap().is_none());
    }
    #[test]
    fn fallback_bounds_scalars_and_sends_url_encoded_original_query_values() {
        let client = TheAudioDbClient::with_test_api(Url::parse("http://127.0.0.1:9/").unwrap());
        assert!(matches!(
            client.cover_url(None, &"é".repeat(MAX_LOOKUP_SCALARS + 1), "album"),
            Err(OnlineMetadataError::InvalidInput)
        ));
        let (endpoint, received) = api_request(r#"{"album":null}"#);
        let client = TheAudioDbClient::with_test_api(endpoint);
        assert!(
            client
                .cover_url(None, "Beyoncé", "The Album!")
                .unwrap()
                .is_none()
        );
        let request = received.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(request.contains("searchalbum.php"));
        assert!(request.contains("s=Beyonc%C3%A9"));
        assert!(request.contains("a=The+Album%21"));
    }
    #[test]
    fn high_byte_values_within_scalar_limit_are_rejected_when_encoded_url_exceeds_cap() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let value = "\u{20000}".repeat(MAX_LOOKUP_SCALARS);
        let client =
            TheAudioDbClient::with_test_api(Url::parse(&format!("http://{address}/")).unwrap());
        assert!(matches!(
            client.cover_url(None, &value, &value),
            Err(OnlineMetadataError::InvalidInput)
        ));
        assert!(listener.accept().is_err());
    }
    #[test]
    fn local_api_redirect_is_not_followed() {
        let client = TheAudioDbClient::with_test_api(api_redirect());
        assert!(matches!(
            client.cover_url(None, "A", "B"),
            Err(OnlineMetadataError::Unavailable)
        ));
    }
    #[test]
    fn local_valid_png_is_validated_before_returning() {
        let bytes = STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=").unwrap();
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
        let url = Url::parse(&format!("http://{address}/cover.png")).unwrap();
        assert_eq!(
            TheAudioDbClient::with_test_image(url.clone())
                .fetch_cover(&url)
                .unwrap(),
            expected
        );
    }
    #[test]
    fn malformed_cover_bytes_are_rejected_by_existing_cover_validator() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Consume the request before responding. Writing immediately can
            // race the client's request body on Windows and surface a socket
            // reset instead of exercising the cover validator below.
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot-image"
            );
        });
        let url = Url::parse(&format!("http://{address}/cover.bin")).unwrap();
        let result = TheAudioDbClient::with_test_image(url.clone()).fetch_cover(&url);
        assert!(
            matches!(result, Err(OnlineMetadataError::MalformedResponse)),
            "unexpected malformed-cover result: {result:?}"
        );
    }
    #[test]
    fn chunked_cover_larger_than_cap_is_rejected_without_content_length() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            if stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
                )
                .is_err()
            {
                return;
            }
            let chunk = vec![b'x'; 8192];
            for _ in 0..=(crate::metadata::tags::MAX_COVER_BYTES / chunk.len()) {
                if write!(stream, "{:X}\r\n", chunk.len()).is_err()
                    || stream.write_all(&chunk).is_err()
                    || stream.write_all(b"\r\n").is_err()
                {
                    // The bounded client is expected to close early. A broken
                    // pipe is therefore a successful server-side test outcome.
                    break;
                }
            }
        });
        let url = Url::parse(&format!("http://{address}/cover.png")).unwrap();
        assert!(matches!(
            TheAudioDbClient::with_test_image(url.clone()).fetch_cover(&url),
            Err(OnlineMetadataError::MalformedResponse)
        ));
    }
    #[test]
    fn image_redirect_is_not_followed() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1/next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
        });
        let url = Url::parse(&format!("http://{address}/cover.png")).unwrap();
        assert!(matches!(
            TheAudioDbClient::with_test_image(url.clone()).fetch_cover(&url),
            Err(OnlineMetadataError::Unavailable)
        ));
    }
    #[test]
    fn production_host_spoofs_are_rejected_before_dispatch() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let client = TheAudioDbClient::production_client_with_api(
            Url::parse(&format!("http://{address}/not-the-audiodb")).unwrap(),
        );
        assert!(matches!(
            client.cover_url(None, "Artist", "Album"),
            Err(OnlineMetadataError::InvalidInput)
        ));
        assert!(matches!(
            TheAudioDbClient::new()
                .unwrap()
                .fetch_cover(&Url::parse(&format!("http://{address}/cover.png")).unwrap()),
            Err(OnlineMetadataError::InvalidInput)
        ));
        assert!(listener.accept().is_err());
    }

    #[test]
    fn production_timeouts_cover_observed_tls_setup() {
        assert_eq!(CONNECT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(20));
    }
}
