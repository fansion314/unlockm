use super::{
    auth::AuthorizationFlow,
    credential_store::{CredentialStore, StoredCredential, system_store},
};
use getrandom::fill;
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use unlockm::metadata::provider::spotify::{
    SpotifyClient, SpotifyConfig, SpotifySearchResult, SpotifyToken,
};
#[cfg(test)]
use unlockm::metadata::provider::{FREE_ARTWORK_SOURCE, FREE_ATTRIBUTION};
use unlockm::metadata::provider::{
    OnlineMetadataCandidate, OnlineMetadataError, OnlineMetadataProviderKind,
};
use unlockm::metadata::provider::{
    free_artwork::FreeArtworkClient, musicbrainz::MusicBrainzClient,
};
use url::Url;

const ACCESS_EARLY_EXPIRY: Duration = Duration::from_secs(60);
const REAUTH_AGE: Duration = Duration::from_secs(6 * 30 * 24 * 60 * 60);
const REAUTH_WARNING: Duration = Duration::from_secs(14 * 24 * 60 * 60);
const COVER_HANDLE_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_COVER_HANDLES: usize = 10;
const MAX_PREVIEW_FETCHES: usize = 2;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OnlineMetadataStatus {
    pub provider: &'static str,
    pub state: OnlineMetadataConnection,
    pub retry_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OnlineMetadataConnection {
    NotConfigured,
    Disconnected,
    Authorizing,
    Connected,
    ReauthorizationRequired,
    CredentialUnavailable,
}

struct AccessToken {
    value: String,
    expires_at_ms: u64,
}
struct Inner {
    access: Option<AccessToken>,
    authorization_active: bool,
    authorization_cancel: Option<Arc<AtomicBool>>,
    spotify_retry_at_ms: Option<u64>,
    musicbrainz_retry_at_ms: Option<u64>,
    reauthorization_required: bool,
    authorization_generation: u64,
}
#[derive(Clone)]
struct CoverBinding {
    provider: OnlineMetadataProviderKind,
    source: CoverSource,
    expires_at_ms: u64,
    generation: u64,
}
#[derive(Clone)]
enum CoverSource {
    Spotify(Url),
    MusicBrainz {
        release_group_id: Option<String>,
        artist: String,
        album: String,
        resolved_url: Option<Url>,
    },
}

trait SpotifyService: Send + Sync {
    fn exchange(
        &self,
        config: &SpotifyConfig,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<SpotifyToken, OnlineMetadataError>;
    fn refresh(
        &self,
        config: &SpotifyConfig,
        refresh: &str,
    ) -> Result<SpotifyToken, OnlineMetadataError>;
    fn search(
        &self,
        access: &str,
        query: &str,
    ) -> Result<Vec<SpotifySearchResult>, OnlineMetadataError>;
    fn cover(&self, url: &Url) -> Result<Vec<u8>, OnlineMetadataError>;
}
struct NetworkSpotifyService;
impl SpotifyService for NetworkSpotifyService {
    fn exchange(
        &self,
        config: &SpotifyConfig,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<SpotifyToken, OnlineMetadataError> {
        SpotifyClient::new()?.exchange_code(config, code, redirect_uri, verifier)
    }
    fn refresh(
        &self,
        config: &SpotifyConfig,
        refresh: &str,
    ) -> Result<SpotifyToken, OnlineMetadataError> {
        SpotifyClient::new()?.refresh(config, refresh)
    }
    fn search(
        &self,
        access: &str,
        query: &str,
    ) -> Result<Vec<SpotifySearchResult>, OnlineMetadataError> {
        SpotifyClient::new()?.search_tracks(access, query, 10)
    }
    fn cover(&self, url: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
        SpotifyClient::new()?.fetch_album_artwork(url)
    }
}

/// Keeps the no-login providers independently injectable. In particular, the
/// state layer can prove that a MusicBrainz text search does not accidentally
/// trigger Cover Art Archive, TheAudioDB, or image traffic.
trait FreeMetadataService: Send + Sync {
    fn search_musicbrainz(
        &self,
        query: &str,
    ) -> Result<Vec<OnlineMetadataCandidate>, OnlineMetadataError>;
    fn lookup_musicbrainz_cover(
        &self,
        release_group_id: Option<&str>,
        artist: &str,
        album: &str,
    ) -> Result<Option<Url>, OnlineMetadataError>;
    fn fetch_musicbrainz_cover(&self, url: &Url) -> Result<Vec<u8>, OnlineMetadataError>;
}

struct NetworkFreeMetadataService;
impl FreeMetadataService for NetworkFreeMetadataService {
    fn search_musicbrainz(
        &self,
        query: &str,
    ) -> Result<Vec<OnlineMetadataCandidate>, OnlineMetadataError> {
        MusicBrainzClient::new()?.search_recordings(query, 10)
    }
    fn lookup_musicbrainz_cover(
        &self,
        release_group_id: Option<&str>,
        artist: &str,
        album: &str,
    ) -> Result<Option<Url>, OnlineMetadataError> {
        FreeArtworkClient::new()?.cover_url(release_group_id, artist, album)
    }
    fn fetch_musicbrainz_cover(&self, url: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
        FreeArtworkClient::new()?.fetch_cover(url)
    }
}

#[derive(Clone)]
pub struct OnlineMetadataState {
    config: Option<SpotifyConfig>,
    credentials: Arc<dyn CredentialStore>,
    inner: Arc<Mutex<Inner>>,
    /// Serializes token refresh and search requests. This bounded, single
    /// service gate also makes refresh rotation atomic with secure storage.
    request_gate: Arc<Mutex<()>>,
    clock: Arc<dyn Fn() -> u64 + Send + Sync>,
    cover_bindings: Arc<Mutex<HashMap<String, CoverBinding>>>,
    service: Arc<dyn SpotifyService>,
    free_service: Arc<dyn FreeMetadataService>,
    generation: Arc<AtomicU64>,
    preview_permits: Arc<(Mutex<usize>, Condvar)>,
    /// Free artwork lookup is explicitly on-demand and single-flight.
    cover_lookup_gate: Arc<Mutex<()>>,
}

impl OnlineMetadataState {
    pub fn from_build_config() -> Self {
        Self::new(
            SpotifyConfig::from_client_id(
                option_env!("UNLOCKM_SPOTIFY_CLIENT_ID").unwrap_or_default(),
            ),
            system_store(),
        )
    }

    pub fn new(config: Option<SpotifyConfig>, credentials: Arc<dyn CredentialStore>) -> Self {
        Self::with_service(
            config,
            credentials,
            Arc::new(now_ms),
            Arc::new(NetworkSpotifyService),
        )
    }

    #[cfg(test)]
    fn with_clock(
        config: Option<SpotifyConfig>,
        credentials: Arc<dyn CredentialStore>,
        clock: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        Self::with_service(config, credentials, clock, Arc::new(NetworkSpotifyService))
    }

    fn with_service(
        config: Option<SpotifyConfig>,
        credentials: Arc<dyn CredentialStore>,
        clock: Arc<dyn Fn() -> u64 + Send + Sync>,
        service: Arc<dyn SpotifyService>,
    ) -> Self {
        Self::with_services(
            config,
            credentials,
            clock,
            service,
            Arc::new(NetworkFreeMetadataService),
        )
    }

    fn with_services(
        config: Option<SpotifyConfig>,
        credentials: Arc<dyn CredentialStore>,
        clock: Arc<dyn Fn() -> u64 + Send + Sync>,
        service: Arc<dyn SpotifyService>,
        free_service: Arc<dyn FreeMetadataService>,
    ) -> Self {
        Self {
            config,
            credentials,
            inner: Arc::new(Mutex::new(Inner {
                access: None,
                authorization_active: false,
                authorization_cancel: None,
                spotify_retry_at_ms: None,
                musicbrainz_retry_at_ms: None,
                reauthorization_required: false,
                authorization_generation: 0,
            })),
            request_gate: Arc::new(Mutex::new(())),
            clock,
            cover_bindings: Arc::new(Mutex::new(HashMap::new())),
            service,
            free_service,
            generation: Arc::new(AtomicU64::new(0)),
            preview_permits: Arc::new((Mutex::new(MAX_PREVIEW_FETCHES), Condvar::new())),
            cover_lookup_gate: Arc::new(Mutex::new(())),
        }
    }

    pub fn config(&self) -> Option<SpotifyConfig> {
        self.config.clone()
    }

    pub fn status(&self) -> OnlineMetadataStatus {
        self.status_for(OnlineMetadataProviderKind::Spotify)
    }

    pub fn status_for(&self, provider: OnlineMetadataProviderKind) -> OnlineMetadataStatus {
        if provider == OnlineMetadataProviderKind::MusicBrainz {
            return self.status_with(
                provider,
                OnlineMetadataConnection::Connected,
                self.retry_at_for(provider),
            );
        }
        let Some(config) = self.config.as_ref() else {
            return self.status_with(provider, OnlineMetadataConnection::NotConfigured, None);
        };
        let inner = self.inner.lock().expect("online metadata lock");
        if inner.authorization_active {
            return self.status_with(
                provider,
                OnlineMetadataConnection::Authorizing,
                inner.spotify_retry_at_ms,
            );
        }
        let retry_at_ms = inner.spotify_retry_at_ms;
        let reauthorization_required = inner.reauthorization_required;
        drop(inner);
        match self.credentials.load() {
            Err(_) => self.status_with(
                provider,
                OnlineMetadataConnection::CredentialUnavailable,
                retry_at_ms,
            ),
            Ok(None) if reauthorization_required => self.status_with(
                provider,
                OnlineMetadataConnection::ReauthorizationRequired,
                retry_at_ms,
            ),
            Ok(None) => self.status_with(
                provider,
                OnlineMetadataConnection::Disconnected,
                retry_at_ms,
            ),
            Ok(Some(credential))
                if !credential.matches_client_id(&config.client_id)
                    || credential_needs_reauth(&credential, self.now()) =>
            {
                self.status_with(
                    provider,
                    OnlineMetadataConnection::ReauthorizationRequired,
                    retry_at_ms,
                )
            }
            Ok(Some(_)) if reauthorization_required => self.status_with(
                provider,
                OnlineMetadataConnection::ReauthorizationRequired,
                retry_at_ms,
            ),
            Ok(Some(_)) => {
                self.status_with(provider, OnlineMetadataConnection::Connected, retry_at_ms)
            }
        }
    }

    fn status_with(
        &self,
        provider: OnlineMetadataProviderKind,
        state: OnlineMetadataConnection,
        retry_at_ms: Option<u64>,
    ) -> OnlineMetadataStatus {
        OnlineMetadataStatus {
            provider: provider.as_str(),
            state,
            retry_at_ms,
        }
    }

    pub fn begin_login(&self) -> Result<AuthorizationFlow, OnlineMetadataError> {
        if self.config.is_none() {
            return Err(OnlineMetadataError::NotConfigured);
        }
        let mut flow = AuthorizationFlow::begin()?;
        let mut inner = self.inner.lock().expect("online metadata lock");
        if inner.authorization_active {
            return Err(OnlineMetadataError::AuthorizationBusy);
        }
        inner.authorization_active = true;
        inner.authorization_generation = inner.authorization_generation.saturating_add(1);
        flow.generation = inner.authorization_generation;
        inner.authorization_cancel = Some(flow.cancellation_handle());
        Ok(flow)
    }

    pub fn complete_login(
        &self,
        flow: &AuthorizationFlow,
        code: &str,
    ) -> Result<(), OnlineMetadataError> {
        let _requests = self
            .request_gate
            .lock()
            .expect("online metadata request gate");
        let result = (|| {
            if !self.auth_flow_is_current(flow.generation) {
                return Err(OnlineMetadataError::AuthorizationCancelled);
            }
            let config = self
                .config
                .as_ref()
                .ok_or(OnlineMetadataError::NotConfigured)?;
            let token = self
                .service
                .exchange(config, code, &flow.redirect_uri, &flow.verifier)?;
            if !self.auth_flow_is_current(flow.generation) {
                return Err(OnlineMetadataError::AuthorizationCancelled);
            }
            self.save_token(config, token)
        })();
        let mut inner = self.inner.lock().expect("online metadata lock");
        inner.authorization_active = false;
        inner.authorization_cancel = None;
        inner.reauthorization_required = matches!(
            result,
            Err(OnlineMetadataError::ReauthorizationRequired)
                | Err(OnlineMetadataError::AuthorizationFailed)
        );
        result
    }

    pub fn cancel_login(&self) -> bool {
        let inner = self.inner.lock().expect("online metadata lock");
        let Some(cancel) = inner.authorization_cancel.as_ref() else {
            return false;
        };
        cancel.store(true, Ordering::Release);
        true
    }

    pub fn abort_login(&self) {
        let mut inner = self.inner.lock().expect("online metadata lock");
        inner.authorization_active = false;
        inner.authorization_cancel = None;
        inner.authorization_generation = inner.authorization_generation.saturating_add(1);
    }

    pub fn disconnect(&self) -> Result<(), OnlineMetadataError> {
        // Serialize deletion against token refresh/search publication. Once a
        // successful disconnect returns, no earlier request can save or cache
        // a token afterwards.
        let _requests = self
            .request_gate
            .lock()
            .expect("online metadata request gate");
        self.cancel_login();
        self.credentials
            .delete()
            .map_err(|_| OnlineMetadataError::CredentialUnavailable)?;
        let mut inner = self.inner.lock().expect("online metadata lock");
        inner.access = None;
        inner.authorization_active = false;
        inner.authorization_cancel = None;
        inner.authorization_generation = inner.authorization_generation.saturating_add(1);
        inner.reauthorization_required = false;
        inner.spotify_retry_at_ms = None;
        inner.musicbrainz_retry_at_ms = None;
        self.invalidate_cover_bindings();
        Ok(())
    }

    pub fn search(&self, query: &str) -> Result<Vec<OnlineMetadataCandidate>, OnlineMetadataError> {
        self.search_for(OnlineMetadataProviderKind::Spotify, query)
    }

    pub fn search_for(
        &self,
        provider: OnlineMetadataProviderKind,
        query: &str,
    ) -> Result<Vec<OnlineMetadataCandidate>, OnlineMetadataError> {
        let _gate = self
            .request_gate
            .lock()
            .expect("online metadata request gate");
        let generation = self
            .generation
            .fetch_add(1, Ordering::AcqRel)
            .saturating_add(1);
        self.clear_cover_bindings();
        if provider == OnlineMetadataProviderKind::MusicBrainz {
            let now = self.now();
            if let Some(retry_at) = self
                .retry_at_for(provider)
                .filter(|retry_at| *retry_at > now)
            {
                return Err(OnlineMetadataError::RateLimited {
                    retry_after_secs: (retry_at - now).div_ceil(1000),
                });
            }
            return match self.free_service.search_musicbrainz(query) {
                Ok(result) => Ok(self.bind_musicbrainz_cover_handles(result, generation)),
                Err(error @ OnlineMetadataError::RateLimited { retry_after_secs }) => {
                    self.record_retry_after(provider, retry_after_secs);
                    Err(error)
                }
                Err(error) => Err(error),
            };
        }
        let now = self.now();
        let retry_at = self.retry_at_for(provider);
        if let Some(retry_at) = retry_at.filter(|retry_at| *retry_at > now) {
            return Err(OnlineMetadataError::RateLimited {
                retry_after_secs: (retry_at - now).div_ceil(1000),
            });
        }
        let token = self.access_token()?;
        match self.service.search(&token, query) {
            Ok(results) => Ok(self.bind_cover_handles(results, generation)),
            Err(OnlineMetadataError::ReauthorizationRequired) => {
                // One request is replayed after a refresh; a second 401 enters
                // a stable reauthorization state.
                self.clear_access();
                let token = self.access_token()?;
                let result = self
                    .service
                    .search(&token, query)
                    .map(|results| self.bind_cover_handles(results, generation));
                if matches!(result, Err(OnlineMetadataError::ReauthorizationRequired)) {
                    self.require_reauthorization();
                }
                if let Err(OnlineMetadataError::RateLimited { retry_after_secs }) = &result {
                    self.record_retry_after(provider, *retry_after_secs);
                }
                result
            }
            Err(error @ OnlineMetadataError::RateLimited { retry_after_secs }) => {
                self.record_retry_after(provider, retry_after_secs);
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    pub fn preview_cover(
        &self,
        provider: OnlineMetadataProviderKind,
        handle: &str,
    ) -> Result<Vec<u8>, OnlineMetadataError> {
        let _permit = self.acquire_preview_permit();
        let now = self.now();
        let generation = self.generation.load(Ordering::Acquire);
        let source = {
            let mut bindings = self.cover_bindings.lock().expect("cover binding lock");
            bindings.retain(|_, binding| binding.expires_at_ms > now);
            bindings
                .get(handle)
                .filter(|binding| binding.generation == generation && binding.provider == provider)
                .map(|binding| binding.source.clone())
                .ok_or(OnlineMetadataError::InvalidInput)?
        };
        let result = match source.clone() {
            CoverSource::Spotify(url) => self.service.cover(&url)?,
            CoverSource::MusicBrainz { .. } => self.musicbrainz_cover(handle, generation)?,
        };
        let still_current = self
            .cover_bindings
            .lock()
            .expect("cover binding lock")
            .get(handle)
            .is_some_and(|binding| {
                binding.generation == generation
                    && binding.provider == provider
                    && binding.expires_at_ms > self.now()
                    && same_cover_source(&binding.source, &source)
            });
        still_current
            .then_some(result)
            .ok_or(OnlineMetadataError::InvalidInput)
    }

    fn bind_cover_handles(
        &self,
        results: Vec<SpotifySearchResult>,
        generation: u64,
    ) -> Vec<OnlineMetadataCandidate> {
        let expires_at_ms = self
            .now()
            .saturating_add(COVER_HANDLE_TTL.as_millis() as u64);
        let mut bindings = self.cover_bindings.lock().expect("cover binding lock");
        if self.generation.load(Ordering::Acquire) != generation {
            return Vec::new();
        }
        bindings.clear();
        results
            .into_iter()
            .take(MAX_COVER_HANDLES)
            .map(|result| {
                let cover_url = result.cover_image_url().cloned();
                let mut candidate = result.candidate;
                if let Some(url) = cover_url
                    && let Ok(handle) = random_handle()
                {
                    bindings.insert(
                        handle.clone(),
                        CoverBinding {
                            provider: OnlineMetadataProviderKind::Spotify,
                            source: CoverSource::Spotify(url),
                            expires_at_ms,
                            generation,
                        },
                    );
                    candidate.cover_handle = Some(handle);
                }
                candidate
            })
            .collect()
    }

    fn bind_musicbrainz_cover_handles(
        &self,
        results: Vec<OnlineMetadataCandidate>,
        generation: u64,
    ) -> Vec<OnlineMetadataCandidate> {
        let expires_at_ms = self
            .now()
            .saturating_add(COVER_HANDLE_TTL.as_millis() as u64);
        let mut bindings = self.cover_bindings.lock().expect("cover binding lock");
        if self.generation.load(Ordering::Acquire) != generation {
            return Vec::new();
        }
        bindings.clear();
        results
            .into_iter()
            .take(MAX_COVER_HANDLES)
            .map(|mut candidate| {
                let can_bind = (!candidate.album.trim().is_empty()
                    && !candidate.artists.is_empty())
                    || candidate.release_group_id.is_some();
                if can_bind && let Ok(handle) = random_handle() {
                    let artist = candidate
                        .credited_artist
                        .clone()
                        .unwrap_or_else(|| candidate.artists.join(", "));
                    bindings.insert(
                        handle.clone(),
                        CoverBinding {
                            provider: OnlineMetadataProviderKind::MusicBrainz,
                            source: CoverSource::MusicBrainz {
                                release_group_id: candidate.release_group_id.clone(),
                                artist,
                                album: candidate.album.clone(),
                                resolved_url: None,
                            },
                            expires_at_ms,
                            generation,
                        },
                    );
                    candidate.cover_handle = Some(handle);
                }
                candidate
            })
            .collect()
    }

    fn musicbrainz_cover(
        &self,
        handle: &str,
        generation: u64,
    ) -> Result<Vec<u8>, OnlineMetadataError> {
        let url = {
            // Only the CAA/TheAudioDB lookup is single-flight. Once its
            // fixed-host URL is bound to this current handle, image downloads
            // may use the shared two-permit preview budget rather than
            // serializing behind a slow lookup.
            let _lookup = self.cover_lookup_gate.lock().expect("cover lookup gate");
            let source = self
                .cover_bindings
                .lock()
                .expect("cover binding lock")
                .get(handle)
                .and_then(|binding| match &binding.source {
                    CoverSource::MusicBrainz {
                        release_group_id,
                        artist,
                        album,
                        resolved_url,
                    } => Some((
                        release_group_id.clone(),
                        artist.clone(),
                        album.clone(),
                        resolved_url.clone(),
                    )),
                    CoverSource::Spotify(_) => None,
                })
                .ok_or(OnlineMetadataError::InvalidInput)?;
            let (url, remember) = match source.3 {
                Some(url) => (url, false),
                None => (
                    self.free_service
                        .lookup_musicbrainz_cover(source.0.as_deref(), &source.1, &source.2)?
                        .ok_or(OnlineMetadataError::ArtworkNotFound)?,
                    true,
                ),
            };
            if remember {
                let mut bindings = self.cover_bindings.lock().expect("cover binding lock");
                let binding = bindings
                    .get_mut(handle)
                    .ok_or(OnlineMetadataError::InvalidInput)?;
                if binding.generation != generation || binding.expires_at_ms <= self.now() {
                    return Err(OnlineMetadataError::InvalidInput);
                }
                if let CoverSource::MusicBrainz { resolved_url, .. } = &mut binding.source {
                    *resolved_url = Some(url.clone());
                }
            }
            url
        };
        self.free_service.fetch_musicbrainz_cover(&url)
    }

    fn clear_cover_bindings(&self) {
        self.cover_bindings
            .lock()
            .expect("cover binding lock")
            .clear();
    }
    fn invalidate_cover_bindings(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.clear_cover_bindings();
    }
    fn acquire_preview_permit(&self) -> PreviewPermit<'_> {
        let (lock, wake) = &*self.preview_permits;
        let mut permits = lock.lock().expect("preview permit lock");
        while *permits == 0 {
            permits = wake.wait(permits).expect("preview permit wait");
        }
        *permits -= 1;
        PreviewPermit(&self.preview_permits)
    }

    fn access_token(&self) -> Result<String, OnlineMetadataError> {
        let now = self.now();
        if let Some(access) = self
            .inner
            .lock()
            .expect("online metadata lock")
            .access
            .as_ref()
            && access
                .expires_at_ms
                .saturating_sub(ACCESS_EARLY_EXPIRY.as_millis() as u64)
                > now
        {
            return Ok(access.value.clone());
        }
        let config = self
            .config
            .as_ref()
            .ok_or(OnlineMetadataError::NotConfigured)?;
        let credential = self
            .credentials
            .load()
            .map_err(|_| OnlineMetadataError::CredentialUnavailable)?
            .filter(|credential| {
                credential.matches_client_id(&config.client_id)
                    && !credential_needs_reauth(credential, now)
            })
            .ok_or(OnlineMetadataError::ReauthorizationRequired)?;
        let token = match self.service.refresh(config, credential.refresh_token()) {
            Ok(token) => token,
            Err(error @ OnlineMetadataError::ReauthorizationRequired) => {
                self.require_reauthorization();
                return Err(error);
            }
            Err(error @ OnlineMetadataError::RateLimited { retry_after_secs }) => {
                self.record_retry_after(OnlineMetadataProviderKind::Spotify, retry_after_secs);
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        self.save_token(config, token)?;
        self.inner
            .lock()
            .expect("online metadata lock")
            .access
            .as_ref()
            .map(|access| access.value.clone())
            .ok_or(OnlineMetadataError::Unavailable)
    }

    fn save_token(
        &self,
        config: &SpotifyConfig,
        token: SpotifyToken,
    ) -> Result<(), OnlineMetadataError> {
        let existing = self
            .credentials
            .load()
            .map_err(|_| OnlineMetadataError::CredentialUnavailable)?;
        let (refresh_token, issued_at_ms) = match token.refresh_token {
            Some(rotated) => (rotated, self.now()),
            None => existing
                .map(|credential| {
                    (
                        credential.refresh_token().to_owned(),
                        credential.issued_at_ms(),
                    )
                })
                .ok_or(OnlineMetadataError::ReauthorizationRequired)?,
        };
        // Persist rotation before exposing the associated access token.
        self.credentials
            .save(&StoredCredential::new(
                refresh_token,
                issued_at_ms,
                &config.client_id,
            ))
            .map_err(|_| OnlineMetadataError::CredentialUnavailable)?;
        let mut inner = self.inner.lock().expect("online metadata lock");
        inner.access = Some(AccessToken {
            value: token.access_token,
            expires_at_ms: self
                .now()
                .saturating_add(token.expires_in_secs.saturating_mul(1000)),
        });
        inner.reauthorization_required = false;
        Ok(())
    }

    fn clear_access(&self) {
        self.inner.lock().expect("online metadata lock").access = None;
    }
    fn require_reauthorization(&self) {
        self.inner
            .lock()
            .expect("online metadata lock")
            .reauthorization_required = true;
    }
    fn auth_flow_is_current(&self, generation: u64) -> bool {
        let inner = self.inner.lock().expect("online metadata lock");
        inner.authorization_active && inner.authorization_generation == generation
    }
    fn retry_at_for(&self, provider: OnlineMetadataProviderKind) -> Option<u64> {
        let inner = self.inner.lock().expect("online metadata lock");
        match provider {
            OnlineMetadataProviderKind::Spotify => inner.spotify_retry_at_ms,
            OnlineMetadataProviderKind::MusicBrainz => inner.musicbrainz_retry_at_ms,
        }
    }
    fn record_retry_after(&self, provider: OnlineMetadataProviderKind, retry_after_secs: u64) {
        let retry_at = Some(
            self.now()
                .saturating_add(retry_after_secs.saturating_mul(1000)),
        );
        let mut inner = self.inner.lock().expect("online metadata lock");
        match provider {
            OnlineMetadataProviderKind::Spotify => inner.spotify_retry_at_ms = retry_at,
            OnlineMetadataProviderKind::MusicBrainz => inner.musicbrainz_retry_at_ms = retry_at,
        }
    }
    fn now(&self) -> u64 {
        (self.clock)()
    }
}

fn same_cover_source(left: &CoverSource, right: &CoverSource) -> bool {
    match (left, right) {
        (CoverSource::Spotify(left), CoverSource::Spotify(right)) => left == right,
        (
            CoverSource::MusicBrainz {
                release_group_id: left_group,
                artist: left_artist,
                album: left_album,
                ..
            },
            CoverSource::MusicBrainz {
                release_group_id: right_group,
                artist: right_artist,
                album: right_album,
                ..
            },
        ) => left_group == right_group && left_artist == right_artist && left_album == right_album,
        _ => false,
    }
}

struct PreviewPermit<'a>(&'a Arc<(Mutex<usize>, Condvar)>);
impl Drop for PreviewPermit<'_> {
    fn drop(&mut self) {
        let (lock, wake) = &**self.0;
        *lock.lock().expect("preview permit lock") += 1;
        wake.notify_one();
    }
}

fn random_handle() -> Result<String, OnlineMetadataError> {
    let mut bytes = [0_u8; 16];
    fill(&mut bytes).map_err(|_| OnlineMetadataError::Unavailable)?;
    Ok(hex::encode(bytes))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn credential_needs_reauth(value: &StoredCredential, now: u64) -> bool {
    let age = now.saturating_sub(value.issued_at_ms());
    age >= (REAUTH_AGE - REAUTH_WARNING).as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::online_metadata::credential_store::{FailingCredentialStore, MemoryCredentialStore};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    struct MockService {
        refreshes: AtomicUsize,
        refresh_results: Mutex<Vec<Result<SpotifyToken, OnlineMetadataError>>>,
        searches: Mutex<Vec<Result<Vec<SpotifySearchResult>, OnlineMetadataError>>>,
    }
    impl MockService {
        fn new(searches: Vec<Result<Vec<SpotifySearchResult>, OnlineMetadataError>>) -> Self {
            Self {
                refreshes: AtomicUsize::new(0),
                refresh_results: Mutex::new(vec![
                    Ok(SpotifyToken {
                        access_token: "access".into(),
                        expires_in_secs: 3600,
                        refresh_token: None
                    });
                    4
                ]),
                searches: Mutex::new(searches),
            }
        }
        fn with_refresh_results(
            refresh_results: Vec<Result<SpotifyToken, OnlineMetadataError>>,
            searches: Vec<Result<Vec<SpotifySearchResult>, OnlineMetadataError>>,
        ) -> Self {
            Self {
                refreshes: AtomicUsize::new(0),
                refresh_results: Mutex::new(refresh_results),
                searches: Mutex::new(searches),
            }
        }
    }
    impl SpotifyService for MockService {
        fn exchange(
            &self,
            _: &SpotifyConfig,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<SpotifyToken, OnlineMetadataError> {
            Err(OnlineMetadataError::Unavailable)
        }
        fn refresh(&self, _: &SpotifyConfig, _: &str) -> Result<SpotifyToken, OnlineMetadataError> {
            self.refreshes.fetch_add(1, AtomicOrdering::SeqCst);
            self.refresh_results.lock().unwrap().remove(0)
        }
        fn search(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Vec<SpotifySearchResult>, OnlineMetadataError> {
            self.searches.lock().unwrap().remove(0)
        }
        fn cover(&self, _: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
            Err(OnlineMetadataError::Unavailable)
        }
    }
    struct SuccessfulExchangeService;
    impl SpotifyService for SuccessfulExchangeService {
        fn exchange(
            &self,
            _: &SpotifyConfig,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<SpotifyToken, OnlineMetadataError> {
            Ok(SpotifyToken {
                access_token: "access".into(),
                expires_in_secs: 3600,
                refresh_token: Some("refresh".into()),
            })
        }
        fn refresh(&self, _: &SpotifyConfig, _: &str) -> Result<SpotifyToken, OnlineMetadataError> {
            Err(OnlineMetadataError::Unavailable)
        }
        fn search(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Vec<SpotifySearchResult>, OnlineMetadataError> {
            Ok(vec![])
        }
        fn cover(&self, _: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
            Err(OnlineMetadataError::Unavailable)
        }
    }
    struct MockFreeService {
        searches: Mutex<Vec<Result<Vec<OnlineMetadataCandidate>, OnlineMetadataError>>>,
        lookups: AtomicUsize,
        lookup_artists: Mutex<Vec<String>>,
        fetches: AtomicUsize,
        block_lookup: bool,
        block_fetch: bool,
        lookup_started: AtomicUsize,
        fetch_started: AtomicUsize,
        released: Arc<(Mutex<bool>, Condvar)>,
    }
    impl MockFreeService {
        fn new(searches: Vec<Result<Vec<OnlineMetadataCandidate>, OnlineMetadataError>>) -> Self {
            Self {
                searches: Mutex::new(searches),
                lookups: AtomicUsize::new(0),
                lookup_artists: Mutex::new(Vec::new()),
                fetches: AtomicUsize::new(0),
                block_lookup: false,
                block_fetch: false,
                lookup_started: AtomicUsize::new(0),
                fetch_started: AtomicUsize::new(0),
                released: Arc::new((Mutex::new(true), Condvar::new())),
            }
        }
        fn blocking(
            searches: Vec<Result<Vec<OnlineMetadataCandidate>, OnlineMetadataError>>,
            block_lookup: bool,
            block_fetch: bool,
        ) -> Self {
            Self {
                released: Arc::new((Mutex::new(false), Condvar::new())),
                block_lookup,
                block_fetch,
                ..Self::new(searches)
            }
        }
        fn release(&self) {
            let (lock, wake) = &*self.released;
            *lock.lock().unwrap() = true;
            wake.notify_all();
        }
        fn wait_if_blocked(&self, block: bool, started: &AtomicUsize) {
            started.fetch_add(1, AtomicOrdering::SeqCst);
            if block {
                let (lock, wake) = &*self.released;
                let mut released = lock.lock().unwrap();
                while !*released {
                    released = wake.wait(released).unwrap();
                }
            }
        }
    }
    impl FreeMetadataService for MockFreeService {
        fn search_musicbrainz(
            &self,
            _: &str,
        ) -> Result<Vec<OnlineMetadataCandidate>, OnlineMetadataError> {
            self.searches.lock().unwrap().remove(0)
        }
        fn lookup_musicbrainz_cover(
            &self,
            _: Option<&str>,
            artist: &str,
            _: &str,
        ) -> Result<Option<Url>, OnlineMetadataError> {
            self.lookups.fetch_add(1, AtomicOrdering::SeqCst);
            self.lookup_artists.lock().unwrap().push(artist.into());
            self.wait_if_blocked(self.block_lookup, &self.lookup_started);
            Ok(Some(
                Url::parse("https://r2.theaudiodb.com/cover.png").unwrap(),
            ))
        }
        fn fetch_musicbrainz_cover(&self, _: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
            self.fetches.fetch_add(1, AtomicOrdering::SeqCst);
            self.wait_if_blocked(self.block_fetch, &self.fetch_started);
            Ok(vec![1])
        }
    }
    struct MissingCoverFreeService;
    impl FreeMetadataService for MissingCoverFreeService {
        fn search_musicbrainz(
            &self,
            _: &str,
        ) -> Result<Vec<OnlineMetadataCandidate>, OnlineMetadataError> {
            Ok(vec![mb_candidate()])
        }
        fn lookup_musicbrainz_cover(
            &self,
            _: Option<&str>,
            _: &str,
            _: &str,
        ) -> Result<Option<Url>, OnlineMetadataError> {
            Ok(None)
        }
        fn fetch_musicbrainz_cover(&self, _: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
            panic!("missing artwork must not dispatch an image request")
        }
    }
    fn mb_candidate() -> OnlineMetadataCandidate {
        OnlineMetadataCandidate {
            provider: OnlineMetadataProviderKind::MusicBrainz,
            requires_authorization: false,
            artwork_source: Some(FREE_ARTWORK_SOURCE),
            attribution: FREE_ATTRIBUTION,
            item_id: "11111111-1111-1111-1111-111111111111".into(),
            title: "song".into(),
            artists: vec!["artist".into()],
            album: "album".into(),
            duration_ms: 1,
            cover_handle: None,
            release_group_id: Some("22222222-2222-2222-2222-222222222222".into()),
            credited_artist: Some("artist".into()),
        }
    }
    struct BlockingRefreshService {
        started: AtomicUsize,
        released: Arc<(Mutex<bool>, Condvar)>,
    }
    impl SpotifyService for BlockingRefreshService {
        fn exchange(
            &self,
            _: &SpotifyConfig,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<SpotifyToken, OnlineMetadataError> {
            Err(OnlineMetadataError::Unavailable)
        }
        fn refresh(&self, _: &SpotifyConfig, _: &str) -> Result<SpotifyToken, OnlineMetadataError> {
            self.started.fetch_add(1, AtomicOrdering::SeqCst);
            let (lock, wake) = &*self.released;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = wake.wait(released).unwrap();
            }
            Ok(SpotifyToken {
                access_token: "late-access".into(),
                expires_in_secs: 3600,
                refresh_token: Some("late-rotation".into()),
            })
        }
        fn search(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Vec<SpotifySearchResult>, OnlineMetadataError> {
            Ok(vec![])
        }
        fn cover(&self, _: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
            Err(OnlineMetadataError::Unavailable)
        }
    }
    struct BlockingCoverService {
        started: AtomicUsize,
        released: Arc<(Mutex<bool>, Condvar)>,
    }
    struct BlockingExchangeService {
        started: AtomicUsize,
        released: Arc<(Mutex<bool>, Condvar)>,
    }
    impl SpotifyService for BlockingExchangeService {
        fn exchange(
            &self,
            _: &SpotifyConfig,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<SpotifyToken, OnlineMetadataError> {
            self.started.fetch_add(1, AtomicOrdering::SeqCst);
            let (lock, wake) = &*self.released;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = wake.wait(released).unwrap();
            }
            Ok(SpotifyToken {
                access_token: "late-access".into(),
                expires_in_secs: 3600,
                refresh_token: Some("late-rotation".into()),
            })
        }
        fn refresh(&self, _: &SpotifyConfig, _: &str) -> Result<SpotifyToken, OnlineMetadataError> {
            Err(OnlineMetadataError::Unavailable)
        }
        fn search(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Vec<SpotifySearchResult>, OnlineMetadataError> {
            Ok(vec![])
        }
        fn cover(&self, _: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
            Err(OnlineMetadataError::Unavailable)
        }
    }
    impl SpotifyService for BlockingCoverService {
        fn exchange(
            &self,
            _: &SpotifyConfig,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<SpotifyToken, OnlineMetadataError> {
            Err(OnlineMetadataError::Unavailable)
        }
        fn refresh(&self, _: &SpotifyConfig, _: &str) -> Result<SpotifyToken, OnlineMetadataError> {
            Err(OnlineMetadataError::Unavailable)
        }
        fn search(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Vec<SpotifySearchResult>, OnlineMetadataError> {
            Ok(vec![])
        }
        fn cover(&self, _: &Url) -> Result<Vec<u8>, OnlineMetadataError> {
            self.started.fetch_add(1, AtomicOrdering::SeqCst);
            let (lock, wake) = &*self.released;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = wake.wait(released).unwrap();
            }
            Ok(vec![1])
        }
    }
    #[test]
    fn missing_configuration_fails_closed_without_touching_credentials() {
        let state =
            OnlineMetadataState::new(None, Arc::new(MemoryCredentialStore(Mutex::new(None))));
        assert_eq!(
            state.status().state,
            OnlineMetadataConnection::NotConfigured
        );
        assert!(matches!(
            state.begin_login(),
            Err(OnlineMetadataError::NotConfigured)
        ));
    }
    #[test]
    fn musicbrainz_search_is_no_login_and_never_fetches_artwork_automatically() {
        let free = Arc::new(MockFreeService::new(vec![Ok(vec![mb_candidate()])]));
        let state = OnlineMetadataState::with_services(
            None,
            Arc::new(MemoryCredentialStore(Mutex::new(None))),
            Arc::new(|| 100),
            Arc::new(MockService::new(vec![])),
            free.clone(),
        );
        assert_eq!(
            state.status().state,
            OnlineMetadataConnection::NotConfigured
        );
        let candidates = state
            .search_for(OnlineMetadataProviderKind::MusicBrainz, "song")
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].cover_handle.is_some());
        assert_eq!(free.lookups.load(AtomicOrdering::SeqCst), 0);
        assert_eq!(free.fetches.load(AtomicOrdering::SeqCst), 0);
    }
    #[test]
    fn missing_free_artwork_has_a_distinct_error_code() {
        let state = OnlineMetadataState::with_services(
            None,
            Arc::new(MemoryCredentialStore(Mutex::new(None))),
            Arc::new(|| 100),
            Arc::new(MockService::new(vec![])),
            Arc::new(MissingCoverFreeService),
        );
        let handle = state
            .search_for(OnlineMetadataProviderKind::MusicBrainz, "song")
            .unwrap()
            .remove(0)
            .cover_handle
            .unwrap();
        let error = state
            .preview_cover(OnlineMetadataProviderKind::MusicBrainz, &handle)
            .unwrap_err();
        assert!(matches!(error, OnlineMetadataError::ArtworkNotFound));
        assert_eq!(error.code(), "online_artwork_not_found");
    }
    #[test]
    fn same_musicbrainz_handle_coalesces_lookup_and_respects_preview_bound() {
        let free = Arc::new(MockFreeService::blocking(
            vec![Ok(vec![mb_candidate()])],
            true,
            false,
        ));
        let state = OnlineMetadataState::with_services(
            None,
            Arc::new(MemoryCredentialStore(Mutex::new(None))),
            Arc::new(|| 100),
            Arc::new(MockService::new(vec![])),
            free.clone(),
        );
        let handle = state
            .search_for(OnlineMetadataProviderKind::MusicBrainz, "song")
            .unwrap()[0]
            .cover_handle
            .clone()
            .unwrap();
        std::thread::scope(|scope| {
            let first_state = state.clone();
            let first_handle = handle.clone();
            let first = scope.spawn(move || {
                first_state.preview_cover(OnlineMetadataProviderKind::MusicBrainz, &first_handle)
            });
            while free.lookup_started.load(AtomicOrdering::SeqCst) == 0 {
                std::thread::yield_now();
            }
            let second_state = state.clone();
            let second = scope.spawn(move || {
                second_state.preview_cover(OnlineMetadataProviderKind::MusicBrainz, &handle)
            });
            std::thread::sleep(Duration::from_millis(20));
            assert_eq!(free.lookups.load(AtomicOrdering::SeqCst), 1);
            free.release();
            assert!(first.join().unwrap().is_ok());
            assert!(second.join().unwrap().is_ok());
        });
        assert_eq!(free.lookups.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(free.fetches.load(AtomicOrdering::SeqCst), 2);
    }
    #[test]
    fn musicbrainz_uses_credit_joinphrases_only_for_the_rust_owned_artwork_lookup() {
        let mut candidate = mb_candidate();
        candidate.artists = vec!["A".into(), "B".into()];
        candidate.credited_artist = Some("A & B".into());
        let free = Arc::new(MockFreeService::new(vec![Ok(vec![candidate])]));
        let state = OnlineMetadataState::with_services(
            None,
            Arc::new(MemoryCredentialStore(Mutex::new(None))),
            Arc::new(|| 100),
            Arc::new(MockService::new(vec![])),
            free.clone(),
        );
        let handle = state
            .search_for(OnlineMetadataProviderKind::MusicBrainz, "song")
            .unwrap()[0]
            .cover_handle
            .clone()
            .unwrap();
        assert!(
            state
                .preview_cover(OnlineMetadataProviderKind::MusicBrainz, &handle)
                .is_ok()
        );
        assert_eq!(free.lookup_artists.lock().unwrap().as_slice(), ["A & B"]);
    }
    #[test]
    fn musicbrainz_cover_invalidates_during_lookup_image_and_ttl_before_publication() {
        let free = Arc::new(MockFreeService::blocking(
            vec![Ok(vec![mb_candidate()]), Ok(vec![mb_candidate()])],
            true,
            false,
        ));
        let state = OnlineMetadataState::with_services(
            None,
            Arc::new(MemoryCredentialStore(Mutex::new(None))),
            Arc::new(|| 100),
            Arc::new(MockService::new(vec![])),
            free.clone(),
        );
        let handle = state
            .search_for(OnlineMetadataProviderKind::MusicBrainz, "first")
            .unwrap()[0]
            .cover_handle
            .clone()
            .unwrap();
        std::thread::scope(|scope| {
            let preview = state.clone();
            let running = scope.spawn(move || {
                preview.preview_cover(OnlineMetadataProviderKind::MusicBrainz, &handle)
            });
            while free.lookup_started.load(AtomicOrdering::SeqCst) == 0 {
                std::thread::yield_now();
            }
            // A new explicit search invalidates the old generation while its
            // TheAudioDB lookup is still pending.
            state
                .search_for(OnlineMetadataProviderKind::MusicBrainz, "second")
                .unwrap();
            free.release();
            assert!(matches!(
                running.join().unwrap(),
                Err(OnlineMetadataError::InvalidInput)
            ));
        });

        let clock = Arc::new(AtomicU64::new(100));
        let clock_for_state = clock.clone();
        let free = Arc::new(MockFreeService::blocking(
            vec![Ok(vec![mb_candidate()])],
            false,
            true,
        ));
        let state = OnlineMetadataState::with_services(
            None,
            Arc::new(MemoryCredentialStore(Mutex::new(None))),
            Arc::new(move || clock_for_state.load(AtomicOrdering::SeqCst)),
            Arc::new(MockService::new(vec![])),
            free.clone(),
        );
        let handle = state
            .search_for(OnlineMetadataProviderKind::MusicBrainz, "song")
            .unwrap()[0]
            .cover_handle
            .clone()
            .unwrap();
        std::thread::scope(|scope| {
            let preview = state.clone();
            let running = scope.spawn(move || {
                preview.preview_cover(OnlineMetadataProviderKind::MusicBrainz, &handle)
            });
            while free.fetch_started.load(AtomicOrdering::SeqCst) == 0 {
                std::thread::yield_now();
            }
            // Disconnect and TTL expiry both make the fetched bytes ineligible
            // for publication; neither can revive a cleared handle.
            state.disconnect().unwrap();
            clock.store(
                100 + COVER_HANDLE_TTL.as_millis() as u64,
                AtomicOrdering::SeqCst,
            );
            free.release();
            assert!(matches!(
                running.join().unwrap(),
                Err(OnlineMetadataError::InvalidInput)
            ));
        });
    }
    #[test]
    fn disconnect_clears_memory_and_secure_store() {
        let store = Arc::new(MemoryCredentialStore(Mutex::new(Some(
            StoredCredential::new("r".into(), now_ms(), "id"),
        ))));
        let state = OnlineMetadataState::new(SpotifyConfig::from_client_id("id"), store.clone());
        state.disconnect().unwrap();
        assert!(store.load().unwrap().is_none());
        assert_eq!(state.status().state, OnlineMetadataConnection::Disconnected);
    }
    #[test]
    fn credential_failure_fails_closed_without_a_plaintext_fallback() {
        let state = OnlineMetadataState::new(
            SpotifyConfig::from_client_id("id"),
            Arc::new(FailingCredentialStore),
        );
        assert_eq!(
            state.status().state,
            OnlineMetadataConnection::CredentialUnavailable
        );
        assert!(matches!(
            state.disconnect(),
            Err(OnlineMetadataError::CredentialUnavailable)
        ));
    }
    #[test]
    fn oauth_save_failure_surfaces_credential_unavailable_without_reauthorization() {
        let state = OnlineMetadataState::with_service(
            SpotifyConfig::from_client_id("id"),
            Arc::new(FailingCredentialStore),
            Arc::new(|| 100),
            Arc::new(SuccessfulExchangeService),
        );
        let flow = state.begin_login().unwrap();
        assert!(matches!(
            state.complete_login(&flow, "code"),
            Err(OnlineMetadataError::CredentialUnavailable)
        ));
        assert_eq!(
            state.status_for(OnlineMetadataProviderKind::Spotify).state,
            OnlineMetadataConnection::CredentialUnavailable
        );
        assert!(!state.inner.lock().unwrap().reauthorization_required);
    }
    #[test]
    fn non_rotating_refresh_preserves_original_issued_at() {
        let now = 1_000_000_000_u64;
        let issued_at_ms = now.saturating_sub(42_000);
        let store = Arc::new(MemoryCredentialStore(Mutex::new(Some(
            StoredCredential::new("old".into(), issued_at_ms, "id"),
        ))));
        let state = OnlineMetadataState::with_clock(
            SpotifyConfig::from_client_id("id"),
            store.clone(),
            Arc::new(move || now),
        );
        state
            .save_token(
                &state.config().unwrap(),
                SpotifyToken {
                    access_token: "access".into(),
                    expires_in_secs: 3600,
                    refresh_token: None,
                },
            )
            .unwrap();
        assert_eq!(store.load().unwrap().unwrap().issued_at_ms(), issued_at_ms);
    }
    #[test]
    fn fixed_clock_marks_a_six_month_credential_for_reauthorization() {
        let now = REAUTH_AGE.as_millis() as u64;
        let store = Arc::new(MemoryCredentialStore(Mutex::new(Some(
            StoredCredential::new("r".into(), 0, "id"),
        ))));
        let state = OnlineMetadataState::with_clock(
            SpotifyConfig::from_client_id("id"),
            store,
            Arc::new(move || now),
        );
        assert_eq!(
            state.status().state,
            OnlineMetadataConnection::ReauthorizationRequired
        );
    }
    #[test]
    fn opaque_cover_handles_are_bounded_and_cleared_on_disconnect() {
        let now = 100_u64;
        let state = OnlineMetadataState::with_clock(
            SpotifyConfig::from_client_id("id"),
            Arc::new(MemoryCredentialStore(Mutex::new(None))),
            Arc::new(move || now),
        );
        let candidate = OnlineMetadataCandidate {
            provider: OnlineMetadataProviderKind::Spotify,
            requires_authorization: true,
            artwork_source: Some("Spotify"),
            attribution: "Spotify",
            item_id: "0123456789ABCDEFGHIJKL".into(),
            title: "song".into(),
            artists: vec!["artist".into()],
            album: "album".into(),
            duration_ms: 1,
            cover_handle: None,
            release_group_id: None,
            credited_artist: None,
        };
        let result =
            unlockm::metadata::provider::spotify::SpotifySearchResult::from_validated_cover_url(
                candidate,
                Some(Url::parse("https://i.scdn.co/image/x").unwrap()),
            );
        let candidates = state.bind_cover_handles(vec![result], 0);
        let handle = candidates[0].cover_handle.as_ref().unwrap();
        assert_eq!(handle.len(), 32);
        assert!(state.cover_bindings.lock().unwrap().contains_key(handle));
        state.disconnect().unwrap();
        assert!(state.cover_bindings.lock().unwrap().is_empty());
    }
    #[test]
    fn cover_handle_is_bound_to_the_provider_that_issued_it() {
        let state = OnlineMetadataState::with_clock(
            SpotifyConfig::from_client_id("id"),
            Arc::new(MemoryCredentialStore(Mutex::new(None))),
            Arc::new(|| 100),
        );
        let candidate = OnlineMetadataCandidate {
            provider: OnlineMetadataProviderKind::Spotify,
            requires_authorization: true,
            artwork_source: Some("Spotify"),
            attribution: "Spotify",
            item_id: "0123456789ABCDEFGHIJKL".into(),
            title: "song".into(),
            artists: vec!["artist".into()],
            album: "album".into(),
            duration_ms: 1,
            cover_handle: None,
            release_group_id: None,
            credited_artist: None,
        };
        let handle = state.bind_cover_handles(
            vec![SpotifySearchResult::from_validated_cover_url(
                candidate,
                Some(Url::parse("https://i.scdn.co/image/x").unwrap()),
            )],
            0,
        )[0]
        .cover_handle
        .clone()
        .unwrap();
        assert!(matches!(
            state.preview_cover(OnlineMetadataProviderKind::MusicBrainz, &handle),
            Err(OnlineMetadataError::InvalidInput)
        ));
    }
    #[test]
    fn generation_and_ttl_invalidate_cover_handles_before_fetch() {
        let clock = Arc::new(std::sync::atomic::AtomicU64::new(100));
        let clock_for_state = clock.clone();
        let state = OnlineMetadataState::with_clock(
            SpotifyConfig::from_client_id("id"),
            Arc::new(MemoryCredentialStore(Mutex::new(None))),
            Arc::new(move || clock_for_state.load(AtomicOrdering::SeqCst)),
        );
        let candidate = OnlineMetadataCandidate {
            provider: OnlineMetadataProviderKind::Spotify,
            requires_authorization: true,
            artwork_source: Some("Spotify"),
            attribution: "Spotify",
            item_id: "0123456789ABCDEFGHIJKL".into(),
            title: "song".into(),
            artists: vec!["artist".into()],
            album: "album".into(),
            duration_ms: 1,
            cover_handle: None,
            release_group_id: None,
            credited_artist: None,
        };
        let result = SpotifySearchResult::from_validated_cover_url(
            candidate,
            Some(Url::parse("https://i.scdn.co/image/x").unwrap()),
        );
        let handle = state.bind_cover_handles(vec![result], 0)[0]
            .cover_handle
            .clone()
            .unwrap();
        state.generation.fetch_add(1, AtomicOrdering::SeqCst);
        assert!(matches!(
            state.preview_cover(OnlineMetadataProviderKind::Spotify, &handle),
            Err(OnlineMetadataError::InvalidInput)
        ));
        let result = SpotifySearchResult::from_validated_cover_url(
            OnlineMetadataCandidate {
                provider: OnlineMetadataProviderKind::Spotify,
                requires_authorization: true,
                artwork_source: Some("Spotify"),
                attribution: "Spotify",
                item_id: "0123456789ABCDEFGHIJKL".into(),
                title: "song".into(),
                artists: vec!["artist".into()],
                album: "album".into(),
                duration_ms: 1,
                cover_handle: None,
                release_group_id: None,
                credited_artist: None,
            },
            Some(Url::parse("https://i.scdn.co/image/x").unwrap()),
        );
        let handle = state.bind_cover_handles(vec![result], 1)[0]
            .cover_handle
            .clone()
            .unwrap();
        clock.store(
            100 + COVER_HANDLE_TTL.as_millis() as u64,
            AtomicOrdering::SeqCst,
        );
        assert!(matches!(
            state.preview_cover(OnlineMetadataProviderKind::Spotify, &handle),
            Err(OnlineMetadataError::InvalidInput)
        ));
    }
    #[test]
    fn concurrent_searches_use_one_refresh_and_second_401_requires_reauthorization() {
        let now = 10_000_u64;
        let credential = StoredCredential::new("refresh".into(), now, "id");
        let store = Arc::new(MemoryCredentialStore(Mutex::new(Some(credential))));
        let service = Arc::new(MockService::new(vec![Ok(vec![]), Ok(vec![])]));
        let state = OnlineMetadataState::with_service(
            SpotifyConfig::from_client_id("id"),
            store,
            Arc::new(move || now),
            service.clone(),
        );
        std::thread::scope(|scope| {
            let left = state.clone();
            let right = state.clone();
            let first = scope.spawn(move || left.search("song"));
            let second = scope.spawn(move || right.search("song"));
            first.join().unwrap().unwrap();
            second.join().unwrap().unwrap();
        });
        assert_eq!(service.refreshes.load(AtomicOrdering::SeqCst), 1);

        let store = Arc::new(MemoryCredentialStore(Mutex::new(Some(
            StoredCredential::new("refresh".into(), now, "id"),
        ))));
        let service = Arc::new(MockService::new(vec![
            Err(OnlineMetadataError::ReauthorizationRequired),
            Err(OnlineMetadataError::ReauthorizationRequired),
        ]));
        let state = OnlineMetadataState::with_service(
            SpotifyConfig::from_client_id("id"),
            store,
            Arc::new(move || now),
            service.clone(),
        );
        assert!(matches!(
            state.search("song"),
            Err(OnlineMetadataError::ReauthorizationRequired)
        ));
        assert_eq!(service.refreshes.load(AtomicOrdering::SeqCst), 2);
        assert_eq!(
            state.status().state,
            OnlineMetadataConnection::ReauthorizationRequired
        );
    }

    #[test]
    fn refresh_and_replay_rate_limits_set_cooldown_and_invalid_grant_requires_reauth() {
        let now = 10_000_u64;
        let store = || {
            Arc::new(MemoryCredentialStore(Mutex::new(Some(
                StoredCredential::new("refresh".into(), now, "id"),
            ))))
        };
        let invalid = Arc::new(MockService::with_refresh_results(
            vec![Err(OnlineMetadataError::ReauthorizationRequired)],
            vec![],
        ));
        let state = OnlineMetadataState::with_service(
            SpotifyConfig::from_client_id("id"),
            store(),
            Arc::new(move || now),
            invalid,
        );
        assert!(matches!(
            state.search("song"),
            Err(OnlineMetadataError::ReauthorizationRequired)
        ));
        assert_eq!(
            state.status().state,
            OnlineMetadataConnection::ReauthorizationRequired
        );

        let refresh_limited = Arc::new(MockService::with_refresh_results(
            vec![Err(OnlineMetadataError::RateLimited {
                retry_after_secs: 7,
            })],
            vec![],
        ));
        let state = OnlineMetadataState::with_service(
            SpotifyConfig::from_client_id("id"),
            store(),
            Arc::new(move || now),
            refresh_limited,
        );
        assert!(matches!(
            state.search("song"),
            Err(OnlineMetadataError::RateLimited { .. })
        ));
        assert_eq!(state.status().retry_at_ms, Some(now + 7000));

        let search_limited = Arc::new(MockService::new(vec![Err(
            OnlineMetadataError::RateLimited {
                retry_after_secs: 8,
            },
        )]));
        let state = OnlineMetadataState::with_service(
            SpotifyConfig::from_client_id("id"),
            store(),
            Arc::new(move || now),
            search_limited,
        );
        assert!(matches!(
            state.search("song"),
            Err(OnlineMetadataError::RateLimited {
                retry_after_secs: 8
            })
        ));
        assert_eq!(state.status().retry_at_ms, Some(now + 8000));

        let replay_limited = Arc::new(MockService::with_refresh_results(
            vec![
                Ok(SpotifyToken {
                    access_token: "a".into(),
                    expires_in_secs: 1,
                    refresh_token: None,
                }),
                Ok(SpotifyToken {
                    access_token: "b".into(),
                    expires_in_secs: 3600,
                    refresh_token: None,
                }),
            ],
            vec![
                Err(OnlineMetadataError::ReauthorizationRequired),
                Err(OnlineMetadataError::RateLimited {
                    retry_after_secs: 9,
                }),
            ],
        ));
        let state = OnlineMetadataState::with_service(
            SpotifyConfig::from_client_id("id"),
            store(),
            Arc::new(move || now),
            replay_limited,
        );
        assert!(matches!(
            state.search("song"),
            Err(OnlineMetadataError::RateLimited {
                retry_after_secs: 9
            })
        ));
        assert_eq!(state.status().retry_at_ms, Some(now + 9000));
    }
    #[test]
    fn spotify_cooldown_does_not_block_the_no_auth_musicbrainz_provider() {
        let now = 10_000_u64;
        let spotify = Arc::new(MockService::new(vec![Err(
            OnlineMetadataError::RateLimited {
                retry_after_secs: 30,
            },
        )]));
        let free = Arc::new(MockFreeService::new(vec![Ok(vec![mb_candidate()])]));
        let state = OnlineMetadataState::with_services(
            SpotifyConfig::from_client_id("id"),
            Arc::new(MemoryCredentialStore(Mutex::new(Some(
                StoredCredential::new("refresh".into(), now, "id"),
            )))),
            Arc::new(move || now),
            spotify,
            free,
        );
        assert!(matches!(
            state.search_for(OnlineMetadataProviderKind::Spotify, "song"),
            Err(OnlineMetadataError::RateLimited { .. })
        ));
        assert!(
            state
                .search_for(OnlineMetadataProviderKind::MusicBrainz, "song")
                .is_ok()
        );
    }
    #[test]
    fn musicbrainz_cooldown_blocks_only_musicbrainz_and_is_exposed_by_its_status() {
        let now = 10_000_u64;
        let free = Arc::new(MockFreeService::new(vec![
            Err(OnlineMetadataError::RateLimited {
                retry_after_secs: 30,
            }),
            Ok(vec![mb_candidate()]),
        ]));
        let state = OnlineMetadataState::with_services(
            SpotifyConfig::from_client_id("id"),
            Arc::new(MemoryCredentialStore(Mutex::new(Some(
                StoredCredential::new("refresh".into(), now, "id"),
            )))),
            Arc::new(move || now),
            Arc::new(MockService::new(vec![Ok(vec![])])),
            free,
        );
        assert!(matches!(
            state.search_for(OnlineMetadataProviderKind::MusicBrainz, "song"),
            Err(OnlineMetadataError::RateLimited {
                retry_after_secs: 30
            })
        ));
        assert_eq!(
            state
                .status_for(OnlineMetadataProviderKind::MusicBrainz)
                .retry_at_ms,
            Some(now + 30_000)
        );
        assert!(matches!(
            state.search_for(OnlineMetadataProviderKind::MusicBrainz, "song"),
            Err(OnlineMetadataError::RateLimited { .. })
        ));
        assert!(
            state
                .search_for(OnlineMetadataProviderKind::Spotify, "song")
                .is_ok()
        );
    }
    #[test]
    fn disconnect_waits_for_inflight_refresh_then_prevents_token_resurrection() {
        let now = 10_000_u64;
        let store = Arc::new(MemoryCredentialStore(Mutex::new(Some(
            StoredCredential::new("refresh".into(), now, "id"),
        ))));
        let released = Arc::new((Mutex::new(false), Condvar::new()));
        let service = Arc::new(BlockingRefreshService {
            started: AtomicUsize::new(0),
            released: released.clone(),
        });
        let state = OnlineMetadataState::with_service(
            SpotifyConfig::from_client_id("id"),
            store.clone(),
            Arc::new(move || now),
            service.clone(),
        );
        std::thread::scope(|scope| {
            let search = state.clone();
            let searching = scope.spawn(move || search.search("song"));
            while service.started.load(AtomicOrdering::SeqCst) == 0 {
                std::thread::yield_now();
            }
            let disconnect = state.clone();
            let disconnecting = scope.spawn(move || disconnect.disconnect());
            {
                let (lock, wake) = &*released;
                *lock.lock().unwrap() = true;
                wake.notify_all();
            }
            searching.join().unwrap().unwrap();
            disconnecting.join().unwrap().unwrap();
        });
        assert!(store.load().unwrap().is_none());
        assert!(state.inner.lock().unwrap().access.is_none());
        assert_eq!(state.status().state, OnlineMetadataConnection::Disconnected);
    }
    #[test]
    fn disconnect_waits_for_inflight_code_exchange_then_prevents_credential_resurrection() {
        let now = 10_000_u64;
        let store = Arc::new(MemoryCredentialStore(Mutex::new(None)));
        let released = Arc::new((Mutex::new(false), Condvar::new()));
        let service = Arc::new(BlockingExchangeService {
            started: AtomicUsize::new(0),
            released: released.clone(),
        });
        let state = OnlineMetadataState::with_service(
            SpotifyConfig::from_client_id("id"),
            store.clone(),
            Arc::new(move || now),
            service.clone(),
        );
        let flow = state.begin_login().unwrap();
        std::thread::scope(|scope| {
            let exchanging = scope.spawn(|| state.complete_login(&flow, "code"));
            while service.started.load(AtomicOrdering::SeqCst) == 0 {
                std::thread::yield_now();
            }
            let disconnecting = scope.spawn(|| state.disconnect());
            {
                let (lock, wake) = &*released;
                *lock.lock().unwrap() = true;
                wake.notify_all();
            }
            assert!(exchanging.join().unwrap().is_ok());
            disconnecting.join().unwrap().unwrap();
        });
        assert!(store.load().unwrap().is_none());
        assert!(state.inner.lock().unwrap().access.is_none());
        assert_eq!(state.status().state, OnlineMetadataConnection::Disconnected);
    }

    #[test]
    fn preview_races_and_gate_are_deterministically_bounded() {
        let now = Arc::new(std::sync::atomic::AtomicU64::new(100));
        let clock = now.clone();
        let released = Arc::new((Mutex::new(false), Condvar::new()));
        let service = Arc::new(BlockingCoverService {
            started: AtomicUsize::new(0),
            released: released.clone(),
        });
        let state = OnlineMetadataState::with_service(
            SpotifyConfig::from_client_id("id"),
            Arc::new(MemoryCredentialStore(Mutex::new(None))),
            Arc::new(move || clock.load(AtomicOrdering::SeqCst)),
            service.clone(),
        );
        let make = |suffix: &str| {
            SpotifySearchResult::from_validated_cover_url(
                OnlineMetadataCandidate {
                    provider: OnlineMetadataProviderKind::Spotify,
                    requires_authorization: true,
                    artwork_source: Some("Spotify"),
                    attribution: "Spotify",
                    item_id: format!("0123456789ABCDEFGHIJK{suffix}"),
                    title: "s".into(),
                    artists: vec!["a".into()],
                    album: "a".into(),
                    duration_ms: 1,
                    cover_handle: None,
                    release_group_id: None,
                    credited_artist: None,
                },
                Some(Url::parse("https://i.scdn.co/image/x").unwrap()),
            )
        };
        let handles = state
            .bind_cover_handles(vec![make("A"), make("B"), make("C")], 0)
            .into_iter()
            .map(|candidate| candidate.cover_handle.unwrap())
            .collect::<Vec<_>>();
        std::thread::scope(|scope| {
            let a = state.clone();
            let b = state.clone();
            let c = state.clone();
            let first_handle = handles[0].clone();
            let second_handle = handles[1].clone();
            let third_handle = handles[2].clone();
            let first = scope
                .spawn(move || a.preview_cover(OnlineMetadataProviderKind::Spotify, &first_handle));
            let second = scope.spawn(move || {
                b.preview_cover(OnlineMetadataProviderKind::Spotify, &second_handle)
            });
            while service.started.load(AtomicOrdering::SeqCst) < 2 {
                std::thread::yield_now();
            }
            let third = scope
                .spawn(move || c.preview_cover(OnlineMetadataProviderKind::Spotify, &third_handle));
            std::thread::sleep(Duration::from_millis(20));
            assert_eq!(service.started.load(AtomicOrdering::SeqCst), 2);
            state.invalidate_cover_bindings();
            {
                let (lock, wake) = &*released;
                *lock.lock().unwrap() = true;
                wake.notify_all();
            }
            assert!(matches!(
                first.join().unwrap(),
                Err(OnlineMetadataError::InvalidInput)
            ));
            assert!(matches!(
                second.join().unwrap(),
                Err(OnlineMetadataError::InvalidInput)
            ));
            assert!(matches!(
                third.join().unwrap(),
                Err(OnlineMetadataError::InvalidInput)
            ));
        });
        now.store(
            100 + COVER_HANDLE_TTL.as_millis() as u64,
            AtomicOrdering::SeqCst,
        );
    }
}
