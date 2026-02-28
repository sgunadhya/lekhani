mod adapters;
mod application;
mod domain;
mod ports;

use adapters::db::{
    MemoryNarrativeRepository, MemoryScreenplayRepository, SqliteScreenplayRepository,
};
use adapters::llm::StubNarrativeEngine;
use adapters::tauri::{
    export_fountain_document, get_active_screenplay, get_current_project,
    get_narrative_snapshot, get_nudge, get_screenplays, get_time, import_fountain_document,
    open_project_document, parse_character, parse_event, save_project_document_as,
    save_screenplay, AppState,
};
use std::sync::Arc;
use tauri::{Emitter, Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let _ = app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                );
            }

            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|err| err.to_string())?;
            std::fs::create_dir_all(&app_data_dir).map_err(|err| err.to_string())?;
            let db_path = app_data_dir.join("lekhani.sqlite3");

            let screenplay_repository: Box<dyn ports::ScreenplayRepository>;
            let narrative_repository: Box<dyn ports::NarrativeRepository>;
            let sqlite_repository: Option<Arc<SqliteScreenplayRepository>>;

            match SqliteScreenplayRepository::open(&db_path) {
                Ok(repository) => {
                    let repository = Arc::new(repository);
                    screenplay_repository = Box::new(repository.clone());
                    narrative_repository = Box::new(repository.clone());
                    sqlite_repository = Some(repository);
                }
                Err(_) => {
                    screenplay_repository = Box::<MemoryScreenplayRepository>::default();
                    narrative_repository = Box::<MemoryNarrativeRepository>::default();
                    sqlite_repository = None;
                }
            }

            app.manage(AppState::new(
                screenplay_repository,
                narrative_repository,
                sqlite_repository,
                Box::<StubNarrativeEngine>::default(),
                Box::<StubNarrativeEngine>::default(),
                Box::<StubNarrativeEngine>::default(),
            ));

            if let Some(project_path) = startup_project_path() {
                let state = app.state::<AppState>();
                let _ = state.switch_project_path(&project_path);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_time,
            get_active_screenplay,
            get_current_project,
            get_narrative_snapshot,
            get_screenplays,
            save_screenplay,
            import_fountain_document,
            export_fountain_document,
            open_project_document,
            save_project_document_as,
            parse_character,
            parse_event,
            get_nudge
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| match event {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            RunEvent::Opened { urls } => {
                if let Some(project_path) = urls.iter().find_map(|url| {
                    if url.scheme() != "file" {
                        return None;
                    }

                    let path = url.to_file_path().ok()?;
                    let is_project = matches!(
                        path.extension().and_then(std::ffi::OsStr::to_str),
                        Some(ext) if ext.eq_ignore_ascii_case("lekhani")
                    );

                    if is_project { Some(path) } else { None }
                }) {
                    let state = app.state::<AppState>();
                    if state.switch_project_path(&project_path).is_ok() {
                        let _ = app.emit("project-opened", project_path.to_string_lossy().to_string());
                    }
                }
            }
            #[cfg(target_os = "macos")]
            RunEvent::Reopen {
                has_visible_windows,
                ..
            } if !has_visible_windows => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        });
}

fn startup_project_path() -> Option<std::path::PathBuf> {
    std::env::args_os()
        .skip(1)
        .map(std::path::PathBuf::from)
        .find(|path| {
            matches!(
                path.extension().and_then(std::ffi::OsStr::to_str),
                Some(ext) if ext.eq_ignore_ascii_case("lekhani")
            )
                && path.exists()
        })
}
