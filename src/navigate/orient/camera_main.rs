//! 小地图视角识别
//! 
//! 1. 中心156x156作为ROI
//! 2. 区分标点
//! 3. 极坐标remap
//! 4. 对BGR->HLS_FULL取H，->GRAY取亮度
//! 5. 遍历(theta,r)，得到角度

use std::sync::Mutex;

use opencv::core::{
    BORDER_CONSTANT, CMP_EQ, CV_8UC1, CV_32F, CV_32FC1, CV_32FC3,
    Mat, MatExprTraitConst, MatTrait, MatTraitConst, MatTraitConstManual, Point, Rect, Scalar,
    Size, Vector, add_def, bitwise_and_def, cart_to_polar, compare, copy_to, divide,
    extract_channel, hconcat, in_range, lut, min_max_loc, multiply, polar_to_cart, repeat,
    subtract,
};
use opencv::imgproc::{
    self as ip, COLOR_BGR2GRAY, COLOR_BGR2HLS_FULL, COLOR_GRAY2BGR, INTER_CUBIC, INTER_LINEAR,
    INTER_NEAREST, MORPH_CLOSE, MORPH_ELLIPSE, calc_hist, circle, cvt_color_def, dilate_def,
    get_structuring_element, integral_def, morphology_ex_def, remap, resize,
};

use crate::navigate::error::NavigateError;

// 常量
const SIZE: i32 = 156;
const TPL_OUT_RAD: f32 = 78.0;
const TPL_INN_RAD: f32 = 19.0;
const R_LENGTH: i32 = 60;
const THETA_LENGTH: i32 = 360;
const F_LENGTH: i32 = 256;
const SCALE: i32 = 2;
const PEAK_WIDTH: i32 = THETA_LENGTH / 4 * SCALE + 1; // = 181

const ALPHA_PARAMS_1: &[f32] = &[
    18.632, 20.157, 24.093, 34.617, 38.566, 41.94, 47.654, 51.087, 58.561, 63.925, 67.759, 71.77,
    75.214,
];

fn linear_spaced(a: f32, b: f32, n: i32, endpoint: bool) -> Vec<f32> {
    let denom = if endpoint { (n - 1) as f32 } else { n as f32 };
    (0..n).map(|i| a + (b - a) * i as f32 / denom).collect()
}

/// 输出(input-bkg)*255/mask + bkg
/// 输入应该是CV_32F
fn apply_mask(input: &mut Mat, mask: &Mat, bkg: f64) -> Result<(), NavigateError> {
    let bkg_scalar = if input.channels() == 3 {
        Scalar::new(bkg, bkg, bkg, 0.0)
    } else {
        Scalar::all(bkg)
    };
    subtract(
        &input.clone(),
        &bkg_scalar,
        input,
        &Mat::default(),
        -1,
    )?;
    // Cv2.Divide
    let mut inv = Mat::default();
    divide(255.0, mask, &mut inv, CV_32F)?; // inv = 255 / mask
    let prod = input.clone();
    multiply(&prod, &inv, input, 1.0, CV_32F)?;
    add_def(&input.clone(), &bkg_scalar, input)?;
    Ok(())
}

/// BGR -> HUE
fn bgr_to_hue(bgr: &Mat, h_img: &mut Mat, fa_img: &mut Mat) -> Result<(), NavigateError> {
    let mut hls = Mat::default();
    cvt_color_def(bgr, &mut hls, COLOR_BGR2HLS_FULL)?;
    let mut h = Mat::default();
    extract_channel(&hls, &mut h, 0)?;
    h.convert_to(h_img, CV_32FC1, 1.0, 0.0)?;
    let mut gray = Mat::default();
    cvt_color_def(bgr, &mut gray, COLOR_BGR2GRAY)?;
    gray.convert_to(fa_img, CV_32FC1, 1.0, 0.0)?;
    Ok(())
}

/// 向右移动k列
fn right_shift_cv(input: &Mat, output: &mut Mat, k: i32) -> Result<(), NavigateError> {
    let cols = input.cols();
    let rows = input.rows();
    let part1 = Mat::roi(input, Rect::new(cols - k, 0, k, rows))?;
    let part2 = Mat::roi(input, Rect::new(0, 0, cols - k, rows))?;
    let mut srcs: Vector<Mat> = Vector::new();
    srcs.push(part1.clone_pointee());
    srcs.push(part2.clone_pointee());
    hconcat(&srcs, output)?;
    Ok(())
}

