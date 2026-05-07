//! 角色朝向识别
//! 
//! 1. 从B[250,255]和R[0,10]按位与提取小地图指针边缘
//! 2. 找出最大轮廓，必须是3顶点
//! 3. 在ROI上转换HSV，阈值(93,155,170)-(255,255,255)提取指针
//! 4. 扫描三条边中线上连续黑边
//! 5. atan2(dy,dx)得到角度
//! 
//! 返回`Some(angle_deg)`，范围(-180, 180]；没三角形返回`None`

use opencv::core::{
    Mat, MatTraitConst, Point, Rect, Scalar, Vector,
    bitwise_and_def, in_range, split,
};
use opencv::imgproc::{
    CHAIN_APPROX_SIMPLE, COLOR_BGR2HSV, RETR_EXTERNAL, approx_poly_dp, arc_length, bounding_rect, cvt_color_def, find_contours_def
};

use crate::navigate::error::NavigateError;

fn midpoint(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) / 2, (a.y + b.y) / 2)
}

/// Bresenham 8连通线上像素逐点访问，返回连续0像素数
fn count_leading_black(mask: &Mat, mut a: Point, b: Point) -> Result<i32, NavigateError> {
    let dx = (b.x - a.x).abs();
    let dy = (b.y - a.y).abs();
    let sx = if a.x < b.x { 1 } else { -1 };
    let sy = if a.y < b.y { 1 } else { -1 };
    let mut err = dx - dy;
    let w = mask.cols();
    let h = mask.rows();
    let mut count = 0i32;
    loop {
        if a.x < 0 || a.y < 0 || a.x >= w || a.y >= h {
            break;
        }
        let v = *mask
            .at_2d::<u8>(a.y, a.x)
            .map_err(|e| NavigateError::Other(format!("at_2d: {e}")))?;
        if v == 255 {
            break;
        }
        count += 1;
        if a.x == b.x && a.y == b.y {
            break;
        }
        let e2 = err * 2;
        if e2 > -dy {
            err -= dy;
            a.x += sx;
        }
        if e2 < dx {
            err += dx;
            a.y += sy;
        }
    }
    Ok(count)
}

fn in_range_scalar(src: &Mat, lo: f64, hi: f64) -> Result<Mat, NavigateError> {
    let mut dst = Mat::default();
    in_range(src, &Scalar::all(lo), &Scalar::all(hi), &mut dst)
        .map_err(|e|NavigateError::Other(format!("inRange: {e}")))?;
    Ok(dst)
}

/// 用212x212小地图(BGR)计算
pub fn compute_character_angle(mini_map: &Mat) -> Result<Option<f32>, NavigateError> {
    if mini_map.channels() != 3 {
        return Err(NavigateError::Other(
            "compute_character_angle: 通道数错误".into(),
        ));
    }
    // 分离通道
    let mut channels: Vector<Mat> = Vector::new();
    split(mini_map, &mut channels)
        .map_err(|e|NavigateError::Other(format!("split: {e}")))?;
    if channels.len() < 3 {
        return Ok(None);
    }
    let blue_mask = in_range_scalar(&channels.get(0)?, 250.0, 255.0)?;
    let red_mask = in_range_scalar(&channels.get(2)?, 0.0, 10.0)?;
    let mut and_mat = Mat::default();
    bitwise_and_def(&blue_mask, &red_mask, &mut and_mat)
        .map_err(|e|NavigateError::Other(format!("bitwise_and: {e}")))?;
    // 最大轮廓
    let mut contours: Vector<Vector<Point>> = Vector::new();
    find_contours_def(&and_mat, &mut contours, RETR_EXTERNAL, CHAIN_APPROX_SIMPLE)
        .map_err(|e|NavigateError::Other(format!("findContours: {e}")))?;
    if contours.is_empty() {
        return Ok(None);
    }
    let mut max_idx = 0usize;
    let mut max_rect = Rect::default();
    for i in 0..contours.len() {
        let r = bounding_rect(&contours.get(i)?)
            .map_err(|e|NavigateError::Other(format!("boundingRect: {e}")))?;
        if r.width * r.height > max_rect.width * max_rect.height {
            max_rect = r;
            max_idx = i;
        }
    }
    let max_contour = contours.get(max_idx)?;
    let perim = arc_length(&max_contour, true)
        .map_err(|e|NavigateError::Other(format!("arcLength: {e}")))?;
    let mut approx: Vector<Point> = Vector::new();
    approx_poly_dp(&max_contour, &mut approx, 0.08 * perim, true)
        .map_err(|e|NavigateError::Other(format!("approxPolyDP: {e}")))?;
    if approx.len() != 3 {
        return Ok(None)
    }
    // ROI HSV阈值
    let roi = Mat::roi(mini_map, max_rect)
        .map_err(|e|NavigateError::Other(format!("Mat::roi: {e}")))?;
    let mut hsv = Mat::default();
    cvt_color_def(&roi, &mut hsv, COLOR_BGR2HSV)
        .map_err(|e|NavigateError::Other(format!("cvtColor: {e}")))?;
    let mut hsv_thr = Mat::default();
    in_range(
        &hsv,
        &Scalar::new(93.0, 155.0, 170.0, 0.0),
        &Scalar::new(255.0, 255.0, 255.0, 0.0),
        &mut hsv_thr,
    )
    .map_err(|e|NavigateError::Other(format!("inRange hsv: {e}")))?;
    // 三边只第一条black > max即返回方向
    let offset = Point::new(max_rect.x, max_rect.y);
    let max_black = 0i32;
    for i in 0..3 {
        let a = approx.get(i)?;
        let b = approx.get((i+1)%3)?;
        let c = approx.get((i+2)%3)?;
        let mid = midpoint(a, b);
        let target = c;
        let p_mid = Point::new(mid.x - offset.x, mid.y - offset.y);
        let p_tar = Point::new(target.x - offset.x, target.y - offset.y);
        let black = count_leading_black(&hsv_thr, p_mid, p_tar)?;
        if black > max_black {
            let dy = (target.y - mid.y) as f32;
            let dx = (target.x - mid.x) as f32;
            return Ok(Some(dy.atan2(dx) * (180.0 / std::f32::consts::PI)));
        }
    }
    Ok(None)
}
