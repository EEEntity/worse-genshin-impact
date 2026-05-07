mod enums;
mod waypoint;

pub use enums::{ActionCode, MoveMode, WaypointType};
pub use waypoint::{ExtParams, Misidentification, PathInfo,
PathingTask, Waypoint};

use std::path::Path;
use super::NavigateError;

/// 从文件加载一条路径任务
pub fn load_from_file(p: impl AsRef<Path>) -> Result<PathingTask, NavigateError> {
    let bytes = std::fs::read(p.as_ref())?;
    load_from_slice(&bytes)
}

/// 从字节切片解析，自动剥离UTF-8 BOM
pub fn load_from_slice(bytes: &[u8]) -> Result<PathingTask, NavigateError> {
    let bytes = strip_bom(bytes);
    Ok(serde_json::from_slice(bytes)?)
}

fn strip_bom(b: &[u8]) -> &[u8] {
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) { &b[3..] } else { b }
}
