//! Provider-neutral IPC boundary for explicit online metadata actions. The
//! webview supplies only the fixed provider enum, a bounded query, or an
//! opaque Spotify track ID—never a URL, token, callback, or filesystem path.

use super::{CommandError, CommandResult, blocking};
use crate::online_metadata::state::{OnlineMetadataState, OnlineMetadataStatus};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use unlockm::metadata::provider::spotify::SpotifyClient;
use unlockm::metadata::provider::{
    ONLINE_METADATA_PROVIDER_CAPABILITIES, OnlineMetadataCandidate, OnlineMetadataError,
    OnlineMetadataProviderCapability, OnlineMetadataProviderKind,
};

fn command_error(error: OnlineMetadataError) -> CommandError {
    CommandError {
        code: error.code(),
        detail: "Online metadata request could not be completed.".into(),
    }
}

fn spotify(provider: OnlineMetadataProviderKind) -> Result<(), CommandError> {
    match provider {
        OnlineMetadataProviderKind::Spotify => Ok(()),
        OnlineMetadataProviderKind::MusicBrainz => {
            Err(command_error(OnlineMetadataError::InvalidInput))
        }
    }
}

fn supported(_provider: OnlineMetadataProviderKind) -> Result<(), CommandError> {
    Ok(())
}

#[tauri::command]
pub fn online_metadata_status(
    state: State<'_, OnlineMetadataState>,
    provider: OnlineMetadataProviderKind,
) -> OnlineMetadataStatus {
    state.status_for(provider)
}

#[tauri::command]
pub fn online_metadata_provider_capabilities() -> Vec<OnlineMetadataProviderCapability> {
    ONLINE_METADATA_PROVIDER_CAPABILITIES.to_vec()
}

#[tauri::command]
pub async fn start_online_metadata_login(
    app: AppHandle,
    state: State<'_, OnlineMetadataState>,
    provider: OnlineMetadataProviderKind,
) -> CommandResult<OnlineMetadataStatus> {
    spotify(provider)?;
    let state = state.inner().clone();
    let flow = state.begin_login().map_err(command_error)?;
    let config = state
        .config()
        .ok_or_else(|| command_error(OnlineMetadataError::NotConfigured))?;
    let url =
        SpotifyClient::authorization_url(&config, &flow.redirect_uri, &flow.state, &flow.challenge)
            .map_err(command_error)?;
    if app
        .opener()
        .open_url(url.to_string(), None::<String>)
        .is_err()
    {
        state.abort_login();
        return Err(command_error(OnlineMetadataError::AuthorizationFailed));
    }
    blocking(move || {
        let code = flow.wait_for_code();
        let result = code.and_then(|code| state.complete_login(&flow, &code));
        if result.is_err() {
            state.abort_login();
        }
        result.map_err(command_error)?;
        Ok(state.status_for(OnlineMetadataProviderKind::Spotify))
    })
    .await
}

#[tauri::command]
pub fn cancel_online_metadata_login(
    state: State<'_, OnlineMetadataState>,
    provider: OnlineMetadataProviderKind,
) -> Result<bool, CommandError> {
    spotify(provider)?;
    Ok(state.cancel_login())
}

#[tauri::command]
pub async fn disconnect_online_metadata(
    state: State<'_, OnlineMetadataState>,
    provider: OnlineMetadataProviderKind,
) -> CommandResult<OnlineMetadataStatus> {
    spotify(provider)?;
    let state = state.inner().clone();
    blocking(move || {
        state.disconnect().map_err(command_error)?;
        Ok(state.status_for(OnlineMetadataProviderKind::Spotify))
    })
    .await
}

#[tauri::command]
pub async fn search_online_metadata(
    state: State<'_, OnlineMetadataState>,
    provider: OnlineMetadataProviderKind,
    query: String,
) -> CommandResult<Vec<OnlineMetadataCandidate>> {
    supported(provider)?;
    let state = state.inner().clone();
    blocking(move || state.search_for(provider, &query).map_err(command_error)).await
}

#[tauri::command]
pub async fn get_online_metadata_cover_preview(
    state: State<'_, OnlineMetadataState>,
    provider: OnlineMetadataProviderKind,
    cover_handle: String,
) -> CommandResult<Vec<u8>> {
    supported(provider)?;
    let state = state.inner().clone();
    blocking(move || {
        state
            .preview_cover(provider, &cover_handle)
            .map_err(command_error)
    })
    .await
}

#[tauri::command]
pub fn open_online_metadata_result(
    app: AppHandle,
    provider: OnlineMetadataProviderKind,
    item_id: String,
) -> Result<bool, CommandError> {
    let url = match provider {
        OnlineMetadataProviderKind::Spotify => {
            unlockm::metadata::provider::spotify::open_track_url(&item_id)
        }
        OnlineMetadataProviderKind::MusicBrainz => {
            unlockm::metadata::provider::musicbrainz::open_recording_url(&item_id)
        }
    }
    .map_err(command_error)?;
    app.opener()
        .open_url(url.to_string(), None::<String>)
        .map_err(|_| command_error(OnlineMetadataError::Unavailable))?;
    Ok(true)
}
