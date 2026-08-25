//! Cover Art Archive artwork is resolved only for an explicit user request.
use super::{OnlineMetadataError, musicbrainz::read_capped};
use reqwest::{StatusCode, blocking::Client, header::LOCATION, redirect::Policy};
use std::time::Duration;
use url::{Host, Url};

pub const API_HOST: &str = "coverartarchive.org";
pub const ARCHIVE_HOST: &str = "archive.org";
pub const ARCHIVE_SUBDOMAIN_SUFFIX: &str = ".archive.org";
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
pub const MAX_REDIRECTS: usize = 3;

#[derive(Clone)]
pub struct CoverArtArchiveClient {
    lookup_client: Client,
    fetch_client: Client,
    root: Url,
    #[cfg(test)]
    local_transport: bool,
}

impl CoverArtArchiveClient {
    pub fn new() -> Result<Self, OnlineMetadataError> {
        Ok(Self {
            lookup_client: lookup_client(REQUEST_TIMEOUT)?,
            fetch_client: fetch_client(REQUEST_TIMEOUT)?,
            root: Url::parse(&format!("https://{API_HOST}/")).expect("constant URL"),
            #[cfg(test)]
            local_transport: false,
        })
    }

    #[cfg(test)]
    pub(super) fn with_test_root(root: Url) -> Self {
        Self {
            lookup_client: Client::builder()
                .redirect(Policy::none())
                .timeout(Duration::from_secs(1))
                .build()
                .unwrap(),
            fetch_client: Client::builder()
                .redirect(Policy::limited(MAX_REDIRECTS))
                .timeout(Duration::from_secs(1))
                .build()
                .unwrap(),
            root,
            local_transport: true,
        }
    }

    #[cfg(test)]
    fn production_client_with_root(root: Url) -> Self {
        Self {
            lookup_client: lookup_client(Duration::from_secs(1)).unwrap(),
            fetch_client: fetch_client(Duration::from_secs(1)).unwrap(),
            root,
            local_transport: false,
        }
    }

    /// Resolves a release-group front image without downloading its bytes.
    /// The CAA endpoint responds with a storage redirect; only that validated
    /// location is retained by the caller's opaque handle.
    pub fn cover_url(
        &self,
        release_group_id: Option<&str>,
    ) -> Result<Option<Url>, OnlineMetadataError> {
        let Some(group) = release_group_id.filter(|value| super::musicbrainz::is_mbid(value))
        else {
            return Ok(None);
        };
        #[cfg(test)]
        let mut url = if self.local_transport {
            self.root.clone()
        } else {
            trusted_api_url(&self.root)?
        };
        #[cfg(not(test))]
        let mut url = trusted_api_url(&self.root)?;
        url.set_path(&format!("/release-group/{group}/front-500"));
        url.set_query(None);
        let response = self
            .lookup_client
            .head(url.clone())
            .send()
            .map_err(request_error)?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| response.url().join(value).ok())
                .ok_or(OnlineMetadataError::MalformedResponse)?;
            #[cfg(test)]
            if self.local_transport {
                return Ok(Some(location));
            }
            return trusted_archive_url(&location).map(Some);
        }
        response
            .status()
            .is_success()
            .then_some(Some(url))
            .ok_or(OnlineMetadataError::Unavailable)
    }

    pub fn supports(&self, url: &Url) -> bool {
        #[cfg(test)]
        if self.local_transport {
            return true;
        }
        trusted_cover_url(url).is_ok()
    }

    pub fn fetch_cover(&self, url: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
        #[cfg(test)]
        let url = if self.local_transport {
            url.clone()
        } else {
            trusted_cover_url(url)?
        };
        #[cfg(not(test))]
        let url = trusted_cover_url(url)?;
        let response = self.fetch_client.get(url).send().map_err(request_error)?;
        if !response.status().is_success() {
            return Err(OnlineMetadataError::Unavailable);
        }
        let bytes = read_capped(response, crate::metadata::tags::MAX_COVER_BYTES)?;
        crate::metadata::tags::validate_cover(&bytes)
            .map_err(|_| OnlineMetadataError::MalformedResponse)?;
        Ok(bytes)
    }
}

fn lookup_client(timeout: Duration) -> Result<Client, OnlineMetadataError> {
    Client::builder()
        .redirect(Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(timeout)
        .build()
        .map_err(request_error)
}

fn fetch_client(timeout: Duration) -> Result<Client, OnlineMetadataError> {
    Client::builder()
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                return attempt.error("too many Cover Art Archive redirects");
            }
            if trusted_archive_url(attempt.url()).is_ok() {
                attempt.follow()
            } else {
                attempt.error("untrusted Cover Art Archive redirect")
            }
        }))
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(timeout)
        .build()
        .map_err(request_error)
}

fn trusted_api_url(url: &Url) -> Result<Url, OnlineMetadataError> {
    trusted_exact_host(url, API_HOST)
}