struct MaskCalculator {
    // alpha_mask1: Mat,
    // alpha_mask2: Mat,
    // radius: Mat,
    // angle: Mat,
    // circle_mask: Mat,
    /// 形态学 5×5 椭圆核
    kernel: Mat,
}

impl MaskCalculator {
    fn new() -> Result<Self, NavigateError> {
        let x_array = linear_spaced(-(SIZE as f32) / 2.0, (SIZE as f32) / 2.0, SIZE, false);
        let x_row = unsafe {
            Mat::new_rows_cols_with_data_unsafe_def(
                1,
                SIZE,
                CV_32FC1,
                x_array.as_ptr() as *mut std::ffi::c_void,
            )?
        };
        let x_mat = repeat(&x_row, SIZE, 1)?;
        let mut y_mat = Mat::default();
        opencv::core::transpose(&x_mat, &mut y_mat)?;
        let mut radius_f = Mat::default();
        let mut angle_f = Mat::default();
        cart_to_polar(&x_mat, &y_mat, &mut radius_f, &mut angle_f, true)?;
        // 截断到0..255
        let mut radius = Mat::default();
        radius_f.convert_to(&mut radius, CV_8UC1, 1.0, 0.0)?;
        // angle
        let mut angle_half = Mat::default();
        divide(0.5, &Mat::default(), &mut angle_half, CV_32F).ok(); // actual below
        multiply(&angle_f, &Scalar::all(0.5), &mut angle_half, 1.0, CV_32F)?;
        let mut angle = Mat::default();
        angle_half.convert_to(&mut angle, CV_8UC1, 1.0, 0.0)?;
        let lut1_data: Vec<u8> = (0..256u32)
            .map(|v| {
                let idx = ALPHA_PARAMS_1
                    .partition_point(|&x| x < v as f32);
                ((229 + idx) as u32).min(255) as u8
            })
            .collect();
        let lut2_data: Vec<u8> = (0..256u32)
            .map(|v| (137.0 + 1.43 * v as f32).min(255.0) as u8)
            .collect();
        let lut1 = unsafe {
            Mat::new_rows_cols_with_data_unsafe_def(
                1,
                256,
                CV_8UC1,
                lut1_data.as_ptr() as *mut std::ffi::c_void,
            )?
        };
        let lut2 = unsafe {
            Mat::new_rows_cols_with_data_unsafe_def(
                1,
                256,
                CV_8UC1,
                lut2_data.as_ptr() as *mut std::ffi::c_void,
            )?
        };
        let mut alpha_mask1_g = Mat::default();
        lut(&radius, &lut1, &mut alpha_mask1_g)?;
        let mut alpha_mask2_g = Mat::default();
        lut(&radius, &lut2, &mut alpha_mask2_g)?;
        let mut alpha_mask1 = Mat::default();
        cvt_color_def(&alpha_mask1_g, &mut alpha_mask1, COLOR_GRAY2BGR)?;
        let mut alpha_mask2 = Mat::default();
        cvt_color_def(&alpha_mask2_g, &mut alpha_mask2, COLOR_GRAY2BGR)?;
        let mut circle_mask = Mat::zeros(SIZE, SIZE, CV_8UC1)?.to_mat()?;        circle(
            &mut circle_mask,
            Point::new(SIZE / 2, SIZE / 2),
            SIZE / 2,
            Scalar::all(255.0),
            -1,
            8,
            0,
        )?;
        let kernel = get_structuring_element(MORPH_ELLIPSE, Size::new(5, 5), Point::new(-1, -1))?;
        Ok(Self {
            // alpha_mask1,
            // alpha_mask2,
            // radius,
            // angle,
            // circle_mask,
            kernel,
        })
    }
    /// ROI
    fn process1(&self, full_minimap: &Mat) -> Result<(Mat, Mat), NavigateError> {
        if full_minimap.channels() != 3 {
            return Err(NavigateError::Other(
                "MaskCalculator.process1: needs BGR".into(),
            ));
        }
        let w = full_minimap.cols();
        let off = (w - SIZE) / 2;
        let cropped = Mat::roi(full_minimap, Rect::new(off, off, SIZE, SIZE))?
            .clone_pointee();
        let mask = self.create_icon_mask(&cropped)?;
        Ok((cropped, mask))
    }
    fn create_icon_mask(&self, bgr: &Mat) -> Result<Mat, NavigateError> {
        let mut chs: Vector<Mat> = Vector::new();
        opencv::core::split(bgr, &mut chs)?;
        let mut cmax = Mat::default();
        opencv::core::max(&chs.get(2)?, &chs.get(1)?, &mut cmax)?;
        let cmax_tmp = cmax.clone();
        opencv::core::max(&cmax_tmp, &chs.get(0)?, &mut cmax)?;
        let mut cmin = Mat::default();
        opencv::core::min(&chs.get(2)?, &chs.get(1)?, &mut cmin)?;
        let cmin_tmp = cmin.clone();
        opencv::core::min(&cmin_tmp, &chs.get(0)?, &mut cmin)?;
        let mut out_mask = Mat::default();
        compare(&cmax, &cmin, &mut out_mask, CMP_EQ)?;
        let mut diff = Mat::default();
        in_range(&cmax, &Scalar::all(50.0), &Scalar::all(127.0), &mut diff)?;
        let prev = out_mask.clone();
        bitwise_and_def(&diff, &prev, &mut out_mask)?;
        subtract(&cmax, &cmin, &mut diff, &Mat::default(), -1)?;
        let mut cmin_inv = Mat::default();
        subtract(&Scalar::all(255.0), &cmax, &mut cmin_inv, &Mat::default(), -1)?;
        let cmin_inv_tmp = cmin_inv.clone();
        multiply(
            &cmin_inv_tmp,
            &Scalar::all(1.0 / 6.0),
            &mut cmin_inv,
            1.0,
            -1,
        )?;
        let diff_tmp = diff.clone();
        opencv::core::min(&cmin_inv, &diff_tmp, &mut diff)?;
        let diff_tmp = diff.clone();
        add_def(&diff_tmp, &Scalar::all(10.0), &mut diff)?;
        let mut cmax_mut = cmax.clone();
        cmax_mut.set_to(&Scalar::all(255.0), &out_mask)?;
        let mut cmax_f = Mat::default();
        cmax_mut.convert_to(&mut cmax_f, CV_32F, 1.0, 0.0)?;
        let mut diff_f = Mat::default();
        diff.convert_to(&mut diff_f, CV_32F, 1.0, 0.0)?;
        let mut inv = Mat::default();
        divide(10.0, &diff_f, &mut inv, CV_32F)?;
        let cmax_f_tmp = cmax_f.clone();
        multiply(&cmax_f_tmp, &inv, &mut cmax_f, 1.0, CV_32F)?;
        ip::threshold(&cmax_f, &mut out_mask, 200.0, 255.0, ip::THRESH_BINARY)?;
        let mut out_u8 = Mat::default();
        out_mask.convert_to(&mut out_u8, CV_8UC1, 1.0, 0.0)?;
        let dilated = out_u8.clone();
        dilate_def(&dilated, &mut out_u8, &self.kernel)?;
        let closed_in = out_u8.clone();
        morphology_ex_def(&closed_in, &mut out_u8, MORPH_CLOSE, &self.kernel)?;
        Ok(out_u8)
    }
}

