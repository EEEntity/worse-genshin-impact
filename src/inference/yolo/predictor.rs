//! YOLOv8检测器
//! 
//! letterbox -> ORT forward -> decode

use anyhow::{Context, Result};
use opencv::core::Mat;
use std::collections::HashMap;
use std::path::Path;

use super::decode::{decode_yolov8, Detection};
use super::letterbox::letterbox_bgr;
use crate::inference::session::{OrtSession, SessionOptions};

/// YOLOv8推理器
pub struct YoloPredictor {
    session: OrtSession,
    pub labels: Vec<String>,
    pub input_size: i32,
    pub conf_thr: f32,
    pub iou_thr: f32,
}

impl YoloPredictor {
    /// 默认参数加载:
    /// CPU, fallback尺寸640, 置信0.25, iou 0.45
    pub fn load(model_path: impl AsRef<Path>) -> Result<Self> {
        let mut session = OrtSession::with_options(model_path.as_ref(), &SessionOptions::cpu())
            .with_context(||format!("load yolo model {}", model_path.as_ref().display()))?;
        let input_size = detect_input_size(&mut session).unwrap_or(640);
        let labels = read_labels_from_metadata(&mut session)
            .context("read class names from ONNX metadata")?;
        Ok(Self {
            session,
            labels,
            input_size,
            conf_thr: 0.25,
            iou_thr: 0.45
        })
    }
    pub fn load_with(
        model_path: impl AsRef<Path>,
        opts: &SessionOptions,
        input_size: i32,
        conf_thr: f32,
        iou_thr: f32,
    ) -> Result<Self> {
        let mut session = OrtSession::with_options(model_path.as_ref(), opts)
            .with_context(|| format!("load yolo model {}", model_path.as_ref().display()))?;
        let labels = read_labels_from_metadata(&mut session)
            .context("read class names from ONNX metadata")?;
        Ok(Self { 
            session,
            labels,
            input_size,
            conf_thr,
            iou_thr
        })
    }
    /// 检测BGR `Mat` -> `Vec<Detection>`
    pub fn detect(&mut self, img: &Mat) -> Result<Vec<Detection>> {
        let (input, lb) = letterbox_bgr(img, self.input_size).context("letterbox")?;
        let out = self.session.run(&input).context("yolo session.run")?;
        Ok(decode_yolov8(&out, &lb, &self.labels, self.conf_thr, self.iou_thr))
    }
}

fn detect_input_size(session: &mut OrtSession) -> Option<i32> {
    let raw = session.raw();
    let inp = raw.inputs().first()?;
    let shape = inp.dtype().tensor_shape()?;
    let dims: Vec<i64> = shape.iter().copied().collect();
    // 期望 [1, 3, H, W]
    if dims.len() == 4 && dims[2] > 0 && dims[2] == dims[3] {
        Some(dims[2] as i32)
    } else {
        None
    }
}

/// 读取类别名
pub(crate) fn read_labels_from_metadata(session: &mut OrtSession) -> Result<Vec<String>> {
    let raw = session.raw();
    let meta = raw.metadata().context("session.metadata()")?;
    let custom = meta.custom_keys().unwrap_or_default();

    let mut names_str: Option<String> = None;
    for key in &custom {
        if key == "names" {
            names_str = meta.custom(key);
            break;
        }
    }
    let s = match names_str {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };
    Ok(parse_ultralytics_names(&s))
}

pub(crate) fn parse_ultralytics_names(s: &str) -> Vec<String> {
    let mut map: HashMap<usize, String> = HashMap::new();
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        // next number
        while i < bytes.len() && !bytes[i].is_ascii_digit() { i += 1; }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() { i += 1; }
        if start == i { break; }
        let idx: usize = match s[start..i].parse() { Ok(n) => n, Err(_) => continue };
        // 跳过尾部
        while i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'' || bytes[i].is_ascii_whitespace()) {
            i += 1;
        }
        // ":"
        if i >= bytes.len() || bytes[i] != b':' { continue; }
        i += 1;
        // 跳过空白
        while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
        // 值的开头
        if i >= bytes.len() { break; }
        let quote = bytes[i];
        if quote != b'\'' && quote != b'"' { continue; }
        i += 1;
        let val_start = i;
        while i < bytes.len() && bytes[i] != quote { i += 1; }
        if i >= bytes.len() { break; }
        let val = &s[val_start..i];
        i += 1;
        map.insert(idx, val.to_string());
    }
    if map.is_empty() { return Vec::new(); }
    let max = *map.keys().max().unwrap();
    (0..=max).map(|k| map.get(&k).cloned().unwrap_or_else(|| k.to_string())).collect()
}
