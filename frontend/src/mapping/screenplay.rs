use crate::api::dto::ScreenplayDto;
use crate::models::screenplay::ScreenplayListItem;

impl From<ScreenplayDto> for ScreenplayListItem {
    fn from(value: ScreenplayDto) -> Self {
        Self {
            id: value.id,
            title: value.title,
        }
    }
}