struct CameraOrientationCalculator {
    /// (THETA_LENGTH, R_LENGTH) CV_32F, x重映射
    rotation_remap_x: Mat,
    rotation_remap_y: Mat,
    /// (THETA_LENGTH, R_LENGTH) CV_32F
    alpha_mask1_remap: Mat,
    alpha_mask2_remap: Mat,
}

impl CameraOrientationCalculator {
    fn new() -> Result<Self, NavigateError> {
        let r_array = linear_spaced(TPL_INN_RAD, TPL_OUT_RAD, R_LENGTH, true);
        let theta_array = linear_spaced(0.0, 360.0, THETA_LENGTH, false);
        let r_row = unsafe {
            Mat::new_rows_cols_with_data_unsafe_def(
                1,
                R_LENGTH,
                CV_32FC1,
                r_array.as_ptr() as *mut std::ffi::c_void,
            )?
        };
        let theta_col = unsafe {
            Mat::new_rows_cols_with_data_unsafe_def(
                THETA_LENGTH,
                1,
                CV_32FC1,
                theta_array.as_ptr() as *mut std::ffi::c_void,
            )?
        };
        let r_mat = repeat(&r_row, THETA_LENGTH, 1)?;
        let theta_mat = repeat(&theta_col, 1, R_LENGTH)?;
        let mut x = Mat::default();
        let mut y = Mat::default();
        polar_to_cart(&r_mat, &theta_mat, &mut x, &mut y, true)?;
        let mut rotation_remap_x = Mat::default();
        let mut rotation_remap_y = Mat::default();
        add_def(&x, &Scalar::all((SIZE / 2) as f64), &mut rotation_remap_x)?;
        add_def(&y, &Scalar::all((SIZE / 2) as f64), &mut rotation_remap_y)?;
        let row2: Vec<f32> = r_array.iter().map(|&v| 137.0 + 1.43 * v).collect();
        let row2_mat = unsafe {
            Mat::new_rows_cols_with_data_unsafe_def(
                1,
                R_LENGTH,
                CV_32FC1,
                row2.as_ptr() as *mut std::ffi::c_void,
            )?
        };
        let alpha_mask2_remap = repeat(&row2_mat, THETA_LENGTH, 1)?;
        let row1: Vec<f32> = r_array
            .iter()
            .map(|&v| {
                let idx = ALPHA_PARAMS_1.partition_point(|&x| x < v);
                229.0 + idx as f32
            })
            .collect();
        let row1_mat = unsafe {
            Mat::new_rows_cols_with_data_unsafe_def(
                1,
                R_LENGTH,
                CV_32FC1,
                row1.as_ptr() as *mut std::ffi::c_void,
            )?
        };
        let alpha_mask1_remap = repeat(&row1_mat, THETA_LENGTH, 1)?;
        Ok(Self {
            rotation_remap_x,
            rotation_remap_y,
            alpha_mask1_remap,
            alpha_mask2_remap,
        })
    }
    /// 得到角度/置信度
    fn predict_rotation(&self, src: &Mat, mask: &Mat) -> Result<(f32, f32), NavigateError> {
        let mut src_f = Mat::default();
        src.convert_to(&mut src_f, CV_32FC3, 1.0, 0.0)?;
        let mut remap_src = Mat::default();
        let mut remap_mask = Mat::default();
        remap(
            &src_f,
            &mut remap_src,
            &self.rotation_remap_x,
            &self.rotation_remap_y,
            INTER_LINEAR,
            BORDER_CONSTANT,
            Scalar::all(0.0),
        )?;
        remap(
            mask,
            &mut remap_mask,
            &self.rotation_remap_x,
            &self.rotation_remap_y,
            INTER_NEAREST,
            BORDER_CONSTANT,
            Scalar::all(0.0),
        )?;
        let mut h_img = Mat::default();
        let mut fa_img = Mat::default();
        bgr_to_hue(&remap_src, &mut h_img, &mut fa_img)?;
        let temp = fa_img.clone();
        apply_mask(&mut fa_img, &self.alpha_mask1_remap, 0.0)?;
        copy_to(&temp, &mut fa_img, &remap_mask)?;
        let mut temp_mut = temp.clone();
        apply_mask(&mut temp_mut, &self.alpha_mask2_remap, 255.0)?;
        let mut fb_img = temp_mut.clone();
        apply_mask(&mut fb_img, &self.alpha_mask1_remap, 0.0)?;
        copy_to(&temp_mut, &mut fb_img, &remap_mask)?;
        let mut hist_a = Mat::default();
        let mut hist_b = Mat::default();
        let channels = Vector::<i32>::from_slice(&[0, 1]);
        let hist_size = Vector::<i32>::from_slice(&[F_LENGTH, F_LENGTH]);
        let ranges = Vector::<f32>::from_slice(&[0.0, 256.0, 0.0, 256.0]);
        let mut imgs_a: Vector<Mat> = Vector::new();
        imgs_a.push(h_img.clone());
        imgs_a.push(fa_img.clone());
        calc_hist(&imgs_a, &channels, &Mat::default(), &mut hist_a, &hist_size, &ranges, false)?;
        let mut imgs_b: Vector<Mat> = Vector::new();
        imgs_b.push(h_img.clone());
        imgs_b.push(fb_img.clone());
        calc_hist(&imgs_b, &channels, &Mat::default(), &mut hist_b, &hist_size, &ranges, false)?;
        let total = (THETA_LENGTH * R_LENGTH) as usize;
        let h_data = h_img.data_typed::<f32>()?;
        let fa_data = fa_img.data_typed::<f32>()?;
        let fb_data = fb_img.data_typed::<f32>()?;
        let ha_data = hist_a.data_typed::<f32>()?;
        let hb_data = hist_b.data_typed::<f32>()?;
        let mut result = vec![0f32; THETA_LENGTH as usize];
        for i in 0..total {
            let h = h_data[i];
            let fa = fa_data[i];
            let fb = fb_data[i];
            if !(h >= 0.0 && h < 256.0) || fa < 0.0 || fb >= 256.0 {
                continue;
            }
            let theta_idx = i / R_LENGTH as usize;
            if fa >= 256.0 {
                result[theta_idx] += 255.0;
            } else if fb < 0.0 {
                result[theta_idx] += -255.0;
            } else {
                let h_bin = (h / 256.0 * F_LENGTH as f32) as usize;
                let fa_bin = (fa / 256.0 * F_LENGTH as f32) as usize;
                let fb_bin = (fb / 256.0 * F_LENGTH as f32) as usize;
                let ha = ha_data[h_bin * F_LENGTH as usize + fa_bin];
                let hb = hb_data[h_bin * F_LENGTH as usize + fb_bin];
                let v = if ha > hb {
                    0.0
                } else if (ha - hb).abs() < 1e-4 {
                    100.0
                } else {
                    255.0
                };
                result[theta_idx] += v;
            }
        }
        let result_mat = unsafe {
            Mat::new_rows_cols_with_data_unsafe_def(
                1,
                THETA_LENGTH,
                CV_32FC1,
                result.as_ptr() as *mut std::ffi::c_void,
            )?
        };
        let mut result_resized = Mat::default();
        resize(
            &result_mat,
            &mut result_resized,
            Size::new(THETA_LENGTH * SCALE, 1),
            0.0,
            0.0,
            INTER_CUBIC,
        )?;
        let mut result_shift = Mat::default();
        right_shift_cv(&result_resized, &mut result_shift, PEAK_WIDTH)?;
        // peakRegionSum = sum(resultShift[0:1, 0:PEAK_WIDTH])
        let peak_roi = Mat::roi(&result_shift, Rect::new(0, 0, PEAK_WIDTH, 1))?;
        let peak_sum = opencv::core::sum_elems(&peak_roi)?[0];
        let prev_shift = result_shift.clone();
        subtract(&result_resized, &prev_shift, &mut result_shift, &Mat::default(), -1)?;
        let mut integral_out = Mat::default();
        integral_def(&result_shift, &mut integral_out)?;
        let mut max_val = 0f64;
        let mut max_loc = Point::default();
        min_max_loc(
            &integral_out,
            None,
            Some(&mut max_val),
            None,
            Some(&mut max_loc),
            &Mat::default(),
        )?;
        let degree =
            (max_loc.x - 1) as f32 / THETA_LENGTH as f32 * 360.0 / SCALE as f32 - 45.0;
        let confidence =
            (max_val + peak_sum) as f32 / (PEAK_WIDTH * R_LENGTH * 255) as f32;
        Ok((degree, confidence.clamp(0.0, 1.0)))
    }
}

