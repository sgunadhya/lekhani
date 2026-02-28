use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum AppError {
    StatePoisoned(&'static str),
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StatePoisoned(message) => f.write_str(message),
        }
    }
}

impl Error for AppError {}
