fn main() {
    println!("cargo:rerun-if-env-changed=UNLOCKM_SPOTIFY_CLIENT_ID");
    if let Ok(client_id) = std::env::var("UNLOCKM_SPOTIFY_CLIENT_ID")
        && !client_id.trim().is_empty()
    {
        // A Spotify client ID is intentionally public configuration. Do not
        // add a client secret here: desktop OAuth uses PKCE.
        println!("cargo:rustc-env=UNLOCKM_SPOTIFY_CLIENT_ID={client_id}");
    }
    tauri_build::build()
}
