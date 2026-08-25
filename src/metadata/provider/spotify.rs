//! Narrow Spotify Web API adapter used by the desktop's explicit metadata
//! search flow.  It deliberately has no relationship with the automatic QQ
//! metadata merge pipeline: callers must present candidates to a user first.

use reqwest::{StatusCode, blocking::Client, redirect::Policy};
use serde::Deserialize;
use std::{io::Read, time::Duration};
use url::{Host, Url, form_urlencoded};

pub const SPOTIFY_ACCOUNTS_HOST: &str = "accounts.spotify.com";
pub const SPOTIFY_API_HOST: &str = "api.spotify.com";
pub const SPOTIFY_OPEN_HOST: &str = "open.spotify.com";
pub const SPOTIFY_TOKEN_ENDPOINT: &str = "https://accounts.spotify.com/api/token";
pub const SPOTIFY_SEARCH_ENDPOINT: &str = "https://api.spotify.com/v1/search";
pub const SPOTIFY_AUTHORIZE_ENDPOINT: &str = "https://accounts.spotify.com/authorize";
pub const MAX_QUERY_SCALARS: usize = 200;
pub const MAX_SEARCH_RESULTS: u8 = 10;
const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_COVER_BYTES: usize = 10 * 1024 * 1024;
pub const SPOTIFY_IMAGE_HOSTS: [&str; 2] = ["i.scdn.co", "image-cdn-ak.spotifycdn.com"];

use super::{
    OnlineMetadataCandidate, OnlineMetadataError, OnlineMetadataProvider,
    OnlineMetadataProviderKind,
};

#[derive(Debug, Clone)]
pub struct SpotifyConfig {
    pub client_id: String,
}

impl SpotifyConfig {
    pub fn from_client_id(client_id: impl Into<String>) -> Option<Self> {
        let client_id = client_id.into();
        (!client_id.trim().is_empty()).then_some(Self { client_id })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SpotifyToken {
    pub access_token: String,
    pub expires_in_secs: u64,
    pub refresh_token: Option<String>,
}

impl std::fmt::Debug for SpotifyToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SpotifyToken")
            .field("access_token", &"[REDACTED]")
            .field("expires_in_secs", &self.expires_in_secs)
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[derive(Clone)]
pub struct SpotifyClient {
    client: Client,
    accounts_endpoint: Url,
    search_endpoint: Url,
    #[cfg(test)]
    local_transport: bool,
}

impl SpotifyClient {
    pub fn new() -> Result<Self, OnlineMetadataError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(request_error)?;
        Ok(Self {
            client,
            accounts_endpoint: Url::parse(SPOTIFY_TOKEN_ENDPOINT).expect("constant URL"),
            search_endpoint: Url::parse(SPOTIFY_SEARCH_ENDPOINT).expect("constant URL"),
            #[cfg(test)]
            local_transport: false,
        })
    }

    #[cfg(test)]
    fn with_test_endpoints(accounts_endpoint: Url, search_endpoint: Url) -> Self {
        Self {
            client: Client::builder().redirect(Policy::none()).build().unwrap(),
            accounts_endpoint,
            search_endpoint,
            local_transport: true,
        }
    }

    pub fn authorization_url(
        config: &SpotifyConfig,
        redirect_uri: &str,
        state: &str,
        challenge: &str,
    ) -> Result<Url, OnlineMetadataError> {
        let redirect = validate_loopback_callback(redirect_uri)?;
        let mut url = Url::parse(SPOTIFY_AUTHORIZE_ENDPOINT)
            .map_err(|_| OnlineMetadataError::AuthorizationFailed)?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &config.client_id)
            .append_pair("redirect_uri", redirect.as_str())
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", challenge)
            .append_pair("state", state);
        Ok(url)
    }

    pub fn exchange_code(
        &self,
        config: &SpotifyConfig,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<SpotifyToken, OnlineMetadataError> {
        self.token(
            config,
            &[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("code_verifier", verifier),
            ],
        )
    }

    pub fn refresh(
        &self,
        config: &SpotifyConfig,
        refresh_token: &str,
    ) -> Result<SpotifyToken, OnlineMetadataError> {
        self.token(
            config,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ],
        )
    }

