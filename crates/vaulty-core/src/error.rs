use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Auth(String),
    #[error("too many attempts; try again in {0} seconds")]
    Throttled(i64),
}

pub type Result<T> = std::result::Result<T, Error>;

// Tauri commands need the error to be serializable; send the message string to the UI.
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
