use serde_json::Value;
use std::{fs, path::Path};
use unlockm::metadata::provider::{
    FREE_ARTWORK_SOURCE, FREE_ATTRIBUTION, OnlineMetadataCandidate, OnlineMetadataProviderKind,
    coverartarchive::{
        API_HOST as CAA_API_HOST, ARCHIVE_HOST as CAA_ARCHIVE_HOST,
        ARCHIVE_SUBDOMAIN_SUFFIX as CAA_ARCHIVE_SUFFIX, MAX_REDIRECTS as CAA_MAX_REDIRECTS,
    },
    musicbrainz::{HOST as MUSICBRAINZ_HOST, ROOT as MUSICBRAINZ_ROOT, USER_AGENT},
    spotify::{
        SPOTIFY_ACCOUNTS_HOST, SPOTIFY_API_HOST, SPOTIFY_AUTHORIZE_ENDPOINT, SPOTIFY_IMAGE_HOSTS,
        SPOTIFY_OPEN_HOST, SPOTIFY_SEARCH_ENDPOINT, SPOTIFY_TOKEN_ENDPOINT, open_track_url,
    },
    theaudiodb::{
        API_HOST as THEAUDIODB_API_HOST, IMAGE_HOST as THEAUDIODB_IMAGE_HOST, PUBLIC_KEY,
    },
};

fn workspace_file(relative: &str) -> String {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest.join(relative)).unwrap()
}

#[test]
fn capabilities_and_csp_exclude_broad_plugins_and_external_assets() {
    let capabilities: Value =
        serde_json::from_str(&workspace_file("capabilities/default.json")).unwrap();
    assert_eq!(
        capabilities["permissions"],
        serde_json::json!([
            "core:event:allow-listen",
            "core:event:allow-unlisten",
            "dialog:allow-open"
        ])
    );
    let permissions_text = capabilities["permissions"].to_string();
    for forbidden in [
        "core:default",
        "core:event:default",
        "core:event:allow-emit",
        "core:path:default",
        "core:menu:default",
        "core:tray:default",
        "core:webview:allow-internal-toggle-devtools",
        "dialog:default",
    ] {
        assert!(
            !permissions_text.contains(forbidden),
            "desktop capabilities must not grant {forbidden}"
        );
    }
    let config: Value = serde_json::from_str(&workspace_file("tauri.conf.json")).unwrap();
    assert_eq!(config["identifier"], "world.fansionia.unlockm");
    let csp = config["app"]["security"]["csp"].as_str().unwrap();
    assert!(!csp.contains("https://"));
    assert!(!csp.contains("data: script"));
    assert!(csp.contains("media-src 'self' unlockm-audio: http://unlockm-audio.localhost"));
    assert!(!csp.contains("asset:"));
    assert!(!csp.contains("http://asset.localhost"));
    assert!(config["app"]["security"].get("assetProtocol").is_none());
    let config_text = config.to_string();
    for forbidden in [
        "sidecar",
        "shell",
        "plugin-sql",
        "plugin-fs",
        "updater",
        "externalBin",
        "core:default",
    ] {
        assert!(
            !config_text.contains(forbidden),
            "desktop configuration must not grant {forbidden}"
        );
    }
}

#[test]
fn desktop_host_never_spawns_unlockm_or_a_helper_process() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = rust_sources(&root.join("src"));
    assert!(!sources.is_empty());
    for source in sources {
        assert!(!source.contains("std::process::Command"));
        assert!(!source.contains("Command::new"));
        assert!(!source.contains("sidecar"));
    }
    let cargo = workspace_file("Cargo.toml");
    for forbidden in ["tauri-plugin-shell", "tauri-plugin-fs", "tauri-plugin-sql"] {
        assert!(!cargo.contains(forbidden));
    }
    assert!(cargo.contains("tauri-plugin-opener = \"=2.5.4\""));
    let capabilities = workspace_file("capabilities/default.json");
    assert!(!capabilities.contains("opener:"));
}

