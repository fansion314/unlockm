use serde_json::Value;
use std::{fs, path::Path};

fn desktop_file(relative: &str) -> String {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest.join(relative)).unwrap()
}

#[test]
fn bundled_font_license_and_notices_are_complete_and_declared_as_resources() {
    let ofl = desktop_file("../../third_party/licenses/SIL-OFL-1.1.txt");
    assert!(ofl.contains("SIL OPEN FONT LICENSE Version 1.1"));
    assert!(ofl.contains("PERMISSION & CONDITIONS"));
    assert!(ofl.contains("THE FONT SOFTWARE IS PROVIDED \"AS IS\""));

    let package: Value = serde_json::from_str(&desktop_file("../package.json")).unwrap();
    assert_eq!(package["dependencies"]["@phosphor-icons/react"], "2.1.10");
    for dependency in [
        "@fontsource-variable/bricolage-grotesque",
        "@fontsource-variable/manrope",
    ] {
        assert_eq!(
            package["dependencies"][dependency], "5.3.0",
            "desktop package must pin {dependency} to the reviewed Fontsource version"
        );
    }
    let lockfile = desktop_file("../pnpm-lock.yaml");
    assert!(lockfile.contains("'@fontsource-variable/bricolage-grotesque@5.3.0'"));
    assert!(lockfile.contains("'@fontsource-variable/manrope@5.3.0'"));
    assert!(lockfile.contains("'@phosphor-icons/react@2.1.10'"));
    let entry = desktop_file("../src/main.tsx");
    assert!(entry.contains("@fontsource-variable/bricolage-grotesque"));
    assert!(entry.contains("@fontsource-variable/manrope"));

    let config: Value = serde_json::from_str(&desktop_file("tauri.conf.json")).unwrap();
    let resources = &config["bundle"]["resources"];
    assert_eq!(
        resources["../../third_party/licenses/SIL-OFL-1.1.txt"],
        "licenses/SIL-OFL-1.1.txt"
    );
    assert_eq!(
        resources["../../THIRD_PARTY_NOTICES.md"],
        "licenses/THIRD_PARTY_NOTICES.md"
    );

    let notices = desktop_file("../../THIRD_PARTY_NOTICES.md");
    let cargo = desktop_file("Cargo.toml");
    let lock = desktop_file("../../Cargo.lock");
    assert!(cargo.contains("keyring = \"=4.1.6\""));
    assert!(lock.contains("name = \"apple-native-keyring-store\""));
    assert!(lock.contains("name = \"windows-native-keyring-store\""));
    for required in [
        "Fontsource",
        "Bricolage Grotesque",
        "Manrope",
        "SIL OFL-1.1",
        "@phosphor-icons/react",
        "tauri-plugin-opener",
        "keyring` 4.1.6",
        "Apache-2.0 OR MIT",
        "Bundled color theme references",
        "Tokyo Night",
        "cdc07ac78467a233fd62c493de29a17e0cf2b2b6",
        "Catppuccin Mocha",
        "07d02aa110ef9eb7e7427afca5c73ba9cf7f8ebd",
        "Dracula",
        "2985f660b04e6961b0887ffac2f8d3f35f431698",
        "Nord",
        "1cef71605416a222e57225b544540ce0fcec18d4",
        "Gruvbox Light",
        "5d15b2765f59754d7ac263c88a0f6e3e58124951",
        "Rosé Pine Dawn",
        "92af52b465ab6e47437aca223c9b8d3009a2023b",
        "MIT/X11",
        "licensed Apache-2.0",
    ] {
        assert!(
            notices.contains(required),
            "notice must identify {required}"
        );
    }
}
