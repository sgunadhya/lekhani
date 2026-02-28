use crate::domain::{AppError, Screenplay};
use std::sync::Arc;

pub trait ScreenplayRepository: Send + Sync {
    fn list(&self) -> Result<Vec<Screenplay>, AppError>;
    fn save(&self, screenplay: Screenplay) -> Result<(), AppError>;
}

impl<T: ScreenplayRepository + ?Sized> ScreenplayRepository for Box<T> {
    fn list(&self) -> Result<Vec<Screenplay>, AppError> {
        (**self).list()
    }

    fn save(&self, screenplay: Screenplay) -> Result<(), AppError> {
        (**self).save(screenplay)
    }
}

impl<T: ScreenplayRepository + ?Sized> ScreenplayRepository for Arc<T> {
    fn list(&self) -> Result<Vec<Screenplay>, AppError> {
        (**self).list()
    }

    fn save(&self, screenplay: Screenplay) -> Result<(), AppError> {
        (**self).save(screenplay)
    }
}