// 公开处理封装
pub struct MiniMapPreprocessor {
    mask_calc: MaskCalculator,
    co_calc: CameraOrientationCalculator,
}

impl MiniMapPreprocessor {
    pub fn new() -> Result<Self, NavigateError> {
        Ok(Self {
            mask_calc: MaskCalculator::new()?,
            co_calc: CameraOrientationCalculator::new()?,
        })
    }
    pub fn predict_rotation_with_confidence(
        &self,
        mini_map: &Mat,
    ) -> Result<(f32, f32), NavigateError> {
        let (src, mask) = self.mask_calc.process1(mini_map)?;
        self.co_calc.predict_rotation(&src, &mask)
    }
}

// 单实例
fn shared() -> &'static Mutex<MiniMapPreprocessor> {
    use std::sync::OnceLock;
    static INSTANCE: OnceLock<Mutex<MiniMapPreprocessor>> = OnceLock::new();
    INSTANCE.get_or_init(||Mutex::new(MiniMapPreprocessor::new().expect("MiniMapPreprocessor::new")))
}

/// 计算角度
/// 置信度<0.2时回退[`super::camera_fallback::compute_camera_angle_fallback`]
pub fn compute_camera_angle(mini_map: &Mat) -> Result<f32, NavigateError> {
    let (angle, conf) = shared()
        .lock()
        .map_err(|e|NavigateError::Other(format!("MiniMapPreprocessor mutex: {e}")))?
        .predict_rotation_with_confidence(mini_map)?;
    if conf < 0.2 {
        log::debug!("Camera orientation confidence low ({conf}), using fallback");
        return super::camera_fallback::compute_camera_angle_fallback(mini_map);
    }
    Ok(angle)
}

/// 直接暴露置信度
pub fn compute_camera_angle_with_confidence(mini_map: &Mat) -> Result<(f32, f32), NavigateError> {
    shared()
        .lock()
        .map_err(|e|NavigateError::Other(format!("MiniMapPreprocessor mutex: {e}")))?
        .predict_rotation_with_confidence(mini_map)
}
