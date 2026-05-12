//! 运动状态识别

use opencv::core::Mat;

pub use super::status::{MotionStatus, get_motion_status};
use crate::navigate::error::NavigateError;

/// 是否在飞行/滑翔
pub fn is_flying(screen: &Mat) -> Result<bool, NavigateError> {
    Ok(matches!(get_motion_status(screen)?, MotionStatus::Fly))
}

/// 是否在攀爬
pub fn is_climbing(screen: &Mat) -> Result<bool, NavigateError> {
    Ok(matches!(get_motion_status(screen)?, MotionStatus::Climb))
}

/// 是否在游泳
#[deprecated(note = "未实现is_swimming")]
fn warn_is_swimming_unimplemented() {}
pub fn is_swimming(_screen: &Mat) -> Result<bool, NavigateError> {
    warn_is_swimming_unimplemented();
    unimplemented!("未实现is_swimming")
}

/// 是否在使用风之翼(和飞行一致)
pub fn is_using_parachute(screen: &Mat) -> Result<bool, NavigateError> {
    is_flying(screen)
}
