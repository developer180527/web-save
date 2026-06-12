use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("save not found: id {0}")]
    NotFound(i64),
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("{0}")]
    Conflict(String),
}

pub type Result<T> = std::result::Result<T, Error>;
