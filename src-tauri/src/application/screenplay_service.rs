use crate::domain::{AppError, Screenplay};
use crate::ports::ScreenplayRepository;
use chrono::Utc;
use uuid::Uuid;

pub struct ScreenplayService<R> {
    repo: R,
}

impl<R> ScreenplayService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: ScreenplayRepository> ScreenplayService<R> {
    pub fn get_active_screenplay(&self) -> Result<Screenplay, AppError> {
        if let Some(screenplay) = self.repo.list()?.into_iter().next() {
            return Ok(screenplay);
        }

        let screenplay = Screenplay {
            id: Uuid::new_v4(),
            title: "Untitled Screenplay".to_string(),
            fountain_text: String::new(),
            parsed: None,
            version: 1,
            changes: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.repo.save(screenplay.clone())?;
        Ok(screenplay)
    }

    pub fn list_screenplays(&self) -> Result<Vec<Screenplay>, AppError> {
        self.repo.list()
    }

    pub fn save_screenplay(&self, mut screenplay: Screenplay) -> Result<Screenplay, AppError> {
        screenplay.updated_at = Utc::now();
        self.repo.save(screenplay.clone())?;
        Ok(screenplay)
    }
}
