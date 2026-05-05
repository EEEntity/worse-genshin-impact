mod constants;
mod det;
mod imgproc;
mod rec;

use anyhow::Context;
use crate::inference::{model::Model, session::OrtSession};
use det::{db_postprocess, det_preprocess, Box4, DbConfig};
use imgproc::{apply_vertical_padding, get_rotate_crop_image, map_boxes_to_original, resize_within_bounds};
use ndarray::Array3;
use opencv::{core::Mat, imgproc as cv_imgproc, prelude::*};
use rec::{CtcDecoder, rec_preprocess_one_batch, rec_sort_indices};
use constants::*;

/// 单条识别结果
#[derive(Debug, Clone)]
pub struct OcrResult {
    /// 文本
    pub text: String,
    /// 置信度[0,1]
    pub score: f32,
    /// 包围框，相对于传入图像区域左上角
    pub bbox: [u32; 4],
}

/// OCR错误
#[derive(Debug)]
pub enum OcrError {
    Init(String),
    Run(String),
}

impl std::fmt::Display for OcrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OcrError::Init(m) => write!(f, "[OcrError::Init] {m}"),
            OcrError::Run(m)  => write!(f, "[OcrError::Run] {m}"),
        }
    }
}

impl std::error::Error for OcrError {}

// 推理引擎
pub struct OcrEngine {
    det_session: OrtSession,
    rec_session: OrtSession,
    ctc: CtcDecoder,
    db_cfg: DbConfig,
    det_limit_side_len: i32,
}

