//! 当前出战角色低血量检测

use opencv::core::{Mat, Vec3b};
use opencv::prelude::MatTraitConst;

use crate::navigate::error::NavigateError;

/// 低血量像素颜色
const LOW_HP_BGR: (u8, u8, u8) = (90, 90, 255);
/// 采样坐标(1920x1080下)
const SAMPLE_X_1080P: i32 = 808;
const SAMPLE_Y_1080P: i32 = 1010;

/// 检测出战角色是否低血量
pub fn current_avatar_low_hp(screen: &Mat) -> Result<bool, NavigateError> {
    let h = screen.rows();
    if h <= 0 {
        return Ok(false);
    }
    let scale = h as f64 / 1080.0;
    let x = (SAMPLE_X_1080P as f64 * scale) as i32;
    let y = (SAMPLE_Y_1080P as f64 * scale) as i32;
    if x < 0 || y < 0 || x >= screen.cols() || y >= h {
        return Ok(false);
    }
    let v: Vec3b = *screen
        .at_2d::<Vec3b>(y, x)
        .map_err(|e| NavigateError::Cv(e.to_string()))?;
    Ok((v[0], v[1], v[2]) == LOW_HP_BGR)
}
