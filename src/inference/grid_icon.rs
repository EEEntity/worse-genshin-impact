//! 图标ROI推断物品名称

use std::collections::HashMap;
use std::path::Path;
use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use ndarray::Array4;
use opencv::core::{AlgorithmHint, Mat, Size};
use opencv::imgproc::{COLOR_BGR2RGB, INTER_LINEAR, cvt_color, resize};
use opencv::prelude::*;

use super::session::OrtSession;

/// Infer只比较前64维
const PROTOTYPE_DIM: usize = 64;
/// L2距离
const DIST_THRESHOLD_SQ: f32 = 100.0;
/// 模型输入尺寸
const ICON_SIZE: i32 = 125;

pub struct GridIconPredictor {
    session: OrtSession,
    prototypes: HashMap<String, Vec<f32>>,
}

impl GridIconPredictor {
    /// 加载模型/csv原型表
    pub fn load(model_path: impl AsRef<Path>, csv_path: impl AsRef<Path>) -> Result<Self> {
        let session = OrtSession::new(model_path.as_ref())
            .with_context(||format!("load gridIcon model `{}`", model_path.as_ref().display()))?;
        let csv = std::fs::read_to_string(csv_path.as_ref())
            .with_context(||format!("read items.csv `{}`", csv_path.as_ref().display()))?;
        let mut prototypes = HashMap::new();
        for (i, line) in csv.lines().enumerate() {
            if i == 0 {
                continue; // 表头
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut iter = line.splitn(2, ',');
            let name = iter
                .next()
                .with_context(||format!("items.csv line {}: missing name", i+1))?
                .to_string();
            let b64 = iter
                .next()
                .with_context(||format!("items.csv line {}: missing vector for `{name}`", i+1))?;
            let bytes = B64.decode(b64.as_bytes())
                .with_context(||{format!("items.csv line {}: base64 decode failed for `{name}`", i+1)})?;
            if bytes.len() < PROTOTYPE_DIM * 4 {
                anyhow::bail!(
                    "items.csv line {}: prototype `{name}` too short ({} bytes < {})",
                    i + 1,
                    bytes.len(),
                    PROTOTYPE_DIM * 4
                );
            }
            let floats: Vec<f32> = bytes
                .chunks_exact(4)
                .take(PROTOTYPE_DIM)
                .map(|c|f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            prototypes.insert(name, floats);
        }
        log::info!(
            "GridIconPredictor: 加载 {} 条原型向量",
            prototypes.len()
        );
        Ok(Self {
            session,
            prototypes,
        })
    }
    /// 推理
    pub fn infer(&mut self, bgr: &Mat) -> Result<Option<String>> {
        // resize
        let mut resized = Mat::default();
        resize(
            bgr,
            &mut resized,
            Size::new(ICON_SIZE, ICON_SIZE),
            0.0,
            0.0,
            INTER_LINEAR,
        )
        .map_err(|e|anyhow::anyhow!("gridIcon resize: {e}"))?;
        // BGR -> RGB
        let mut rgb = Mat::default();
        cvt_color(&resized, &mut rgb, COLOR_BGR2RGB, 0, AlgorithmHint::ALGO_HINT_DEFAULT)
            .map_err(|e|anyhow::anyhow!("gridIcon cvt_color: {e}"))?;
        // HWC u8 -> CHW f32 /255
        // 尝试写快一些？
        let h = rgb.rows() as usize;
        let w = rgb.cols() as usize;
        let bytes = rgb
            .data_bytes()
            .map_err(|e|anyhow::anyhow!("gridIcon data_bytes: {e}"))?;
        let mut input = Array4::<f32>::zeros((1, 3, h, w));
        for y in 0..h {
            for x in 0..w {
                let p = (y * w + x) * 3;
                input[[0, 0, y, x]] = bytes[p] as f32 / 255.0;
                input[[0, 1, y, x]] = bytes[p + 1] as f32 / 255.0;
                input[[0, 2, y, x]] = bytes[p + 2] as f32 / 255.0;
            }
        }
        // ONNX infer
        let out = self.session.run(&input).context("gridIcon ORT run")?;
        let feat: Vec<f32> = out.iter().take(PROTOTYPE_DIM).cloned().collect();
        if feat.len() < PROTOTYPE_DIM {
            anyhow::bail!(
                "gridIcon feature output too small: {} < {PROTOTYPE_DIM}",
                feat.len()
            );
        }
        // L2最近邻
        let mut best: Option<(f32, &str)> = None;
        for (name, proto) in &self.prototypes {
            let d2: f32 = (0..PROTOTYPE_DIM)
                .map(|i| {
                    let d = proto[i] - feat[i];
                    d * d
                })
                .sum();
            if best.map_or(true, |(b, _)| d2 < b) {
                best = Some((d2, name.as_str()));
            }
        }
        Ok(best.and_then(|(d2, n) | {
            if d2 < DIST_THRESHOLD_SQ {
                Some(n.to_string())
            } else {
                log::debug!("gridIcon 距离 {d2:2} 超过阈值 {DIST_THRESHOLD_SQ}, 最近原型{n}");
                None
            }
        }))
    }
}
