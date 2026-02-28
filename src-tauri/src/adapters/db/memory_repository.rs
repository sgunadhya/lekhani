use std::collections::HashMap;
use std::sync::Mutex;

use crate::domain::{AppError, Screenplay};
use crate::ports::ScreenplayRepository;
use uuid::Uuid;

#[derive(Default)]
pub struct MemoryScreenplayRepository {
    screenplays: Mutex<HashMap<Uuid, Screenplay>>,
}

impl ScreenplayRepository for MemoryScreenplayRepository {
    fn list(&self) -> Result<Vec<Screenplay>, AppError> {
        let screenplays = self
            .screenplays
            .lock()
            .map_err(|_| AppError::StatePoisoned("screenplay store lock poisoned"))?;

        Ok(screenplays.values().cloned().collect())
    }

    fn save(&self, screenplay: Screenplay) -> Result<(), AppError> {
        let mut screenplays = self
            .screenplays
            .lock()
            .map_err(|_| AppError::StatePoisoned("screenplay store lock poisoned"))?;

        screenplays.insert(screenplay.id, screenplay);
        Ok(())
    }
}
