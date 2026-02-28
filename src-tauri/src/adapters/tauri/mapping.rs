use crate::adapters::tauri::dto::{ScreenplayChangeDto, ScreenplayDto};
use crate::domain::{Screenplay, ScreenplayChange};

impl From<ScreenplayChange> for ScreenplayChangeDto {
    fn from(value: ScreenplayChange) -> Self {
        Self {
            id: value.id,
            timestamp: value.timestamp,
            author: value.author,
            change_type: value.change_type,
            range_start: value.range_start,
            range_end: value.range_end,
            new_text: value.new_text,
            old_text: value.old_text,
            provenance: value.provenance,
        }
    }
}

impl From<Screenplay> for ScreenplayDto {
    fn from(value: Screenplay) -> Self {
        Self {
            id: value.id,
            title: value.title,
            fountain_text: value.fountain_text,
            version: value.version,
            changes: value.changes.into_iter().map(Into::into).collect(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<ScreenplayChangeDto> for ScreenplayChange {
    fn from(value: ScreenplayChangeDto) -> Self {
        Self {
            id: value.id,
            timestamp: value.timestamp,
            author: value.author,
            change_type: value.change_type,
            range_start: value.range_start,
            range_end: value.range_end,
            new_text: value.new_text,
            old_text: value.old_text,
            provenance: value.provenance,
        }
    }
}

impl From<ScreenplayDto> for Screenplay {
    fn from(value: ScreenplayDto) -> Self {
        Self {
            id: value.id,
            title: value.title,
            fountain_text: value.fountain_text,
            version: value.version,
            changes: value.changes.into_iter().map(Into::into).collect(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
