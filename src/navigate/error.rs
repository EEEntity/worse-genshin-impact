//! 导航相关错误

use std::io;

#[derive(Debug)]
pub enum NavigateError {
    Io(io::Error),
    Json(serde_json::Error),
    Cache(String),
    Sift(String),
    Capture(String),
    Device(String),
    Timeout(String),
    Unsupported(String),
    Cv(String),
    Retry(String),
    TpPointNotActivate(String),
    Other(String),
}

impl std::fmt::Display for NavigateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "[NavigateError::Io] {e}"),
            Self::Json(e) => write!(f, "[NavigateError::Json] {e}"),
            Self::Cache(m) => write!(f, "[NavigateError::Cache] {m}"),
            Self::Sift(m) => write!(f, "[NavigateError::Sift] {m}"),
            Self::Capture(m) => write!(f, "[NavigateError::Capture] {m}"),
            Self::Device(m) => write!(f, "[NavigateError::Device] {m}"),
            Self::Timeout(m) => write!(f, "[NavigateError::Timeout] {m}"),
            Self::Unsupported(m) => write!(f, "[NavigateError::Unsupported] {m}"),
            Self::Cv(m) => write!(f, "[NavigateError::Cv] {m}"),
            Self::Retry(m) => write!(f, "[NavigateError::Retry] {m}"),
            Self::TpPointNotActivate(m) => write!(f, "[NavigateError::TpPointNotActivate] {m}"),
            Self::Other(m) => write!(f, "[NavigateError::Other] {m}"),
        }
    }
}

impl std::error::Error for NavigateError {}

impl From<io::Error> for NavigateError {
    fn from(e: io::Error) -> Self { Self::Io(e) }
}
impl From<serde_json::Error> for NavigateError {
    fn from(e: serde_json::Error) -> Self { Self::Json(e) }
}
impl From<opencv::Error> for NavigateError {
    fn from(e: opencv::Error) -> Self { Self::Cv(e.to_string()) }
}
