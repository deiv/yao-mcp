use std::fmt;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub enum VaultError {
    /// IO error
    IoError(io::Error),

    /// Invalid path
    InvalidPath { reason: String },
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VaultError::IoError(err) => write!(f, "IO error: {}", err),
            VaultError::InvalidPath { reason } => write!(f, "Invalid path: {}", reason),
        }
    }
}

impl VaultError {
    pub fn io(err: io::Error) -> Self {
        VaultError::IoError(err)
    }

    pub fn invalid_path(reason: impl Into<String>) -> Self {
        VaultError::InvalidPath {
            reason: reason.into(),
        }
    }

    pub fn invalid_path_traversal(path: &Path) -> Self {
        VaultError::InvalidPath {
            reason: format!("Invalid Path: path traversal detected: {:?}", path),
        }
    }
}
