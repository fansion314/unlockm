use super::{
    FREE_ARTWORK_SOURCE, FREE_ATTRIBUTION, OnlineMetadataCandidate, OnlineMetadataProvider,
    OnlineMetadataProviderKind,
};
use reqwest::{StatusCode, blocking::Client, redirect::Policy};
use serde::Deserialize;
use std::{
    collections::HashSet,
    io::Read,
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
use url::{Host, Url};

pub const ROOT: &str = "https://musicbrainz.org/ws/2/";
pub const USER_AGENT: &str = concat!(
    "UnlockM/",
    env!("CARGO_PKG_VERSION"),
    " (https://git.fansionia.world/fansion/unlockm)"
);
pub const HOST: &str = "musicbrainz.org";
pub const MAX_QUERY_SCALARS: usize = 200;
pub const MAX_SEARCH_RESULTS: u8 = 10;
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_RESPONSE_BYTES: usize = 512 * 1024;

/// This seam keeps the mandatory global one-request-per-second policy
/// deterministic in tests, while the production client shares one gate.
pub trait RequestScheduler: Send + Sync {
    fn wait_turn(&self);
}
trait RateClock: Send + Sync {
    fn now(&self) -> Instant;
    fn sleep(&self, duration: Duration);
}
struct SystemRateClock;
impl RateClock for SystemRateClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }
}
pub struct OneRequestPerSecond {
    last: Mutex<Option<Instant>>,
    clock: Arc<dyn RateClock>,
}
impl OneRequestPerSecond {
    pub fn new() -> Self {
        Self {
            last: Mutex::new(None),
            clock: Arc::new(SystemRateClock),
        }
    }
    #[cfg(test)]
    fn with_clock(clock: Arc<dyn RateClock>) -> Self {
        Self {
            last: Mutex::new(None),
            clock,
        }
    }
}
impl Default for OneRequestPerSecond {
    fn default() -> Self {
        Self::new()
    }
}
impl RequestScheduler for OneRequestPerSecond {
    fn wait_turn(&self) {
        let mut last = self.last.lock().expect("MusicBrainz rate gate");
        if let Some(previous) = *last {
            let remaining = Duration::from_secs(1)
                .saturating_sub(self.clock.now().saturating_duration_since(previous));
            if !remaining.is_zero() {
                self.clock.sleep(remaining);
            }
        }
        *last = Some(self.clock.now());
    }
}
fn global_scheduler() -> Arc<dyn RequestScheduler> {
    static VALUE: OnceLock<Arc<OneRequestPerSecond>> = OnceLock::new();
    VALUE
        .get_or_init(|| Arc::new(OneRequestPerSecond::new()))
        .clone()
}
#[derive(Clone)]
pub struct MusicBrainzClient {
    client: Client,
    endpoint: Url,
    scheduler: Arc<dyn RequestScheduler>,
    #[cfg(test)]
    local_transport: bool,
}
impl MusicBrainzClient {
    pub fn new() -> Result<Self, super::OnlineMetadataError> {
        Ok(Self {
            client: Client::builder()
                .redirect(Policy::none())
                // This bound includes DNS, TCP, and TLS establishment. TLS to
                // the public service can exceed three seconds even when the
                // request succeeds, so keep it bounded independently without
                // cutting off the observed healthy path.
                .connect_timeout(CONNECT_TIMEOUT)
                // MusicBrainz search can take more than ten seconds during
                // normal public-service load. Give the complete request an
                // independent deadline after allowing connection setup.
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(request_error)?,
            endpoint: Url::parse(ROOT).expect("constant URL"),
            scheduler: global_scheduler(),
            #[cfg(test)]
            local_transport: false,
        })
    }
    #[cfg(test)]
    fn with_test_endpoint(endpoint: Url, scheduler: Arc<dyn RequestScheduler>) -> Self {
        Self {
            client: Client::builder()
                .redirect(Policy::none())
                .timeout(Duration::from_secs(1))
                .build()
                .unwrap(),
            endpoint,
            scheduler,
            local_transport: true,
        }
    }
    #[cfg(test)]
    fn production_client_with_endpoint(endpoint: Url) -> Self {
        Self {
            client: Client::builder()
                .redirect(Policy::none())
                .timeout(Duration::from_secs(1))
                .build()
                .unwrap(),
            endpoint,
            scheduler: Arc::new(OneRequestPerSecond::new()),
            local_transport: false,
        }
    }
    pub fn search_recordings(
        &self,
        query: &str,
        requested_limit: u8,
    ) -> Result<Vec<OnlineMetadataCandidate>, super::OnlineMetadataError> {
        #[cfg(test)]
        let mut url = if self.local_transport {
            self.endpoint.clone()
        } else {
            trusted_root(&self.endpoint)?
        };
        #[cfg(not(test))]
        let mut url = trusted_root(&self.endpoint)?;
        url.set_path("ws/2/recording/");
        url.query_pairs_mut()
            .append_pair("query", &recording_query(query)?)
            .append_pair("fmt", "json")
            .append_pair("dismax", "true")
            .append_pair(
                "limit",
                &requested_limit.clamp(1, MAX_SEARCH_RESULTS).to_string(),
            );
        self.scheduler.wait_turn();
        let response = self
            .client
            .get(url)
            .header("user-agent", USER_AGENT)
            .header("accept", "application/json")
            .send()
            .map_err(request_error)?;
        match response.status() {
            StatusCode::OK => serde_json::from_slice(&read_capped(response, MAX_RESPONSE_BYTES)?)
                .map(map_response)
                .map_err(|_| super::OnlineMetadataError::MalformedResponse),
            StatusCode::TOO_MANY_REQUESTS => Err(super::OnlineMetadataError::RateLimited {
                retry_after_secs: retry_after(&response),
            }),
            _ => Err(super::OnlineMetadataError::Unavailable),
        }
    }
}
impl OnlineMetadataProvider for MusicBrainzClient {
    fn kind(&self) -> OnlineMetadataProviderKind {
        OnlineMetadataProviderKind::MusicBrainz
    }
}
pub fn recording_query(query: &str) -> Result<String, super::OnlineMetadataError> {
    let value = query.trim();
    if value.is_empty() || value.chars().count() > MAX_QUERY_SCALARS {
        return Err(super::OnlineMetadataError::InvalidInput);
    }
    // `dismax=true` selects MusicBrainz plain indexed search, which escapes
    // special query syntax itself and searches the recording index's default
    // fields. The editor combines title, artist, and album hints, so wrapping
    // the whole value in `recording:` would incorrectly restrict every term
    // to the recording-title field.
    Ok(value.to_owned())
}
pub fn is_mbid(value: &str) -> bool {
    value.len() == 36
        && value.as_bytes().iter().enumerate().all(|(i, byte)| {
            if [8, 13, 18, 23].contains(&i) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}
pub fn open_recording_url(id: &str) -> Result<Url, super::OnlineMetadataError> {
    is_mbid(id)
        .then(|| Url::parse(&format!("https://{HOST}/recording/{id}")).expect("constant URL"))
        .ok_or(super::OnlineMetadataError::InvalidInput)
}
fn trusted_root(url: &Url) -> Result<Url, super::OnlineMetadataError> {
    (url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.host(), Some(Host::Domain(host)) if host.eq_ignore_ascii_case(HOST)))
    .then_some(url.clone())
    .ok_or(super::OnlineMetadataError::InvalidInput)
}
fn request_error(error: reqwest::Error) -> super::OnlineMetadataError {
    if error.is_timeout() {
        super::OnlineMetadataError::RequestTimeout
    } else {
        super::OnlineMetadataError::Unavailable
    }
}
fn retry_after(response: &reqwest::blocking::Response) -> u64 {
    response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .clamp(1, 3600)
}
pub(crate) fn read_capped(
    mut response: reqwest::blocking::Response,
    limit: usize,
) -> Result<Vec<u8>, super::OnlineMetadataError> {
    if response
        .content_length()
        .is_some_and(|n| n as usize > limit)
    {
        return Err(super::OnlineMetadataError::MalformedResponse);
    }
    let mut body = Vec::with_capacity(limit.min(16 * 1024));
    response
        .by_ref()
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut body)
        .map_err(|_| super::OnlineMetadataError::Unavailable)?;
    (body.len() <= limit)
        .then_some(body)
        .ok_or(super::OnlineMetadataError::MalformedResponse)
}