    fn token(
        &self,
        config: &SpotifyConfig,
        form: &[(&str, &str)],
    ) -> Result<SpotifyToken, OnlineMetadataError> {
        let url = self.endpoint(&self.accounts_endpoint, SPOTIFY_ACCOUNTS_HOST)?;
        let mut encoded = form_urlencoded::Serializer::new(String::new());
        encoded.extend_pairs(form.iter().copied());
        encoded.append_pair("client_id", &config.client_id);
        let response = self
            .client
            .post(url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(encoded.finish())
            .send()
            .map_err(request_error)?;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(OnlineMetadataError::RateLimited {
                retry_after_secs: retry_after(&response),
            });
        }
        if response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::UNAUTHORIZED
        {
            return Err(OnlineMetadataError::ReauthorizationRequired);
        }
        if !response.status().is_success() {
            return Err(OnlineMetadataError::Unavailable);
        }
        parse_token(read_bounded(response)?)
    }

    pub fn search_tracks(
        &self,
        access_token: &str,
        query: &str,
        requested_limit: u8,
    ) -> Result<Vec<SpotifySearchResult>, OnlineMetadataError> {
        let query = bounded_query(query)?;
        let limit = requested_limit.clamp(1, MAX_SEARCH_RESULTS);
        let mut url = self.endpoint(&self.search_endpoint, SPOTIFY_API_HOST)?;
        url.query_pairs_mut()
            .append_pair("q", &query)
            .append_pair("type", "track")
            .append_pair("limit", &limit.to_string());
        let response = self
            .client
            .get(url)
            .bearer_auth(access_token)
            .send()
            .map_err(request_error)?;
        match response.status() {
            StatusCode::OK => parse_search(read_bounded(response)?),
            StatusCode::UNAUTHORIZED => Err(OnlineMetadataError::ReauthorizationRequired),
            StatusCode::TOO_MANY_REQUESTS => Err(OnlineMetadataError::RateLimited {
                retry_after_secs: retry_after(&response),
            }),
            _ => Err(OnlineMetadataError::Unavailable),
        }
    }

    /// Fetches only an image URL that was parsed from the immediately-current
    /// Spotify search response. It is revalidated before dispatch, never
    /// redirected, bounded, and decoded by the existing cover validator.
    pub fn fetch_album_artwork(&self, url: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
        #[cfg(test)]
        let url = if self.local_transport {
            url.clone()
        } else {
            trusted_spotify_image_url(url.as_str())?
        };
        #[cfg(not(test))]
        let url = trusted_spotify_image_url(url.as_str())?;
        let response = self.client.get(url).send().map_err(request_error)?;
        if !response.status().is_success() {
            return Err(OnlineMetadataError::Unavailable);
        }
        if response
            .content_length()
            .is_some_and(|size| size as usize > MAX_COVER_BYTES)
        {
            return Err(OnlineMetadataError::MalformedResponse);
        }
        let body = read_capped(response, MAX_COVER_BYTES)?;
        if crate::metadata::tags::validate_cover(&body).is_err() {
            return Err(OnlineMetadataError::MalformedResponse);
        }
        Ok(body.to_vec())
    }

    fn endpoint(&self, endpoint: &Url, expected_host: &str) -> Result<Url, OnlineMetadataError> {
        #[cfg(test)]
        if self.local_transport {
            return Ok(endpoint.clone());
        }
        trusted_https_url(endpoint.as_str(), expected_host)
    }
}

impl OnlineMetadataProvider for SpotifyClient {
    fn kind(&self) -> OnlineMetadataProviderKind {
        OnlineMetadataProviderKind::Spotify
    }
}

pub fn bounded_query(query: &str) -> Result<String, OnlineMetadataError> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > MAX_QUERY_SCALARS {
        return Err(OnlineMetadataError::InvalidInput);
    }
    Ok(query.to_owned())
}

pub fn open_track_url(item_id: &str) -> Result<Url, OnlineMetadataError> {
    if !is_track_id(item_id) {
        return Err(OnlineMetadataError::InvalidInput);
    }
    trusted_https_url(
        &format!("https://open.spotify.com/track/{item_id}"),
        SPOTIFY_OPEN_HOST,
    )
}

