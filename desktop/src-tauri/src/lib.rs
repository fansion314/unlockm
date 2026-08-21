//! In-process desktop host for the UnlockM conversion engine.
//!
//! Tauri and application-specific code is added here without changing the
//! root crate's CLI boundary. The path dependency is intentional: conversion
//! always calls the Rust library and never launches the `unlockm` executable.

pub mod commands;
pub mod history;
pub mod state;

use tauri::Manager;

pub const PRODUCT_NAME: &str = "UnlockM";

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let history = history::HistoryStore::open(app_data.join("history.sqlite"))?;
            let state = state::AppState::new(history)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::validate_input,
            commands::list_runs,
            commands::get_run_items,
            commands::get_artwork,
            commands::get_artwork_mime,
            commands::get_preferences,
            commands::set_preferences,
            commands::delete_run,
            commands::clear_history,
            commands::start_conversion,
            commands::cancel_conversion,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run UnlockM");
}