pub struct MusicBrainzProvider;
impl OnlineMetadataProvider for MusicBrainzProvider {
    fn kind(&self) -> OnlineMetadataProviderKind {
        OnlineMetadataProviderKind::MusicBrainz
    }
}

#[derive(Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    recordings: Vec<Recording>,
}
#[derive(Deserialize)]
struct Recording {
    id: Option<String>,
    title: Option<String>,
    length: Option<u32>,
    #[serde(rename = "artist-credit", default)]
    artists: Vec<Artist>,
    #[serde(default)]
    releases: Vec<Release>,
}
#[derive(Deserialize)]
struct Artist {
    name: Option<String>,
    #[serde(default)]
    joinphrase: String,
}
#[derive(Deserialize)]
struct Release {
    title: Option<String>,
    #[serde(rename = "release-group")]
    group: Option<Group>,
}
#[derive(Deserialize)]
struct Group {
    id: Option<String>,
    title: Option<String>,
}

pub fn map_response(response: SearchResponse) -> Vec<OnlineMetadataCandidate> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    for recording in response.recordings {
        if candidates.len() == 10 {
            break;
        }
        let (Some(id), Some(title)) = (recording.id, recording.title) else {
            continue;
        };
        let mut credited_artist = String::new();
        let artists = recording
            .artists
            .into_iter()
            .filter_map(|artist| {
                let name = artist.name?;
                credited_artist.push_str(&name);
                credited_artist.push_str(&artist.joinphrase);
                Some(name)
            })
            .collect::<Vec<_>>();
        if artists.is_empty() {
            continue;
        }
        let duration_ms = recording.length.unwrap_or_default();
        let mut added_release = false;

        for release in recording.releases {
            if candidates.len() == 10 {
                break;
            }
            let group_id = release
                .group
                .as_ref()
                .and_then(|group| group.id.as_deref())
                .unwrap_or_default()
                .to_owned();
            let album = release
                .group
                .as_ref()
                .and_then(|group| group.title.clone())
                .or(release.title)
                .unwrap_or_default();
            let dedupe_key = if group_id.is_empty() {
                format!("release:{album}")
            } else {
                group_id.clone()
            };
            if !seen.insert((id.clone(), dedupe_key)) {
                continue;
            }
            candidates.push(OnlineMetadataCandidate {
                provider: OnlineMetadataProviderKind::MusicBrainz,
                requires_authorization: false,
                artwork_source: Some(FREE_ARTWORK_SOURCE),
                attribution: FREE_ATTRIBUTION,
                item_id: id.clone(),
                title: title.clone(),
                artists: artists.clone(),
                album,
                duration_ms,
                cover_handle: None,
                release_group_id: (!group_id.is_empty()).then_some(group_id),
                credited_artist: (!credited_artist.is_empty()).then_some(credited_artist.clone()),
            });
            added_release = true;
        }

        if !added_release && seen.insert((id.clone(), String::new())) {
            candidates.push(OnlineMetadataCandidate {
                provider: OnlineMetadataProviderKind::MusicBrainz,
                requires_authorization: false,
                artwork_source: Some(FREE_ARTWORK_SOURCE),
                attribution: FREE_ATTRIBUTION,
                item_id: id,
                title,
                artists,
                album: String::new(),
                duration_ms,
                cover_handle: None,
                release_group_id: None,
                credited_artist: (!credited_artist.is_empty()).then_some(credited_artist),
            });
        }
    }

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        net::TcpListener,
        sync::atomic::{AtomicUsize, Ordering},
        thread,
    };
    struct CountingScheduler(AtomicUsize);
    impl RequestScheduler for CountingScheduler {
        fn wait_turn(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    struct LogicalScheduler(Mutex<(u64, Vec<u64>)>);
    impl RequestScheduler for LogicalScheduler {
        fn wait_turn(&self) {
            let mut state = self.0.lock().unwrap();
            let reserved = state.0;
            state.1.push(reserved);
            state.0 += 1000;
        }
    }
    struct SimulatedClock {
        now: Mutex<Instant>,
        sleeps: Mutex<Vec<Duration>>,
    }
    impl SimulatedClock {
        fn new() -> Self {
            Self {
                now: Mutex::new(Instant::now()),
                sleeps: Mutex::new(Vec::new()),
            }
        }
    }
    impl RateClock for SimulatedClock {
        fn now(&self) -> Instant {
            *self.now.lock().unwrap()
        }
        fn sleep(&self, duration: Duration) {
            self.sleeps.lock().unwrap().push(duration);
            *self.now.lock().unwrap() += duration;
        }
    }
    fn local_response(status: &str, body: &str) -> (Url, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sent, received) = std::sync::mpsc::channel();
        let status = status.to_owned();
        let body = body.to_owned();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 8192];
            let count = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..count]).into_owned();
            let _ = sent.send(request);
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        (Url::parse(&format!("http://{address}/")).unwrap(), received)
    }
    fn chunked_oversize_response() -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 8192];
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
            for _ in 0..=(MAX_RESPONSE_BYTES / chunk.len()) {
                if write!(stream, "{:X}\r\n", chunk.len()).is_err()
                    || stream.write_all(&chunk).is_err()
                    || stream.write_all(b"\r\n").is_err()
                {
                    break;
                }
            }
        });
        Url::parse(&format!("http://{address}/")).unwrap()
    }
    fn unused_loopback_endpoint() -> Url {
        // Ask the OS for an actually unused port instead of assuming that the
        // diagnostic discard port is closed. The resulting transport failure
        // may be an immediate refusal or a timeout depending on the OS network
        // stack, but it must never be mistaken for a successful response.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        Url::parse(&format!("http://{address}/")).unwrap()
    }
    #[test]
    fn builds_bounded_plain_dismax_query() {
        assert_eq!(recording_query("  a+b  ").unwrap(), "a+b");
        assert!(recording_query("   ").is_err());
        assert!(recording_query(&"x".repeat(MAX_QUERY_SCALARS + 1)).is_err());
        assert!(USER_AGENT.contains("UnlockM/"));
        assert_eq!(CONNECT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(20));
    }

    #[test]
    fn maps_joinphrases_dedupes_groups_and_keeps_missing_release_fallback() {
        let response: SearchResponse = serde_json::from_str(r#"{"recordings":[{"id":"r","title":"t","length":123,"artist-credit":[{"name":"A","joinphrase":" & "},{"name":"B"}],"releases":[{"title":"release","release-group":{"id":"g","title":"group"}},{"title":"duplicate","release-group":{"id":"g","title":"group"}}]},{"id":"none","title":"x","artist-credit":[{"name":"C"}]}]}"#).unwrap();
        let values = map_response(response);
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].artists, ["A", "B"]);
        assert_eq!(values[0].credited_artist.as_deref(), Some("A & B"));
        assert_eq!(values[0].album, "group");
        assert_eq!(values[1].album, "");
    }
    #[test]
    fn local_fixture_sends_meaningful_ua_bounded_query_and_uses_scheduler() {
        let body = r#"{"recordings":[]}"#;
        let (endpoint, received) = local_response("200 OK", body);
        let scheduler = Arc::new(CountingScheduler(AtomicUsize::new(0)));
        let client = MusicBrainzClient::with_test_endpoint(endpoint, scheduler.clone());
        assert!(client.search_recordings("a+b", 99).unwrap().is_empty());
        let request = received.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(request.contains(USER_AGENT));
        assert!(request.contains("limit=10"));
        assert!(request.contains("dismax=true"));
        assert!(request.contains("query=a%2Bb"));
        assert!(!request.contains("recording%3A"));
        assert_eq!(scheduler.0.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn local_503_is_unavailable_without_following_a_redirect() {
        let (endpoint, _) = local_response("503 Service Unavailable", "");
        let scheduler = Arc::new(CountingScheduler(AtomicUsize::new(0)));
        let client = MusicBrainzClient::with_test_endpoint(endpoint, scheduler);
        assert!(matches!(
            client.search_recordings("song", 1),
            Err(super::super::OnlineMetadataError::Unavailable)
        ));
    }
    #[test]
    fn local_redirect_and_offline_are_rejected_with_stable_network_errors() {
        let scheduler = Arc::new(CountingScheduler(AtomicUsize::new(0)));
        let (redirect, _) = local_response("302 Found\r\nLocation: http://127.0.0.1/next", "");
        let redirect_result = MusicBrainzClient::with_test_endpoint(redirect, scheduler.clone())
            .search_recordings("song", 1);
        assert!(
            matches!(
                redirect_result,
                Err(super::super::OnlineMetadataError::Unavailable)
            ),
            "unexpected redirect result: {redirect_result:?}"
        );
        let offline_result =
            MusicBrainzClient::with_test_endpoint(unused_loopback_endpoint(), scheduler)
                .search_recordings("song", 1);
        assert!(
            matches!(
                offline_result,
                Err(super::super::OnlineMetadataError::Unavailable)
                    | Err(super::super::OnlineMetadataError::RequestTimeout)
            ),
            "unexpected offline result: {offline_result:?}"
        );
    }
    #[test]
    fn chunked_response_larger_than_cap_is_rejected() {
        let scheduler = Arc::new(CountingScheduler(AtomicUsize::new(0)));
        let client = MusicBrainzClient::with_test_endpoint(chunked_oversize_response(), scheduler);
        assert!(matches!(
            client.search_recordings("song", 1),
            Err(super::super::OnlineMetadataError::MalformedResponse)
        ));
    }
    #[test]
    fn injected_scheduler_reserves_starts_at_least_one_second_apart_without_sleeping() {
        let scheduler = Arc::new(LogicalScheduler(Mutex::new((0, Vec::new()))));
        let (first, _) = local_response("200 OK", r#"{"recordings":[]}"#);
        let (second, _) = local_response("200 OK", r#"{"recordings":[]}"#);
        MusicBrainzClient::with_test_endpoint(first, scheduler.clone())
            .search_recordings("one", 1)
            .unwrap();
        MusicBrainzClient::with_test_endpoint(second, scheduler.clone())
            .search_recordings("two", 1)
            .unwrap();
        let starts = &scheduler.0.lock().unwrap().1;
        assert!(starts[1] >= starts[0] + 1000);
    }
    #[test]
    fn production_rate_gate_reserves_one_second_starts_with_an_injected_clock() {
        let clock = Arc::new(SimulatedClock::new());
        let scheduler = OneRequestPerSecond::with_clock(clock.clone());
        scheduler.wait_turn();
        scheduler.wait_turn();
        assert_eq!(*clock.sleeps.lock().unwrap(), vec![Duration::from_secs(1)]);
    }
    #[test]
    fn production_root_spoof_is_rejected_before_dispatch_and_slow_local_request_times_out() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        assert!(matches!(
            MusicBrainzClient::production_client_with_endpoint(
                Url::parse(&format!("http://{address}/not-musicbrainz")).unwrap()
            )
            .search_recordings("song", 1),
            Err(super::super::OnlineMetadataError::InvalidInput)
        ));
        assert!(listener.accept().is_err());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            thread::sleep(Duration::from_millis(1200));
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 17\r\n\r\n{\"recordings\":[]}");
        });
        assert!(matches!(
            MusicBrainzClient::with_test_endpoint(
                Url::parse(&format!("http://{address}/")).unwrap(),
                Arc::new(CountingScheduler(AtomicUsize::new(0))),
            )
            .search_recordings("song", 1),
            Err(super::super::OnlineMetadataError::RequestTimeout)
        ));
    }
}
