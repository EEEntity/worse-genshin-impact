//! 钓鱼用图像识别

use opencv::core::{MatTraitConst, Point, Rect, Scalar, Size, Vector};
use opencv::imgproc::{
    self, COLOR_BGR2HSV_FULL, COLOR_BGR2RGB, ContourApproximationModes,
    MorphShapes, RetrievalModes, ThresholdTypes,
};
use opencv::prelude::*;

use crate::navigate::error::NavigateError;

/// 钓鱼条HSV矩形
pub fn get_fish_bar_rects(src: &Mat) -> Result<Vec<Rect>, NavigateError> {
    // BGR -> HSV
    let mut hsv = Mat::default();
    imgproc::cvt_color(src, &mut hsv, COLOR_BGR2HSV_FULL, 0, opencv::core::AlgorithmHint::ALGO_HINT_DEFAULT)
        .map_err(|e|NavigateError::Cv(e.to_string()))?;
    // H = 60度 / 360 * 255 = 42.5
    // S = 0.25 * 255 = 63.75
    // V = 1.00 * 255 = 255
    let h_center = 60.0 / 360.0 * 255.0;
    let s_center = 0.25 * 255.0;
    let v_center = 1.00 * 255.0;
    let low = Scalar::new(h_center - 3.0, s_center - 20.0, v_center - 10.0, 0.0);
    let high = Scalar::new(h_center + 3.5, s_center + 40.0, v_center, 0.0);
    let mut mask = Mat::default();
    opencv::core::in_range(&hsv, &low, &high, &mut mask)
        .map_err(|e|NavigateError::Cv(e.to_string()))?;
    // 二值化
    let mut bin = Mat::default();
    imgproc::threshold(&mask, &mut bin, 0.0, 255.0, ThresholdTypes::THRESH_BINARY as i32)
        .map_err(|e|NavigateError::Cv(e.to_string()))?;
    // 找外轮廓
    let mut contours: Vector<Vector<Point>> = Vector::new();
    imgproc::find_contours(
        &bin,
        &mut contours,
        RetrievalModes::RETR_EXTERNAL as i32,
        ContourApproximationModes::CHAIN_APPROX_SIMPLE as i32,
        Point::new(0, 0),
    )
    .map_err(|e|NavigateError::Cv(e.to_string()))?;
    if contours.is_empty() {
        return Ok(Vec::new());
    }
    // 剔除倾斜
    // 保留angle % 45 <= 1的轮廓
    let mut boxes: Vec<Rect> = Vec::new();
    for c in contours.iter() {
        let mar = imgproc::min_area_rect(&c).map_err(|e|NavigateError::Cv(e.to_string()))?;
        let angle = mar.angle.abs();
        let mod45 = angle - (angle / 45.0).floor() * 45.0;
        if mod45 > 1.0 {
            continue;
        }
        let r = imgproc::bounding_rect(&c).map_err(|e|NavigateError::Cv(e.to_string()))?;
        boxes.push(r);
    }
    if boxes.is_empty() {
        return Ok(Vec::new());
    }
    // 取最宽的当基准
    let widest = *boxes.iter().max_by_key(|b| b.width).unwrap();
    let widest_cy = widest.y + widest.height / 2;
    let h_tol = (widest.height / 5).max(1);
    let h_diff_cap = (widest.height / 3).max(1);
    let w_min = (widest.height / 4).max(1);
    let filtered: Vec<Rect> = boxes
        .into_iter()
        .filter(|r| {
            let cy = r.y + r.height / 2;
            (widest_cy - cy).abs() < h_tol
                && (widest.height - r.height).abs() < h_diff_cap
                && r.width > w_min
        })
        .collect();
    Ok(filtered)
}

/// 上钩文字识别
pub fn match_fish_bite_words(src: &Mat, lifting_words_area: Rect) -> Result<Option<Rect>, NavigateError> {
    // BGR -> RGB -> InRange纯白(253..=255)
    let mut rgb = Mat::default();
    imgproc::cvt_color(src, &mut rgb, COLOR_BGR2RGB, 0, opencv::core::AlgorithmHint::ALGO_HINT_DEFAULT)
        .map_err(|e|NavigateError::Cv(e.to_string()))?;
    let low = Scalar::new(253.0, 253.0, 253.0, 0.0);
    let high = Scalar::new(255.0, 255.0, 255.0, 0.0);
    let mut white = Mat::default();
    opencv::core::in_range(&rgb, &low, &high, &mut white)
        .map_err(|e|NavigateError::Cv(e.to_string()))?;
    let mut bin = Mat::default();
    imgproc::threshold(&white, &mut bin, 0.0, 255.0, ThresholdTypes::THRESH_BINARY as i32)
        .map_err(|e|NavigateError::Cv(e.to_string()))?;
    // 膨胀(让分散的字符块连通)
    let kernel = imgproc::get_structuring_element(
        MorphShapes::MORPH_RECT as i32,
        Size::new(20, 20),
        Point::new(-1, -1),
    )
    .map_err(|e|NavigateError::Cv(e.to_string()))?;
    let mut dilated = Mat::default();
    imgproc::dilate(
        &bin,
        &mut dilated,
        &kernel,
        Point::new(-1, -1),
        1,
        opencv::core::BORDER_CONSTANT,
        imgproc::morphology_default_border_value().map_err(|e|NavigateError::Cv(e.to_string()))?,
    )
    .map_err(|e|NavigateError::Cv(e.to_string()))?;
    let mut contours: Vector<Vector<Point>> = Vector::new();
    imgproc::find_contours(
        &dilated,
        &mut contours,
        RetrievalModes::RETR_EXTERNAL as i32,
        ContourApproximationModes::CHAIN_APPROX_SIMPLE as i32,
        Point::new(0, 0),
    )
    .map_err(|e|NavigateError::Cv(e.to_string()))?;
    if contours.is_empty() {
        return Ok(None);
    }
    let mut boxes: Vec<Rect> = contours
        .iter()
        .map(|c| imgproc::bounding_rect(&c).unwrap_or_default())
        .collect();
    // 多于1个时按高度降序，取第一个候选
    if boxes.len() > 1 {
        boxes.sort_by(|a, b| b.height.cmp(&a.height));
    }
    let r = boxes[0];
    let src_h = src.rows();
    let area_w = lifting_words_area.width;
    if r.height < src_h
        && r.height > 0
        && (r.width as f32) / (r.height as f32) >= 3.0
        && (area_w as f32) > (r.width as f32) * 3.0
        && area_w / 2 > r.x
        && area_w / 2 < r.x + r.width
    {
        Ok(Some(r))
    } else {
        Ok(None)
    }
}