fn trusted_archive_url(url: &Url) -> Result<Url, OnlineMetadataError> {
    let trusted = url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.host(), Some(Host::Domain(host)) if {
            let host = host.to_ascii_lowercase();
            host == ARCHIVE_HOST || host.ends_with(ARCHIVE_SUBDOMAIN_SUFFIX)
        });
    trusted
        .then_some(url.clone())
        .ok_or(OnlineMetadataError::InvalidInput)
}

fn trusted_cover_url(url: &Url) -> Result<Url, OnlineMetadataError> {
    trusted_api_url(url).or_else(|_| trusted_archive_url(url))
}

fn trusted_exact_host(url: &Url, expected: &str) -> Result<Url, OnlineMetadataError> {
    (url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.host(), Some(Host::Domain(host)) if host.eq_ignore_ascii_case(expected)))
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

    fn response(status: &'static str, headers: String, body: Vec<u8>) -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request);
            let content_length = if headers.to_ascii_lowercase().contains("content-length:") {
                String::new()
            } else {
                format!("Content-Length: {}\r\n", body.len())
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\n{headers}{content_length}Connection: close\r\n\r\n"
            );
            let _ = stream.write_all(&body);
        });
        Url::parse(&format!("http://{address}/")).unwrap()
    }

    #[test]
    fn lookup_builds_release_group_front_500_and_accepts_one_location() {
        let (sent, received) = std::sync::mpsc::channel();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let count = stream.read(&mut request).unwrap();
            sent.send(String::from_utf8_lossy(&request[..count]).into_owned())
                .unwrap();
            let _ = write!(
                stream,
                "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:9/cover.jpg\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
        });
        let client = CoverArtArchiveClient::with_test_root(
            Url::parse(&format!("http://{address}/")).unwrap(),
        );
        assert_eq!(
            client
                .cover_url(Some("11111111-1111-1111-1111-111111111111"))
                .unwrap()
                .unwrap()
                .as_str(),
            "http://127.0.0.1:9/cover.jpg"
        );
        let request = received.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(
            request
                .starts_with("HEAD /release-group/11111111-1111-1111-1111-111111111111/front-500")
        );
    }

    #[test]
    fn missing_release_group_cover_is_none() {
        let client = CoverArtArchiveClient::with_test_root(response(
            "404 Not Found",
            String::new(),
            Vec::new(),
        ));
        assert!(
            client
                .cover_url(Some("11111111-1111-1111-1111-111111111111"))
                .unwrap()
                .is_none()
        );
        assert!(client.cover_url(None).unwrap().is_none());
    }

    #[test]
    fn local_redirected_image_is_followed_and_validated() {
        let png = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
            .unwrap();
        let image = response("200 OK", "Content-Type: image/png\r\n".into(), png.clone());
        let redirect = response(
            "307 Temporary Redirect",
            format!("Location: {image}\r\n"),
            Vec::new(),
        );
        assert_eq!(
            CoverArtArchiveClient::with_test_root(redirect.clone())
                .fetch_cover(&redirect)
                .unwrap(),
            png
        );
    }

    #[test]
    fn malformed_and_declared_oversize_images_are_rejected() {
        let malformed = response("200 OK", String::new(), b"not-an-image".to_vec());
        assert!(matches!(
            CoverArtArchiveClient::with_test_root(malformed.clone()).fetch_cover(&malformed),
            Err(OnlineMetadataError::MalformedResponse)
        ));

        let oversize = response(
            "200 OK",
            format!(
                "Content-Length: {}\r\n",
                crate::metadata::tags::MAX_COVER_BYTES + 1
            ),
            Vec::new(),
        );
        assert!(matches!(
            CoverArtArchiveClient::with_test_root(oversize.clone()).fetch_cover(&oversize),
            Err(OnlineMetadataError::MalformedResponse)
        ));
    }

    #[test]
    fn production_hosts_redirects_and_timeouts_are_strict() {
        for value in [
            "https://archive.org/download/a/cover.jpg",
            "https://dn710200.ca.archive.org/0/items/a/cover.jpg",
        ] {
            assert!(trusted_archive_url(&Url::parse(value).unwrap()).is_ok());
        }
        for value in [
            "http://archive.org/a.jpg",
            "https://archive.org.evil.example/a.jpg",
            "https://127.0.0.1/a.jpg",
            "https://user@archive.org/a.jpg",
            "https://archive.org:444/a.jpg",
        ] {
            assert!(trusted_archive_url(&Url::parse(value).unwrap()).is_err());
        }
        assert_eq!(CONNECT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(20));
        assert_eq!(MAX_REDIRECTS, 3);
    }

    #[test]
    fn production_root_spoof_is_rejected_before_dispatch() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let client = CoverArtArchiveClient::production_client_with_root(
            Url::parse(&format!("http://{address}/")).unwrap(),
        );
        assert!(matches!(
            client.cover_url(Some("11111111-1111-1111-1111-111111111111")),
            Err(OnlineMetadataError::InvalidInput)
        ));
        assert!(listener.accept().is_err());
    }
}
