use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use uuid::Uuid;

struct AppState {
    screenplays: Mutex<HashMap<Uuid, Screenplay>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Screenplay {
    id: Uuid,
    title: String,
    fountain_text: String,
    #[serde(skip)]
    parsed: Option<()>,
    version: u64,
    changes: Vec<ScreenplayChange>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScreenplayChange {
    id: Uuid,
    timestamp: DateTime<Utc>,
    author: String,
    change_type: ChangeType,
    range_start: usize,
    range_end: usize,
    new_text: String,
    old_text: String,
    provenance: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum ChangeType {
    Insert,
    Delete,
    Replace,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let _ = app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                );
            }

            app.manage(AppState {
                screenplays: Mutex::new(HashMap::new()),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_time,
            get_screenplays,
            save_screenplay
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn get_time() -> String {
    format!("Current time: {}", chrono::Local::now().to_rfc2822())
}

#[tauri::command]
fn get_screenplays(state: tauri::State<'_, AppState>) -> Result<Vec<Screenplay>, String> {
    let screenplays = state
        .screenplays
        .lock()
        .map_err(|_| "screenplay store lock poisoned".to_string())?;

    Ok(screenplays.values().cloned().collect())
}

#[tauri::command]
fn save_screenplay(
    state: tauri::State<'_, AppState>,
    screenplay: Screenplay,
) -> Result<(), String> {
    let mut screenplays = state
        .screenplays
        .lock()
        .map_err(|_| "screenplay store lock poisoned".to_string())?;

    screenplays.insert(screenplay.id, screenplay);
    Ok(())
}
