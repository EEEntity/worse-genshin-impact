//! 角色技能就绪判定
//! 
use opencv::core::{Mat, Rect, Scalar};
use opencv::imgproc;
use opencv::prelude::MatTraitConst;

use crate::navigate::error::NavigateError;

/// 当前出战角色E是否就绪
pub fn read_e_cooldown_ready(screen: &Mat) -> bool {
    let w = screen.cols();
    let h = screen.rows();
    if w <= 0 || h <= 0 {
        return true;
    }
    let roi = Rect::new(w * 1688 / 1920, h * 988 / 1080, w * 22 / 1920, h * 12 / 1080);
    is_skill_icon_ready(screen, roi).unwrap_or(true)
}

/// 当前出战角色Q是否就绪
pub fn read_q_cooldown_ready(screen: &Mat) -> bool {
    let w = screen.cols();
    let h = screen.rows();
    if w <= 0 || h <= 0 {
        return true;
    }
    let roi = Rect::new(w * 1809 / 1920, h * 968 / 1080, w * 30 / 1920, h * 15 / 1080);
    is_skill_icon_ready(screen, roi).unwrap_or(true)
}

/// 阈值化 -> 连通域计数判断
fn is_skill_icon_ready(screen: &Mat, roi: Rect) -> Result<bool, NavigateError> {
    let screen_rect = Rect::new(0, 0, screen.cols(), screen.rows());
    let safe = intersect_rect(roi, screen_rect);
    if safe.width <= 0 || safe.height <= 0 {
        return Ok(true);
    }
    let crop = Mat::roi(screen, safe).map_err(|e| NavigateError::Cv(e.to_string()))?;
    let mut mask = Mat::default();
    opencv::core::in_range(
        &crop,
        &Scalar::new(255.0, 255.0, 255.0, 0.0),
        &Scalar::new(255.0, 255.0, 255.0, 0.0),
        &mut mask,
    )
    .map_err(|e|NavigateError::Cv(e.to_string()))?;
    let mut labels = Mat::default();
    let mut stats = Mat::default();
    let mut centroids = Mat::default();
    let n = imgproc::connected_components_with_stats(
        &mask,
        &mut labels,
        &mut stats,
        &mut centroids,
        8,
        opencv::core::CV_32S,
    )
    .map_err(|e|NavigateError::Cv(e.to_string()))?;
    Ok(n <= 2)
}

fn intersect_rect(a: Rect, b: Rect) -> Rect {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y2 = (a.y + a.height).min(b.y + b.height);
    if x2 <= x1 || y2 <= y1 {
        return Rect::new(0, 0, 0, 0);
    }
    Rect::new(x1, y1, x2 - x1, y2 - y1)
}
