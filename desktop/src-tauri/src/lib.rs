//! In-process desktop host for the UnlockM conversion engine.
//!
//! Tauri and application-specific code is added here without changing the
//! root crate's CLI boundary. The path dependency is intentional: conversion
//! always calls the Rust library and never launches the `unlockm` executable.

pub mod audio_protocol;
pub mod commands;
pub mod history;
pub mod state;

use std::path::{Path, PathBuf};
use tauri::{
    AppHandle, Manager,
    menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu},
};

pub const PRODUCT_NAME: &str = "UnlockM";
pub const APP_IDENTIFIER: &str = "world.fansionia.unlockm";
const LEGACY_APP_IDENTIFIER: &str = "world.fansion.unlockm";

fn about_metadata() -> AboutMetadata<'static> {
    let credits = "感谢加性饼干提供的词元，为魔仙煲和文森特李73制作\n\nThanks to Additive Cookie for providing the tokens.\nMade for Moxianbao and vincentlee73.";
    AboutMetadata {
        name: Some(PRODUCT_NAME.into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        comments: Some(credits.into()),
        copyright: Some("Copyright © Hamlet Fansion (Peilin Fan)".into()),
        credits: Some(credits.into()),
        ..Default::default()
    }
}

#[cfg(target_os = "macos")]
fn application_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let about = about_metadata();
    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    let help_menu = Submenu::with_items(app, "Help", true, &[])?;
    Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                PRODUCT_NAME,
                true,
                &[
                    &PredefinedMenuItem::about(app, None, Some(about))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::services(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, None)?,
                    &PredefinedMenuItem::hide_others(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::quit(app, None)?,
                ],
            )?,
            &Submenu::with_items(
                app,
                "File",
                true,
                &[&PredefinedMenuItem::close_window(app, None)?],
            )?,
            &Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app, None)?,
                    &PredefinedMenuItem::redo(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, None)?,
                    &PredefinedMenuItem::copy(app, None)?,
                    &PredefinedMenuItem::paste(app, None)?,
                    &PredefinedMenuItem::select_all(app, None)?,
                ],
            )?,
            &Submenu::with_items(
                app,
                "View",
                true,
                &[&PredefinedMenuItem::fullscreen(app, None)?],
            )?,
            &window_menu,
            &help_menu,
        ],
    )
}

#[cfg(not(target_os = "macos"))]
fn application_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &PredefinedMenuItem::close_window(app, None)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    let help_menu = Submenu::with_items(
        app,
        "Help",
        true,
        &[&PredefinedMenuItem::about(
            app,
            None,
            Some(about_metadata()),
        )?],
    )?;
    Menu::with_items(app, &[&file_menu, &edit_menu, &window_menu, &help_menu])
}

fn history_path_with_legacy_backup(app_data: &Path) -> Result<PathBuf, history::HistoryError> {
    let history_path = app_data.join("history.sqlite");
    if history_path.exists() {
        return Ok(history_path);
    }
    let legacy = app_data
        .parent()
        .filter(|_| {
            app_data
                .file_name()
                .is_some_and(|name| name == APP_IDENTIFIER)
        })
        .map(|parent| parent.join(LEGACY_APP_IDENTIFIER).join("history.sqlite"));
    if let Some(legacy) = legacy {
        history::HistoryStore::backup_legacy_database(legacy, &history_path)?;
    }
    Ok(history_path)
}

pub fn run() {
    tauri::Builder::default()
        .menu(application_menu)
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
        .register_asynchronous_uri_scheme_protocol(
            "unlockm-audio",
            |context, request, responder| {
                let history = context
                    .app_handle()
                    .state::<state::AppState>()
                    .history()
                    .clone();
                tauri::async_runtime::spawn(async move {
                    let response = tauri::async_runtime::spawn_blocking(move || {
                        audio_protocol::response(&history, request)
                    })
                    .await
                    .unwrap_or_else(|_| {
                        tauri::http::Response::builder()
                            .status(tauri::http::StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Vec::new())
                            .unwrap()
                    });
                    responder.respond(response);
                });
            },
        )
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let history = history::HistoryStore::open(history_path_with_legacy_backup(&app_data)?)?;
            let state = state::AppState::new(history)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::validate_input,
            commands::list_runs,
            commands::query_run_items,
            commands::list_run_item_facets,
            commands::get_artwork,
            commands::get_artwork_mime,
            commands::get_preferences,
            commands::set_preferences,
            commands::reveal_run_output,
            commands::reveal_run_item,
            commands::open_run_item,
            commands::delete_run,
            commands::clear_history,
            commands::start_conversion,
            commands::cancel_conversion,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run UnlockM");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{CreateRun, InputKind, RunSettings, Theme};

    #[test]
    fn about_metadata_contains_requested_credits_and_copyright() {
        let metadata = about_metadata();
        assert_eq!(metadata.name.as_deref(), Some("UnlockM"));
        assert_eq!(
            metadata.copyright.as_deref(),
            Some("Copyright © Hamlet Fansion (Peilin Fan)")
        );
        let credits = metadata.credits.unwrap();
        assert_eq!(metadata.comments.as_deref(), Some(credits.as_str()));
        for required in [
            "加性饼干",
            "魔仙煲",
            "文森特李73",
            "Additive Cookie",
            "Moxianbao",
            "vincentlee73",
        ] {
            assert!(
                credits.contains(required),
                "missing About credit {required}"
            );
        }
    }

    #[test]
    fn corrected_identifier_backs_up_legacy_history_once() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_data = directory.path().join(LEGACY_APP_IDENTIFIER);
        let new_data = directory.path().join(APP_IDENTIFIER);
        let legacy_path = legacy_data.join("history.sqlite");
        let legacy = history::HistoryStore::open(&legacy_path).unwrap();
        let run = legacy
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: "/input/song.ncm".into(),
                output_path: "/output".into(),
                settings: RunSettings {
                    update_metadata: false,
                    copy_passthrough: true,
                    overwrite: false,
                },
                total_items: 1,
            })
            .unwrap();
        let mut preferences = legacy.preferences().unwrap();
        preferences.locale = "zh-CN".into();
        preferences.theme = Theme::Dark;
        legacy.set_preferences(&preferences).unwrap();
        drop(legacy);

        let migrated_path = history_path_with_legacy_backup(&new_data).unwrap();
        assert_eq!(migrated_path, new_data.join("history.sqlite"));
        assert!(
            legacy_path.is_file(),
            "legacy history must remain recoverable"
        );
        let migrated = history::HistoryStore::open(&migrated_path).unwrap();
        assert_eq!(
            migrated.run(run.id).unwrap().unwrap().input_path,
            Path::new("/input/song.ncm")
        );
        assert_eq!(migrated.preferences().unwrap(), preferences);
        drop(migrated);

        assert_eq!(
            history_path_with_legacy_backup(&new_data).unwrap(),
            migrated_path
        );
    }
}
