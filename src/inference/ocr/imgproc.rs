// 图像尺寸处理

use anyhow::{Context, Result};
use opencv::{
    core::{self, Mat, Point2f, Scalar, Size, BORDER_REPLICATE},
    imgproc::{self, INTER_LINEAR},
    prelude::*,
};

/// 限制图像范围
pub fn resize_within_bounds(img: &Mat, min_side: i32, max_side: i32) -> Result<(Mat, f64, f64)> {
    let rows = img.rows();
    let cols = img.cols();
    let mut ratio_h = 1.0f64;
    let mut ratio_w = 1.0f64;
    let mut new_h = rows;
    let mut new_w = cols;
    if rows.max(cols) > max_side {
        let ratio = if rows > cols { max_side as f64 / rows as f64 } else { max_side as f64 / cols as f64 };
        new_h = (((rows as f64 * ratio) / 32.0).round() as i32) * 32;
        new_w = (((cols as f64 * ratio) / 32.0).round() as i32) * 32;
        ratio_h = rows as f64 / new_h as f64;
        ratio_w = cols as f64 / new_w as f64;
    }
    if new_h.min(new_w) < min_side {
        let ratio = if new_h < new_w { min_side as f64 / new_h as f64 } else { min_side as f64 / new_w as f64 };
        new_h = (((new_h as f64 * ratio) / 32.0).round() as i32) * 32;
        new_w = (((new_w as f64 * ratio) / 32.0).round() as i32) * 32;
        ratio_h = rows as f64 / new_h as f64;
        ratio_w = cols as f64 / new_w as f64;
    }
    if new_h == rows && new_w == cols {
        return Ok((img.clone(), ratio_h, ratio_w));
    }
    let mut resized = Mat::default();
    imgproc::resize(img, &mut resized, Size::new(new_w, new_h), 0.0, 0.0, INTER_LINEAR)
        .context("resize_within_bounds failed")?;
    Ok((resized, ratio_h, ratio_w))
}

/// 纵向padding
pub fn apply_vertical_padding(img: &Mat, width_height_ratio: f64, min_height: i32) -> Result<(Mat, i32)> {
    let h = img.rows();
    let w = img.cols();
    let use_limit_ratio = width_height_ratio >= 0.0 && w as f64 / h as f64 > width_height_ratio;
    if h > min_height && !use_limit_ratio {
        return Ok((img.clone(), 0));
    }
    let new_h = ((w as f64 / width_height_ratio).max(min_height as f64) * 2.0) as i32;
    let pad_top = ((new_h - h).abs() / 2).max(0);
    let mut padded = Mat::default();
    core::copy_make_border(
        img, &mut padded, pad_top, pad_top, 0, 0,
        core::BORDER_CONSTANT, Scalar::all(0.0),
    )
    .context("copy_make_border failed")?;
    Ok((padded, pad_top))
}

/// 透视变换裁剪单个文字区域(左上-右上-右下-左下)
pub fn get_rotate_crop_image(img: &Mat, points: &[[f32; 2]; 4]) -> Result<Mat> {
    let p = points;
    let crop_w = dist(p[0], p[1]).max(dist(p[2], p[3])).round() as i32;
    let crop_h = dist(p[0], p[3]).max(dist(p[1], p[2])).round() as i32;
    if crop_w <= 0 || crop_h <= 0 {
        return Ok(Mat::default());
    }
    let src_arr = [
        Point2f::new(p[0][0], p[0][1]), Point2f::new(p[1][0], p[1][1]),
        Point2f::new(p[2][0], p[2][1]), Point2f::new(p[3][0], p[3][1]),
    ];
    let dst_arr = [
        Point2f::new(0.0, 0.0), Point2f::new(crop_w as f32, 0.0),
        Point2f::new(crop_w as f32, crop_h as f32), Point2f::new(0.0, crop_h as f32),
    ];
    let src_pts = opencv::core::Mat::from_slice(src_arr.as_slice()).context("src_pts")?;
    let dst_pts = opencv::core::Mat::from_slice(dst_arr.as_slice()).context("dst_pts")?;
    let m = imgproc::get_perspective_transform(&src_pts, &dst_pts, core::DECOMP_LU)
        .context("get_perspective_transform failed")?;
    let mut dst = Mat::default();
    imgproc::warp_perspective(img, &mut dst, &m, Size::new(crop_w, crop_h), INTER_LINEAR, BORDER_REPLICATE, Scalar::all(0.0))
        .context("warp_perspective failed")?;
    // 竖排文字(h/w >= 1.5)旋转90度
    let dh = dst.rows();
    let dw = dst.cols();
    if dw > 0 && dh as f64 / dw as f64 >= 1.5 {
        let mut rotated = Mat::default();
        core::transpose(&dst, &mut rotated).context("transpose failed")?;
        let mut flipped = Mat::default();
        core::flip(&rotated, &mut flipped, 1).context("flip failed")?;
        return Ok(flipped);
    }
    Ok(dst)
}

/// 逆映射检测框到原图
pub fn map_boxes_to_original(
    boxes: &mut [[[i32; 2]; 4]],
    ratio_h: f64, ratio_w: f64, pad_top: i32, ori_h: i32, ori_w: i32,
) {
    for bx in boxes.iter_mut() {
        for pt in bx.iter_mut() {
            pt[1] -= pad_top;
            pt[0] = ((pt[0] as f64 * ratio_w).round() as i32).clamp(0, ori_w);
            pt[1] = ((pt[1] as f64 * ratio_h).round() as i32).clamp(0, ori_h);
        }
    }
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f64 {
    let dx = (a[0] - b[0]) as f64;
    let dy = (a[1] - b[1]) as f64;
    (dx * dx + dy * dy).sqrt()
}
