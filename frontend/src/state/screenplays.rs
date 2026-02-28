use crate::api::tauri;
use crate::models::screenplay::ScreenplayListItem;
use leptos::*;

pub fn create_screenplays_resource() -> Resource<(), Result<Vec<ScreenplayListItem>, String>> {
    create_resource(
        || (),
        |_| async move {
            let screenplays = tauri::get_screenplays().await?;
            Ok(screenplays.into_iter().map(Into::into).collect())
        },
    )
}