pub fn is_track_id(value: &str) -> bool {
    (20..=30).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn validate_loopback_callback(raw: &str) -> Result<Url, OnlineMetadataError> {
    let url = Url::parse(raw).map_err(|_| OnlineMetadataError::InvalidInput)?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.path() != "/callback"
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(OnlineMetadataError::InvalidInput);
    }
    Ok(url)
}

fn trusted_https_url(raw: &str, expected_host: &str) -> Result<Url, OnlineMetadataError> {
    let url = Url::parse(raw).map_err(|_| OnlineMetadataError::InvalidInput)?;
    let allowed = url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.host(), Some(Host::Domain(host)) if host.eq_ignore_ascii_case(expected_host));
    allowed
        .then_some(url)
        .ok_or(OnlineMetadataError::InvalidInput)
}

fn trusted_spotify_image_url(raw: &str) -> Result<Url, OnlineMetadataError> {
    let url = Url::parse(raw).map_err(|_| OnlineMetadataError::InvalidInput)?;
    let allowed = url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.host(), Some(Host::Domain(host)) if SPOTIFY_IMAGE_HOSTS.iter().any(|allowed| host.eq_ignore_ascii_case(allowed)));
    allowed
        .then_some(url)
        .ok_or(OnlineMetadataError::InvalidInput)
}

fn read_bounded(response: reqwest::blocking::Response) -> Result<Vec<u8>, OnlineMetadataError> {
    read_capped(response, MAX_RESPONSE_BYTES)
}

fn read_capped(
    mut response: reqwest::blocking::Response,
    limit: usize,
) -> Result<Vec<u8>, OnlineMetadataError> {
    if response
        .content_length()
        .is_some_and(|size| size as usize > limit)
    {
        return Err(OnlineMetadataError::MalformedResponse);
    }
    let mut body = Vec::with_capacity(limit.min(16 * 1024));
    response
        .by_ref()
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut body)
        .map_err(|_| OnlineMetadataError::Unavailable)?;
    (body.len() <= limit)
        .then_some(body)
        .ok_or(OnlineMetadataError::MalformedResponse)
}

fn retry_after(response: &reqwest::blocking::Response) -> u64 {
    response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1)
        .clamp(1, 3600)
}

