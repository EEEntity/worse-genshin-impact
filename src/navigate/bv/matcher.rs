//! 模板匹配辅助

use opencv::core::{Mat, MatTraitConst, Point, Rect};
use opencv::imgproc::{TM_CCOEFF_NORMED, match_template};

use crate::navigate::bv::assets::Template;
use crate::navigate::error::NavigateError;

/// 模板匹配单次结果
#[derive(Debug, Clone, Copy)]
pub struct MatchResult {
    /// 在原图坐标系中的左上角
    pub top_left: Point,
    pub width: i32,
    pub height: i32,
    pub score: f64,
}

impl MatchResult {
    pub fn rect(&self) -> Rect {
        Rect { x: self.top_left.x, y: self.top_left.y, width: self.width, height: self.height }
    }
    pub fn center(&self) -> Point {
        Point { x: self.top_left.x + self.width / 2, y: self.top_left.y + self.height / 2 }
    }
}

/// 进行模板匹配
pub fn find_template(
    screen: &Mat,
    template: &Template,
) -> Result<Option<MatchResult>, NavigateError> {
    // ROI不越界
    let screen_rect = Rect {
        x: 0,
        y: 0,
        width: screen.cols(),
        height: screen.rows(),
    };
    let roi = intersect(template.roi, screen_rect);
    if roi.width <= template.mat.cols() || roi.height <= template.mat.rows() {
        return Ok(None);
    }
    let region = Mat::roi(screen, roi)
        .map_err(|e|NavigateError::Cv(e.to_string()))?;
    let mut result = Mat::default();
    match_template(
        &region,
        &template.mat,
        &mut result,
        TM_CCOEFF_NORMED,
        &opencv::core::no_array(),
    )
    .map_err(|e|NavigateError::Cv(e.to_string()))?;
    let mut max_val = 0.0f64;
    let mut max_loc = Point::default();
    opencv::core::min_max_loc(
        &result,
        None,
        Some(&mut max_val),
        None,
        Some(&mut max_loc),
        &opencv::core::no_array(),
    )
    .map_err(|e|NavigateError::Cv(e.to_string()))?;
    if max_val < template.threshold {
        return Ok(None);
    }
    Ok(Some(MatchResult{
        top_left: Point{
            x: max_loc.x + roi.x,
            y: max_loc.x + roi.y,
        },
        width: template.mat.cols(),
        height: template.mat.rows(),
        score: max_val,
    }))
}

/// 只关心模板是否命中
pub fn matches(screen: &Mat, template: &Template) -> Result<bool, NavigateError> {
    Ok(find_template(screen, template)?.is_some())
}

/// 找到所有足够匹配的模板
pub fn find_template_all(
    screen: &Mat,
    template: &Template,
    max_results: usize,
) -> Result<Vec<MatchResult>, NavigateError> {
    // 移出去
    use opencv::core::{Scalar, no_array};
    use opencv::imgproc::{FILLED, rectangle};
    let screen_rect = Rect {
        x: 0,
        y: 0,
        width: screen.cols(),
        height: screen.rows(),
    };
    let roi = intersect(template.roi, screen_rect);
    if roi.width <= template.mat.cols() || roi.height <= template.mat.rows() {
        return Ok(Vec::new());
    }
    let region = Mat::roi(screen, roi)
        .map_err(|e|NavigateError::Cv(e.to_string()))?;
    let mut result = Mat::default();
    match_template(
        &region,
        &template.mat,
        &mut result,
        TM_CCOEFF_NORMED,
        &no_array()
    )
    .map_err(|e|NavigateError::Cv(e.to_string()))?;
    let mut hits = Vec::new();
    let tw = template.mat.cols();
    let th = template.mat.rows();
    for _ in 0..max_results {
        let mut max_val = 0.0f64;
        let mut max_loc = Point::default();
        opencv::core::min_max_loc(
            &result,
            None,
            Some(&mut max_val),
            None,
            Some(&mut max_loc),
            &no_array(),
        )
        .map_err(|e|NavigateError::Cv(e.to_string()))?;
        if max_val < template.threshold {
            break;
        }
        hits.push(MatchResult {
            top_left: Point {
                x: max_loc.x + roi.x,
                y: max_loc.y + roi.y,
            },
            width: tw,
            height: th,
            score: max_val
        });
        let suppress = Rect {
            x: (max_loc.x - tw / 2).max(0),
            y: (max_loc.y - th / 2).max(0),
            width: tw,
            height: th,
        };
        let _ = rectangle(
            &mut result,
            suppress,
            Scalar::all(-1.0),
            FILLED,
            opencv::imgproc::LINE_8,
            0
        );
    }
    Ok(hits)
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y2 = (a.y + a.height).min(b.y + b.height);
    if x2 <= x1 || y2 <= y1 {
        return Rect { x: 0, y: 0, width: 0, height: 0 };
    }
    Rect { x: x1, y: y1, width: x2 - x1, height: y2 - y1 }
}