#[test]
fn windows_release_gui_subsystem_and_payload_check_are_configured() {
    let main = workspace_file("src/main.rs");
    let compact_main: String = main
        .chars()
        .filter(|value| !value.is_whitespace())
        .collect();
    assert!(compact_main.contains(
        "#![cfg_attr(all(target_os=\"windows\",not(debug_assertions)),windows_subsystem=\"windows\")]"
    ));

    let release_workflow = workspace_file("../../.github/workflows/desktop-release.yml");
    assert!(release_workflow.contains("Get-PeSubsystem"));
    assert!(release_workflow.contains("DOS MZ header is missing"));
    assert!(release_workflow.contains("$optionalHeaderSize -lt 70"));
    assert!(release_workflow.contains("IMAGE_SUBSYSTEM_WINDOWS_GUI"));
    assert!(release_workflow.contains("$subsystem -ne 2"));
}

#[test]
fn online_metadata_boundary_is_handle_only_and_release_has_no_secret_path() {
    let commands = workspace_file("src/commands/online_metadata.rs");
    let store = workspace_file("src/online_metadata/credential_store.rs");
    let spotify = workspace_file("../../src/metadata/provider/spotify.rs");
    let musicbrainz = workspace_file("../../src/metadata/provider/musicbrainz.rs");
    let coverartarchive = workspace_file("../../src/metadata/provider/coverartarchive.rs");
    let free_artwork = workspace_file("../../src/metadata/provider/free_artwork.rs");
    let theaudiodb = workspace_file("../../src/metadata/provider/theaudiodb.rs");
    let provider_contract = workspace_file("../../src/metadata/provider/mod.rs");
    let contracts = workspace_file("../src/lib/contracts.ts");
    let release = workspace_file("../../.github/workflows/desktop-release.yml");
    let build = workspace_file("build.rs");
    let config: Value = serde_json::from_str(&workspace_file("tauri.conf.json")).unwrap();
    for forbidden in ["url: String", "token: String", "image_url", "imageUrl"] {
        assert!(!commands.contains(forbidden));
    }
    assert!(commands.contains("cover_handle: String"));
    assert!(commands.contains("provider: OnlineMetadataProviderKind,\n    query: String"));
    assert!(commands.contains("provider: OnlineMetadataProviderKind,\n    cover_handle: String"));
    assert!(commands.contains("spotify::open_track_url(&item_id)"));
    assert!(commands.contains("musicbrainz::open_recording_url(&item_id)"));
    assert!(!commands.contains("open_url(item_id"));
    assert!(!contracts.contains("imageUrl"));
    assert!(!contracts.contains("releaseGroupId"));
    assert!(provider_contract.contains("#[serde(skip_serializing)]\n    pub release_group_id"));
    let candidate = OnlineMetadataCandidate {
        provider: OnlineMetadataProviderKind::MusicBrainz,
        requires_authorization: false,
        artwork_source: Some(FREE_ARTWORK_SOURCE),
        attribution: FREE_ATTRIBUTION,
        item_id: "11111111-1111-1111-1111-111111111111".into(),
        title: "title".into(),
        artists: vec!["artist".into()],
        album: "album".into(),
        duration_ms: 1,
        cover_handle: Some("opaque".into()),
        release_group_id: Some("22222222-2222-2222-2222-222222222222".into()),
        credited_artist: Some("artist feat. guest".into()),
    };
    let dto = serde_json::to_value(candidate).unwrap();
    assert_eq!(dto["coverHandle"], "opaque");
    assert!(dto.get("releaseGroupId").is_none());
    assert!(dto.get("creditedArtist").is_none());
    assert!(store.contains("SystemCredentialStore"));
    assert!(store.contains("keyring::Entry"));
    assert!(!store.contains("history.sqlite"));
    assert!(!store.contains("std::fs"));
    assert!(!store.contains("localStorage"));
    for forbidden in [
        "rusqlite",
        "Connection::open",
        "File::",
        "OpenOptions",
        "std::env::var",
        "from_reader",
        "to_writer",
    ] {
        assert!(
            !store.contains(forbidden),
            "credential store must not use {forbidden}"
        );
    }
    assert_eq!(SPOTIFY_ACCOUNTS_HOST, "accounts.spotify.com");
    assert_eq!(SPOTIFY_API_HOST, "api.spotify.com");
    assert_eq!(SPOTIFY_OPEN_HOST, "open.spotify.com");
    assert_eq!(
        SPOTIFY_TOKEN_ENDPOINT,
        "https://accounts.spotify.com/api/token"
    );
    assert_eq!(SPOTIFY_SEARCH_ENDPOINT, "https://api.spotify.com/v1/search");
    assert_eq!(
        SPOTIFY_AUTHORIZE_ENDPOINT,
        "https://accounts.spotify.com/authorize"
    );
    assert_eq!(
        SPOTIFY_IMAGE_HOSTS,
        ["i.scdn.co", "image-cdn-ak.spotifycdn.com"]
    );
    assert_eq!(
        open_track_url("0123456789ABCDEFGHIJKL").unwrap().as_str(),
        "https://open.spotify.com/track/0123456789ABCDEFGHIJKL"
    );
    assert!(spotify.contains("Policy::none"));
    assert_eq!(MUSICBRAINZ_ROOT, "https://musicbrainz.org/ws/2/");
    assert_eq!(MUSICBRAINZ_HOST, "musicbrainz.org");
    assert!(USER_AGENT.starts_with("UnlockM/"));
    assert!(USER_AGENT.contains("https://git.fansionia.world/fansion/unlockm"));
    assert!(musicbrainz.contains("USER_AGENT"));
    assert!(musicbrainz.contains("OneRequestPerSecond"));
    assert!(musicbrainz.contains("Policy::none"));
    assert_eq!(CAA_API_HOST, "coverartarchive.org");
    assert_eq!(CAA_ARCHIVE_HOST, "archive.org");
    assert_eq!(CAA_ARCHIVE_SUFFIX, ".archive.org");
    assert_eq!(CAA_MAX_REDIRECTS, 3);
    assert!(coverartarchive.contains("Policy::none"));
    assert!(coverartarchive.contains("Policy::custom"));
    assert!(coverartarchive.contains("trusted_archive_url"));
    assert!(coverartarchive.contains("validate_cover"));
    assert!(free_artwork.contains("cover_art_archive.cover_url"));
    assert!(free_artwork.contains("the_audio_db.cover_url"));
    assert_eq!(THEAUDIODB_API_HOST, "www.theaudiodb.com");
    assert_eq!(THEAUDIODB_IMAGE_HOST, "r2.theaudiodb.com");
    assert_eq!(PUBLIC_KEY, "123");
    assert!(theaudiodb.contains("Policy::none"));
    assert!(theaudiodb.contains("validate_cover"));
    let csp = config["app"]["security"]["csp"].as_str().unwrap();
    assert_eq!(
        csp_sources(csp, "connect-src"),
        [
            "ipc:",
            "http://ipc.localhost",
            "http://127.0.0.1:1420",
            "ws://127.0.0.1:1420"
        ]
    );
    assert_eq!(csp_sources(csp, "img-src"), ["'self'", "blob:", "data:"]);
    assert_eq!(
        csp_sources(csp, "media-src"),
        ["'self'", "unlockm-audio:", "http://unlockm-audio.localhost"]
    );
    assert!(release.contains("Validate public Spotify configuration"));
    assert!(release.contains("UNLOCKM_SPOTIFY_CLIENT_ID"));
    assert!(release.contains("forbidden=\"SPOTIFY_CLIENT\"\"_SECRET\""));
    assert!(release.contains("vars.UNLOCKM_SPOTIFY_CLIENT_ID"));
    assert_eq!(
        release
            .matches("UNLOCKM_SPOTIFY_CLIENT_ID: ${{ vars.UNLOCKM_SPOTIFY_CLIENT_ID || '' }}")
            .count(),
        2
    );
    assert!(!release.contains("${{ secrets."));
    assert!(!release.contains("secrets.UNLOCKM_SPOTIFY"));
    assert!(!release.contains("SPOTIFY_CLIENT_SECRET"));
    assert!(build.contains("rerun-if-env-changed=UNLOCKM_SPOTIFY_CLIENT_ID"));
    assert!(!build.contains("SPOTIFY_CLIENT_SECRET"));
}

fn csp_sources<'a>(csp: &'a str, directive: &str) -> Vec<&'a str> {
    csp.split(';')
        .map(str::trim)
        .find_map(|entry| entry.strip_prefix(directive).map(str::trim))
        .expect("required CSP directive")
        .split_whitespace()
        .collect()
}

fn rust_sources(directory: &Path) -> Vec<String> {
    let mut sources = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            sources.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(fs::read_to_string(path).unwrap());
        }
    }
    sources
}