fn request_error(error: reqwest::Error) -> OnlineMetadataError {
    if error.is_timeout() {
        OnlineMetadataError::RequestTimeout
    } else {
        OnlineMetadataError::Unavailable
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: u64,
    #[serde(default)]
    refresh_token: Option<String>,
}

fn parse_token(body: Vec<u8>) -> Result<SpotifyToken, OnlineMetadataError> {
    let token: TokenResponse =
        serde_json::from_slice(&body).map_err(|_| OnlineMetadataError::MalformedResponse)?;
    if token.access_token.is_empty() || token.expires_in == 0 {
        return Err(OnlineMetadataError::MalformedResponse);
    }
    Ok(SpotifyToken {
        access_token: token.access_token,
        expires_in_secs: token.expires_in,
        refresh_token: token.refresh_token.filter(|token| !token.is_empty()),
    })
}

#[derive(Deserialize)]
struct SearchResponse {
    tracks: Option<Tracks>,
}
#[derive(Deserialize)]
struct Tracks {
    #[serde(default)]
    items: Vec<Track>,
}
#[derive(Deserialize)]
struct Track {
    id: Option<String>,
    name: Option<String>,
    #[serde(default)]
    artists: Vec<Artist>,
    album: Option<Album>,
    duration_ms: Option<u32>,
}
#[derive(Deserialize)]
struct Artist {
    name: Option<String>,
}
#[derive(Deserialize)]
struct Album {
    name: Option<String>,
    #[serde(default)]
    images: Vec<AlbumImage>,
}
#[derive(Deserialize)]
struct AlbumImage {
    url: Option<String>,
}

/// Rust-only search value. It is intentionally not serializable and its
/// artwork URL is only available to the desktop state for immediate binding to
/// a random opaque handle.
pub struct SpotifySearchResult {
    pub candidate: OnlineMetadataCandidate,
    cover_image_url: Option<Url>,
}
impl SpotifySearchResult {
    pub fn cover_image_url(&self) -> Option<&Url> {
        self.cover_image_url.as_ref()
    }
    pub fn from_validated_cover_url(
        candidate: OnlineMetadataCandidate,
        cover_image_url: Option<Url>,
    ) -> Self {
        Self {
            candidate,
            cover_image_url,
        }
    }
}

fn parse_search(body: Vec<u8>) -> Result<Vec<SpotifySearchResult>, OnlineMetadataError> {
    let response: SearchResponse =
        serde_json::from_slice(&body).map_err(|_| OnlineMetadataError::MalformedResponse)?;
    let candidates = response
        .tracks
        .map(|tracks| tracks.items)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|track| {
            let id = track.id.filter(|id| is_track_id(id))?;
            let title = track.name.filter(|name| !name.trim().is_empty())?;
            let artists = track
                .artists
                .into_iter()
                .filter_map(|artist| artist.name.filter(|name| !name.trim().is_empty()))
                .collect::<Vec<_>>();
            if artists.is_empty() {
                return None;
            }
            let (album, cover_image_url) = track
                .album
                .map(|album| {
                    let cover = album
                        .images
                        .into_iter()
                        .filter_map(|image| image.url)
                        .find_map(|url| trusted_spotify_image_url(&url).ok());
                    (album.name.unwrap_or_default(), cover)
                })
                .unwrap_or_default();
            Some(SpotifySearchResult {
                candidate: OnlineMetadataCandidate {
                    provider: OnlineMetadataProviderKind::Spotify,
                    requires_authorization: true,
                    artwork_source: Some("Spotify"),
                    attribution: "Spotify",
                    item_id: id,
                    title,
                    artists,
                    album,
                    duration_ms: track.duration_ms.unwrap_or_default(),
                    cover_handle: None,
                    release_group_id: None,
                    credited_artist: None,
                },
                cover_image_url,
            })
        })
        .take(MAX_SEARCH_RESULTS as usize)
        .collect();
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    fn local_server(response: String) -> (Url, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let read = stream.read(&mut request).unwrap();
            let _ = sender.send(String::from_utf8_lossy(&request[..read]).into_owned());
            stream.write_all(response.as_bytes()).unwrap();
        });
        (
            Url::parse(&format!("http://{address}/endpoint")).unwrap(),
            receiver,
        )
    }

    #[test]
    fn search_mapping_tolerates_removed_fields_and_skips_incomplete_items() {
        let result = parse_search(br#"{"tracks":{"items":[{"id":"0123456789ABCDEFGHIJKL","name":"Song","artists":[{"name":"A"}],"album":{"name":"Album","images":[{"url":"https://i.scdn.co/image/cover"}]},"duration_ms":1234,"popularity":99},{"id":"bad","name":"skip","artists":[]}]}}"#.to_vec()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].candidate.title, "Song");
        assert_eq!(result[0].candidate.album, "Album");
        assert_eq!(
            result[0].cover_image_url().unwrap().host_str(),
            Some("i.scdn.co")
        );
    }

    #[test]
    fn image_url_policy_is_exact_and_never_enters_candidate() {
        assert!(trusted_spotify_image_url("https://i.scdn.co/image/x").is_ok());
        for image in [
            "http://i.scdn.co/x",
            "https://i.scdn.co.evil/x",
            "https://127.0.0.1/x",
            "https://user@i.scdn.co/x",
        ] {
            assert!(trusted_spotify_image_url(image).is_err(), "{image}");
        }
    }

    #[test]
    fn test_transport_exercises_pkce_form_without_a_client_secret() {
        let body = "{\"access_token\":\"access\",\"expires_in\":3600}";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let (token, request) = local_server(response);
        let client = SpotifyClient::with_test_endpoints(token.clone(), token);
        let config = SpotifyConfig {
            client_id: "public-client".into(),
        };
        let value = client
            .exchange_code(&config, "code", "http://127.0.0.1:1/callback", "verifier")
            .unwrap();
        assert_eq!(value.expires_in_secs, 3600);
        let request = request.recv().unwrap();
        assert!(request.contains("code_verifier=verifier"));
        assert!(request.contains("client_id=public-client"));
        assert!(!request.contains("client_secret"));
    }

    #[test]
    fn chunked_oversize_response_is_rejected_without_content_length() {
        let body = "x".repeat(MAX_RESPONSE_BYTES + 1);
        let response = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:X}\r\n{}\r\n0\r\n\r\n",
            body.len(),
            body
        );
        let (search, _) = local_server(response);
        let client = SpotifyClient::with_test_endpoints(search.clone(), search);
        assert!(matches!(
            client.search_tracks("token", "song", 1),
            Err(OnlineMetadataError::MalformedResponse)
        ));
    }

    #[test]
    fn local_cover_fixture_rejects_redirect_malformed_and_oversize_bodies() {
        for response in [
            "HTTP/1.1 302 Found\r\nLocation: https://evil.example/x\r\nContent-Length: 0\r\n\r\n"
                .to_owned(),
            "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nbad".to_owned(),
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                MAX_COVER_BYTES + 1
            ),
        ] {
            let (cover, _) = local_server(response);
            let client = SpotifyClient::with_test_endpoints(cover.clone(), cover.clone());
            assert!(client.fetch_album_artwork(&cover).is_err());
        }
    }

    #[test]
    fn local_token_and_search_statuses_map_to_stable_errors() {
        let (token, _) =
            local_server("HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n".into());
        let client = SpotifyClient::with_test_endpoints(token.clone(), token);
        assert!(matches!(
            client.refresh(
                &SpotifyConfig {
                    client_id: "id".into()
                },
                "refresh"
            ),
            Err(OnlineMetadataError::ReauthorizationRequired)
        ));

        let (search, _) = local_server(
            "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 99999\r\nContent-Length: 0\r\n\r\n"
                .into(),
        );
        let client = SpotifyClient::with_test_endpoints(search.clone(), search);
        assert!(matches!(
            client.search_tracks("token", "song", 1),
            Err(OnlineMetadataError::RateLimited {
                retry_after_secs: 3600
            })
        ));

        let (search, _) =
            local_server("HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n".into());
        let client = SpotifyClient::with_test_endpoints(search.clone(), search);
        assert!(matches!(
            client.search_tracks("token", "song", 1),
            Err(OnlineMetadataError::Unavailable)
        ));

        let unused = TcpListener::bind("127.0.0.1:0").unwrap();
        let offline =
            Url::parse(&format!("http://{}/search", unused.local_addr().unwrap())).unwrap();
        drop(unused);
        let client = SpotifyClient::with_test_endpoints(offline.clone(), offline);
        assert!(matches!(
            client.search_tracks("token", "song", 1),
            Err(OnlineMetadataError::Unavailable)
        ));
    }

    #[test]
    fn production_host_validation_rejects_spoof_before_dispatch() {
        let (spoof, receiver) = local_server("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".into());
        let mut client = SpotifyClient::new().unwrap();
        client.accounts_endpoint = spoof;
        assert!(matches!(
            client.refresh(
                &SpotifyConfig {
                    client_id: "id".into()
                },
                "refresh"
            ),
            Err(OnlineMetadataError::InvalidInput)
        ));
        assert!(receiver.recv_timeout(Duration::from_millis(30)).is_err());

        let (spoof, receiver) = local_server("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".into());
        let mut client = SpotifyClient::new().unwrap();
        client.search_endpoint = spoof;
        assert!(matches!(
            client.search_tracks("token", "song", 1),
            Err(OnlineMetadataError::InvalidInput)
        ));
        assert!(receiver.recv_timeout(Duration::from_millis(30)).is_err());
    }

    #[test]
    fn bounds_and_open_url_are_fixed() {
        assert!(bounded_query("").is_err());
        assert!(bounded_query(&"a".repeat(MAX_QUERY_SCALARS + 1)).is_err());
        assert!(
            open_track_url("0123456789ABCDEFGHIJKL")
                .unwrap()
                .as_str()
                .starts_with("https://open.spotify.com/track/")
        );
        assert!(open_track_url("https://evil.example").is_err());
        assert!(
            SpotifyClient::authorization_url(
                &SpotifyConfig {
                    client_id: "public".into()
                },
                "http://localhost/callback",
                "state",
                "challenge"
            )
            .is_err()
        );
        assert!(
            !format!(
                "{:?}",
                SpotifyToken {
                    access_token: "access-secret".into(),
                    expires_in_secs: 1,
                    refresh_token: Some("refresh-secret".into())
                }
            )
            .contains("secret")
        );
    }
}
