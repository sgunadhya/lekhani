mod adapters;
mod application;
mod domain;
mod ports;

use adapters::db::{
    MemoryNarrativeRepository, MemoryScreenplayRepository, SqliteScreenplayRepository,
};
#[cfg(target_os = "macos")]
use adapters::llm::FmRsNarrativeEngine;
use adapters::llm::StubNarrativeEngine;
use adapters::tauri::{
    commit_narrative_input, export_fountain_document, get_active_screenplay,
    get_current_project, get_llm_status, get_narrative_snapshot, get_nudge, get_screenplays,
    get_time, import_fountain_document, open_project_document, parse_character, parse_event,
    preview_narrative_input, save_project_document_as, save_screenplay, AppState,
};
use std::sync::Arc;
use tauri::menu::{AboutMetadataBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager, RunEvent};

const MENU_OPEN_PROJECT: &str = "file.open-project";
const MENU_SAVE_PROJECT: &str = "file.save-project";
const MENU_SAVE_PROJECT_AS: &str = "file.save-project-as";
const MENU_IMPORT_FOUNTAIN: &str = "file.import-fountain";
const MENU_EXPORT_FOUNTAIN: &str = "file.export-fountain";
const MENU_RELOAD_PROJECT: &str = "file.reload-project";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .on_menu_event(|app, event| match event.id().0.as_str() {
            MENU_OPEN_PROJECT => {
                let _ = app.emit("menu-open-project", ());
            }
            MENU_SAVE_PROJECT => {
                let _ = app.emit("menu-save-project", ());
            }
            MENU_SAVE_PROJECT_AS => {
                let _ = app.emit("menu-save-project-as", ());
            }
            MENU_IMPORT_FOUNTAIN => {
                let _ = app.emit("menu-import-fountain", ());
            }
            MENU_EXPORT_FOUNTAIN => {
                let _ = app.emit("menu-export-fountain", ());
            }
            MENU_RELOAD_PROJECT => {
                let _ = app.emit("menu-reload-project", ());
            }
            _ => {}
        })
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

            let (llm_backend, llm_detail) = narrative_backend_status();

            app.manage(AppState::new(
                screenplay_repository,
                narrative_repository,
                sqlite_repository,
                llm_backend,
                llm_detail,
                narrative_character_parser(),
                narrative_event_parser(),
                narrative_nudge_generator(),
            ));

            if let Some(project_path) = startup_project_path() {
                let state = app.state::<AppState>();
                let _ = state.switch_project_path(&project_path);
            }

            let menu = build_app_menu(app)?;
            let _ = app.set_menu(menu);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_time,
            get_llm_status,
            get_active_screenplay,
            get_current_project,
            get_narrative_snapshot,
            get_screenplays,
            save_screenplay,
            import_fountain_document,
            export_fountain_document,
            open_project_document,
            save_project_document_as,
            preview_narrative_input,
            commit_narrative_input,
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

fn build_app_menu<R: tauri::Runtime>(app: &tauri::App<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    let about = AboutMetadataBuilder::new()
        .name(Some("Lekhani".to_string()))
        .version(Some("0.1.0".to_string()))
        .authors(Some(vec!["Sushant Srivastava".to_string()]))
        .build();

    let app_menu = SubmenuBuilder::new(app, "Lekhani")
        .about(Some(about))
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(
            &MenuItemBuilder::with_id(MENU_OPEN_PROJECT, "Open…")
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id(MENU_SAVE_PROJECT, "Save")
                .accelerator("CmdOrCtrl+S")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id(MENU_SAVE_PROJECT_AS, "Save As…")
                .accelerator("CmdOrCtrl+Shift+S")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id(MENU_IMPORT_FOUNTAIN, "Import Fountain…")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id(MENU_EXPORT_FOUNTAIN, "Export Fountain…")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id(MENU_RELOAD_PROJECT, "Reload Project")
                .accelerator("CmdOrCtrl+R")
                .build(app)?,
        )
        .close_window()
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let view_menu = SubmenuBuilder::new(app, "View")
        .fullscreen()
        .separator()
        .minimize()
        .maximize()
        .build()?;

    MenuBuilder::new(app)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .build()
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

fn narrative_character_parser() -> Box<dyn ports::CharacterParser> {
    #[cfg(target_os = "macos")]
    if let Ok(engine) = FmRsNarrativeEngine::new() {
        return Box::new(engine);
    }

    Box::<StubNarrativeEngine>::default()
}

fn narrative_event_parser() -> Box<dyn ports::EventParser> {
    #[cfg(target_os = "macos")]
    if let Ok(engine) = FmRsNarrativeEngine::new() {
        return Box::new(engine);
    }

    Box::<StubNarrativeEngine>::default()
}

fn narrative_nudge_generator() -> Box<dyn ports::NudgeGenerator> {
    #[cfg(target_os = "macos")]
    if let Ok(engine) = FmRsNarrativeEngine::new() {
        return Box::new(engine);
    }

    Box::<StubNarrativeEngine>::default()
}

fn narrative_backend_status() -> (String, String) {
    #[cfg(target_os = "macos")]
    if FmRsNarrativeEngine::new().is_ok() {
        return (
            "fm-rs".to_string(),
            "Apple Foundation Models is available for structured narrative hydration.".to_string(),
        );
    }

    (
        "fallback".to_string(),
        "Using the local heuristic hydrator because Foundation Models is unavailable.".to_string(),
    )
}
