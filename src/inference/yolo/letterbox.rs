//! YOLOv8前处理

use anyhow::{Context, Result};
use ndarray::Array4;
use opencv::{
    core::{Mat, Scalar, Size, CV_8UC3},
    imgproc,
    prelude::*,
};

/// letterbox变换记录
#[derive(Debug, Clone, Copy)]
pub struct Letterbox {
    /// 原图尺寸(w,h)
    pub orig_size: (i32, i32),
    /// 目标边长
    pub target: i32,
    /// 等比缩放系数
    pub scale: f32,
    /// 左padding
    pub pad_x: i32,
    /// 上padding
    pub pad_y: i32,
}

impl Letterbox {
    /// 反向映射
    pub fn unproject_xy(&self, x: f32, y: f32) -> (f32, f32) {
        ((x - self.pad_x as f32) / self.scale, (y - self.pad_y as f32) / self.scale)
    }
}

/// latterbox处理
pub fn letterbox_bgr(img: &Mat, target: i32) -> Result<(Array4<f32>, Letterbox)> {
    let orig_w = img.cols();
    let orig_h = img.rows();
    anyhow::ensure!(orig_w > 0 && orig_h > 0, "empty image");
    // 优化?
    let scale = (target as f32 / orig_w as f32).min(target as f32 / orig_h as f32);
    let new_w = (orig_w as f32 * scale).round() as i32;
    let new_h = (orig_h as f32 * scale).round() as i32;
    // resize
    let mut resized = Mat::default();
    imgproc::resize(
        img,
        &mut resized,
        Size::new(new_w, new_h),
        0.0,
        0.0,
        imgproc::INTER_LINEAR,
    )
    .context("letterbox resize")?;
    // 灰底
    let mut canvas = Mat::new_rows_cols_with_default(
        target,
        target,
        CV_8UC3,
        Scalar::new(114.0, 114.0, 114.0, 0.0)
    )
    .context("latter canvas alloc")?;
    let pad_x = (target - new_w) / 2;
    let pad_y = (target - new_h) / 2;
    // 放在canvas中心
    let roi_rect = opencv::core::Rect::new(pad_x, pad_y, new_w, new_h);
    let mut roi = Mat::roi_mut(&mut canvas, roi_rect).context("letterbox roi")?;
    resized.copy_to(&mut roi).context("letterbox copy_to roi")?;
    // BGR u8 -> RGB f32 [1,3,H,W] /255
    let mut tensor = Array4::<f32>::zeros((1, 3, target as usize, target as usize));
    for y in 0..target {
        for x in 0..target {
            let p = canvas
                .at_2d::<opencv::core::Vec3b>(y, x)
                .context("letterbox at_2d")?;
            tensor[[0, 0, y as usize, x as usize]] = p[2] as f32 / 255.0;
            tensor[[0, 1, y as usize, x as usize]] = p[1] as f32 / 255.0;
            tensor[[0, 2, y as usize, x as usize]] = p[0] as f32 / 255.0;
        }
    }
    Ok((
        tensor,
        Letterbox {
            orig_size: (orig_w, orig_h),
            target,
            scale,
            pad_x,
            pad_y,
        },
    ))
}
