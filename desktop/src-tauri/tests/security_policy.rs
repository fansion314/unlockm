use serde_json::Value;
use std::{fs, path::Path};

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
    let csp = config["app"]["security"]["csp"].as_str().unwrap();
    assert!(!csp.contains("https://"));
    assert!(!csp.contains("data: script"));
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
