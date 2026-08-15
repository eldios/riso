use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ColorError {
    #[error("color is missing the leading '#': {0}")]
    MissingHash(String),
    #[error("color must have exactly 6 hex digits: {0}")]
    Length(String),
    #[error("color contains non-hexadecimal digits: {0}")]
    Digits(String),
}

#[derive(Debug, Error)]
pub enum IoError {
    #[error("path has no parent directory: {0}")]
    NoParent(PathBuf),
    #[error("failed to write {0}: {1}")]
    Write(PathBuf, #[source] std::io::Error),
    #[error("failed to read {0}: {1}")]
    Read(PathBuf, #[source] std::io::Error),
}