impl OcrEngine {
    /// 从[`Model`]加载模型，初始化引擎
    pub fn new() -> Result<Self, OcrError> {
        let det_session = OrtSession::new(Model::PaddleOCRDetV4.model_path())
            .map_err(|e|OcrError::Init(format!("load det model: {e}")))?;
        let rec_session = OrtSession::new(Model::PaddleOCRRecV4.model_path())
            .map_err(|e|OcrError::Init(format!("load rec model: {e}")))?;
        let dict_path = Model::PaddleOCRRecV4
            .label_full_path()
            .ok_or_else(||OcrError::Init("PaddleOCRRecV4 missing label path".to_string()))?;
        let ctc = CtcDecoder::load(dict_path)
            .map_err(|e|OcrError::Init(format!("load dict: {e}")))?;
        Ok(Self { det_session, rec_session, ctc, db_cfg: DbConfig::default(), det_limit_side_len: DET_LIMIT_SIDE_LEN })
    }
    /// 对RGB字节切片执行OCR
    /// 
    /// - `rgb`: 行主序RGB像素数据，来自`Capture::get_region()`
    /// - `width`/`height`: 图像尺寸
    pub fn run(&mut self, rgb: &[u8], width: u32, height: u32) -> Result<Vec<OcrResult>, OcrError> {
        self.run_inner(rgb, width, height)
            .map_err(|e| OcrError::Run(e.to_string()))
    }
    fn run_inner(&mut self, rgb: &[u8], width: u32, height: u32) -> anyhow::Result<Vec<OcrResult>> {
        let ori_img = rgb_to_bgr_mat(rgb, width, height)?;
        let ori_h = ori_img.rows();
        let ori_w = ori_img.cols();
        // 尺寸限制
        let (img1, ratio_h, ratio_w) = resize_within_bounds(&ori_img, MIN_SIDE_LEN, MAX_SIDE_LEN)
            .context("Step1 resize_within_bounds")?;
        // 纵向padding
        let (img2, pad_top) = apply_vertical_padding(&img1, WIDTH_HEIGHT_RATIO, MIN_HEIGHT)
            .context("Step2 apply_vertical_padding")?;
        // 检测预处理
        let det_input = det_preprocess(&img2, self.det_limit_side_len, "min")
            .context("Step3 det_preprocess")?;
        let det_h = img2.rows();
        let det_w = img2.cols();
        // 检测推理
        let det_output = self.det_session.run(&det_input).context("Step4 det inference")?;
        let det_4d = det_output.into_dimensionality::<ndarray::Ix4>()
            .context("reshape det output to 4d")?;
        // DB后处理
        let boxes = db_postprocess(&det_4d, (det_h, det_w), &self.db_cfg)
            .context("Step5 db_postprocess")?;
        if boxes.is_empty() { return Ok(vec![]); }
        // 透视变换裁剪
        let crops: Vec<Mat> = boxes
            .iter()
            .map(|bx| {
                let pts = [
                    [bx[0][0] as f32, bx[0][1] as f32],
                    [bx[1][0] as f32, bx[1][1] as f32],
                    [bx[2][0] as f32, bx[2][1] as f32],
                    [bx[3][0] as f32, bx[3][1] as f32]
                ];
                get_rotate_crop_image(&img2, &pts)
            })
            .collect::<anyhow::Result<_>>()
            .context("Step6 crop")?;
        let (crops, boxes): (Vec<_>, Vec<_>) = crops.into_iter().zip(boxes).filter(|(c, _)| !c.empty()).unzip();
        if crops.is_empty() { return Ok(vec![]); }
        // 批次识别
        let n = crops.len();
        let sorted_indices = rec_sort_indices(&crops);
        let mut results_unordered: Vec<(String, f32)> = vec![("".to_string(), 0.0); n];
        for chunk in sorted_indices.chunks(REC_BATCH_NUM) {
            let batch_crops: Vec<&Mat> = chunk.iter().map(|&i| &crops[i]).collect();
            let rec_input = rec_preprocess_one_batch(&batch_crops)
                .context("Step7 rec_preprocess_one_batch")?;
            let rec_output = self.rec_session.run(&rec_input).context("Step8 rec inference")?;
            let shape = rec_output.shape().to_vec();
            let rec_3d: Array3<f32> = rec_output.into_shape_with_order((shape[0], shape[1], shape[2]))
                .context("reshape rec output to 3d")?;
            let decoded = self.ctc.decode_batch(&rec_3d);
            for (local_i, &orig_i) in chunk.iter().enumerate() {
                results_unordered[orig_i] = decoded[local_i].clone();
            }
        }
        // 坐标逆映射
        let mut boxes_mut = boxes;
        map_boxes_to_original(&mut boxes_mut, ratio_h, ratio_w, pad_top, ori_h, ori_w);
        // 过滤&转换为xywh输出
        let output = results_unordered
            .into_iter()
            .zip(boxes_mut)
            .filter(|((text, score), _)| *score >= TEXT_SCORE && !text.trim().is_empty())
            .map(|((text, score), bbox)| OcrResult { text, score, bbox: box4_to_xywh(bbox) })
            .collect();
        Ok(output)
    }
}

/// RGB -> BGR Mat
fn rgb_to_bgr_mat(rgb: &[u8], width: u32, height: u32) -> anyhow::Result<Mat> {
    let mat_rgb = unsafe {
        Mat::new_rows_cols_with_data_unsafe(
            height as i32, width as i32, opencv::core::CV_8UC3,
            rgb.as_ptr() as *mut std::ffi::c_void, opencv::core::Mat_AUTO_STEP,
        )
    }
    .context("create rgb mat failed")?;
    let mut bgr = Mat::default();
    cv_imgproc::cvt_color_def(&mat_rgb, &mut bgr, cv_imgproc::COLOR_RGB2BGR)
        .context("rgb to bgr failed")?;
    Ok(bgr)
}

/// 4顶点box -> xywh
fn box4_to_xywh(b: Box4) -> [u32; 4] {
    let x_min = b.iter().map(|p| p[0]).min().unwrap_or(0).max(0) as u32;
    let y_min = b.iter().map(|p| p[1]).min().unwrap_or(0).max(0) as u32;
    let x_max = b.iter().map(|p| p[0]).max().unwrap_or(0).max(0) as u32;
    let y_max = b.iter().map(|p| p[1]).max().unwrap_or(0).max(0) as u32;
    [x_min, y_min, x_max.saturating_sub(x_min), y_max.saturating_sub(y_min)]
}
