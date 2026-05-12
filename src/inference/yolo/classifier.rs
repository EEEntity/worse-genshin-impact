//! 简易图像分类器

use anyhow::{Context, Result};
use ndarray::Array4;
use opencv::{
    core::{Mat, Size},
    imgproc,
    prelude::*,
};
use std::path::Path;

use crate::inference::session::{OrtSession, SessionOptions};

/// 单类结果
#[derive(Debug, Clone, PartialEq)]
pub struct Classification {
    pub class_id: usize,
    pub label: String,
    pub score: f32,
}

/// ONNX分类器
pub struct OrtClassifier {
    session: OrtSession,
    pub labels: Vec<String>,
    pub input_size: (i32, i32),
}

impl OrtClassifier {
    /// 加载分类器
    pub fn load(model_path: impl AsRef<Path>, labels: Vec<String>, input_size: (i32, i32)) -> Result<Self> {
        let session = OrtSession::with_options(model_path.as_ref(), &SessionOptions::cpu())
            .with_context(||format!("load classifier {}", model_path.as_ref().display()))?;
        Ok(Self { session, labels, input_size })
    }
    /// 自动从模型`[N,C,H,W]`读取输入尺寸
    pub fn load_auto(model_path: impl AsRef<Path>, labels: Vec<String>) -> Result<Self> {
        let mut session = OrtSession::with_options(model_path.as_ref(), &SessionOptions::cpu())
            .with_context(||format!("load classifier {}", model_path.as_ref().display()))?;
        let input_size = detect_classifier_input(&mut session)
            .context("classifier input shape must be fully specified [1,3,H,W]")?;
        Ok(Self { session, labels, input_size })
    }
    /// 分类
    pub fn classify(&mut self, roi: &Mat) -> Result<Classification> {
        let (w, h) = self.input_size;
        let mut resized = Mat::default();
        imgproc::resize(
            roi,
            &mut resized,
            Size::new(w, h),
            0.0,
            0.0,
            imgproc::INTER_LINEAR,
        )
        .context("classifier resize")?;
        let mut tensor = Array4::<f32>::zeros((1, 3, h as usize, w as usize));
        for y in 0..h {
            for x in 0..w {
                let p = resized.at_2d::<opencv::core::Vec3b>(y, x).context("at_2d")?;
                // BGR -> RGB
                tensor[[0, 0, y as usize, x as usize]] = p[2] as f32 / 255.0; // R
                tensor[[0, 1, y as usize, x as usize]] = p[1] as f32 / 255.0; // G
                tensor[[0, 2, y as usize, x as usize]] = p[0] as f32 / 255.0; // B
            }
        }
        let out = self.session.run(&tensor).context("classifier run")?;
        let view = out.view();
        let flat: Vec<f32> = view.iter().copied().collect();
        // softmax
        let max = flat.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = flat.iter().map(|x| (x - max).exp()).collect();
        let sum: f32 = exps.iter().sum();
        let probs: Vec<f32> = exps.iter().map(|x| x / sum).collect();
        let (best_idx, &best_p) = probs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .context("empty classifier output")?;
        let label = self.labels.get(best_idx).cloned().unwrap_or_else(|| best_idx.to_string());
        Ok(Classification { class_id: best_idx, label, score: best_p })
    }
}

fn detect_classifier_input(session: &mut OrtSession) -> Option<(i32, i32)> {
    let raw = session.raw();
    let inp = raw.inputs().first()?;
    let shape = inp.dtype().tensor_shape()?;
    let dims: Vec<i64> = shape.iter().copied().collect();
    if dims.len() == 4 && dims[2] > 0 && dims[3] > 0 {
        Some((dims[3] as i32, dims[2] as i32)) // (W, H)
    } else {
        None
    }
}
