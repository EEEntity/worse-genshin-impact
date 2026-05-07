//! 小地图视角识别(备用)
//! 
//! 1. GaussianBlur(3x3)去噪
//! 2. 极坐标展开
//! 3. ROI并逆时针旋转90度(径向->水平)
//! 4. Scharr一阶水平方向，每行求左/右波峰并计数
//! 5. 在pm2度内左移-90加权乘，再加权卷积
//! 6. 取最大值索引+45即角度

use opencv::core::{
    BORDER_DEFAULT, CV_32F, Mat, MatTraitConst, Point2f as CvP2f, ROTATE_90_COUNTERCLOCKWISE, Rect,
    Size, rotate,
};
use opencv::imgproc::{
    COLOR_BGR2GRAY, INTER_LINEAR, WARP_POLAR_LINEAR, gaussian_blur_def, scharr, warp_polar,
};

use crate::navigate::error::NavigateError;

pub fn compute_camera_angle_fallback(mini_map: &Mat) -> Result<f32, NavigateError> {
    // 转灰度
    let mut gray = Mat::default();
    if mini_map.channels() == 3 {
        opencv::imgproc::cvt_color_def(mini_map, &mut gray, COLOR_BGR2GRAY)
            .map_err(|e| NavigateError::Other(format!("cvtColor: {e}")))?;
    } else {
        gray = mini_map
            .try_clone() // 不能影响调用mat
            .map_err(|e| NavigateError::Other(format!("clone: {e}")))?;
    }
    let mut blurred = Mat::default();
    gaussian_blur_def(&gray, &mut blurred, Size::new(3, 3), 0.0)
        .map_err(|e| NavigateError::Other(format!("GaussianBlur: {e}")))?;
    // 极坐标展开
    let center = CvP2f::new(blurred.cols() as f32 / 2.0, blurred.rows() as f32 / 2.0);
    let mut polar = Mat::default();
    warp_polar(
        &blurred,
        &mut polar,
        Size::new(360, 360),
        center,
        360.0,
        INTER_LINEAR | WARP_POLAR_LINEAR,
    )
    .map_err(|e| NavigateError::Other(format!("warpPolar: {e}")))?;
    let polar_h = polar.rows();
    let roi = Mat::roi(&polar, Rect::new(10, 0, 70, polar_h))
        .map_err(|e| NavigateError::Other(format!("Mat::roi polar: {e}")))?;
    let mut polar_roi = Mat::default();
    rotate(&roi, &mut polar_roi, ROTATE_90_COUNTERCLOCKWISE)
        .map_err(|e| NavigateError::Other(format!("rotate: {e}")))?;
    // Scharr 水平方向
    let mut scharr_f32 = Mat::default();
    scharr(
        &polar_roi,
        &mut scharr_f32,
        CV_32F,
        1,
        0,
        1.0,
        0.0,
        BORDER_DEFAULT,
    )
    .map_err(|e| NavigateError::Other(format!("Scharr: {e}")))?;
    let rows = scharr_f32.rows() as usize;
    let cols = scharr_f32.cols() as usize;
    if rows == 0 || cols == 0 {
        return Err(NavigateError::Other("scharr empty".into()));
    }
    let mut data = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        let row = scharr_f32
            .ptr(r as i32)
            .map_err(|e| NavigateError::Other(format!("ptr: {e}")))?;
        let slice = unsafe { std::slice::from_raw_parts(row as *const f32, cols) };
        data.extend_from_slice(slice);
    }
    let neg: Vec<f32> = data.iter().map(|x| -x).collect();
    let mut left = vec![0i32; 360];
    let mut right = vec![0i32; 360];
    for i in find_peaks(&data) {
        left[i % 360] += 1;
    }
    for i in find_peaks(&neg) {
        right[i % 360] += 1;
    }
    let left2: Vec<i32> = left.iter().zip(&right).map(|(&l, &r)| (l - r).max(0)).collect();
    let right2: Vec<i32> = right.iter().zip(&left).map(|(&r, &l)| (r - l).max(0)).collect();
    let mut sum = vec![0i32; 360];
    for i in -2..=2i32 {
        let shifted = shift(&right2, -90 + i);
        let weight = (3 - i.abs()) / 3;
        for j in 0..360 {
            sum[j] += left2[j] * shifted[j] * weight;
        }
    }
    let mut result = vec![0i32; 360];
    for i in -2..=2i32 {
        let shifted = shift(&sum, i);
        let weight = (3 - i.abs()) / 3;
        for j in 0..360 {
            result[j] += shifted[j] * weight;
        }
    }
    let (max_idx, _) = result.iter().enumerate().max_by_key(|&(_, v)| *v).unwrap();
    let mut angle = max_idx as i32 + 45;
    if angle > 360 {
        angle -= 360;
    }
    Ok(angle as f32)
}

fn find_peaks(data: &[f32]) -> Vec<usize> {
    let mut out = Vec::new();
    if data.len() < 3 {
        return out;
    }
    for i in 1..(data.len() - 1) {
        if data[i] > data[i - 1] && data[i] > data[i + 1] {
            out.push(i);
        }
    }
    out
}

/// 平移
fn shift(arr: &[i32], k: i32) -> Vec<i32> {
    let n = arr.len() as i32;
    if n == 0 {
        return Vec::new();
    }
    let kk = ((k % n) + n) % n; // 等价右移[0, n)
    let split = (n - kk) as usize;
    let mut out = Vec::with_capacity(n as usize);
    out.extend_from_slice(&arr[split..]);
    out.extend_from_slice(&arr[..split]);
    out
}
