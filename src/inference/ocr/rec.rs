// 识别预处理/CTC解码
use anyhow::{Context, Result};
use ndarray::{Array3, Array4, Axis};
use opencv::{core::{Mat, Size}, imgproc::{self, INTER_LINEAR}, prelude::*};
use rayon::prelude::*;
use std::{fs, path::Path};

const IMG_H: usize = 48;
const IMG_W_BASE: usize = 320;

/// 宽高比升序排序
pub fn rec_sort_indices(crops: &[Mat]) -> Vec<usize> {
    let ratios: Vec<f64> = crops.iter().map(|m| m.cols() as f64 / m.rows().max(1) as f64).collect();
    let mut indices: Vec<usize> = (0..crops.len()).collect();
    indices.sort_by(|&a, &b| ratios[a].partial_cmp(&ratios[b]).unwrap());
    indices
}

/// 预处理crops
pub fn rec_preprocess_one_batch(crops: &[&Mat]) -> Result<Array4<f32>> {
    if crops.is_empty() {
        return Ok(Array4::zeros((0, 3, IMG_H, 1)));
    }
    let max_wh_ratio = crops
        .iter()
        .map(|m| m.cols() as f64 / m.rows().max(1) as f64)
        .fold(IMG_W_BASE as f64 / IMG_H as f64, f64::max);
    let img_w = (IMG_H as f64 * max_wh_ratio).ceil() as usize;
    let n = crops.len();
    let mut batch = Array4::<f32>::zeros((n, 3, IMG_H, img_w));
    for (batch_idx, crop) in crops.iter().enumerate() {
        let resized_w = ((IMG_H as f64 * (crop.cols() as f64 / crop.rows().max(1) as f64)).ceil() as usize)
            .min(img_w)
            .max(1);
        let mut resized = Mat::default();
        imgproc::resize(crop, &mut resized, Size::new(resized_w as i32, IMG_H as i32), 0.0, 0.0, INTER_LINEAR)
            .context("rec resize failed")?;
        for c in 0..3usize {
            for row in 0..IMG_H {
                let row_ptr = resized.ptr(row as i32).context("ptr failed")?;
                for col in 0..resized_w {
                    let v = unsafe { *row_ptr.add(col * 3 + c) } as f32;
                    batch[[batch_idx, c, row, col]] = v / 127.5 - 1.0;
                }
            }
        }
    }
    Ok(batch)
}

/// 字符字典/CTC贪心解码
pub struct CtcDecoder {
    chars: Vec<String>,
}

impl CtcDecoder {
    /// 加载字典
    pub fn load(dict_path: impl AsRef<Path>) -> Result<Self> {
        let content = fs::read_to_string(&dict_path)
            .with_context(|| format!("failed to read dict: {}", dict_path.as_ref().display()))?;
        let mut chars: Vec<String> = Vec::with_capacity(6630);
        chars.push("blank".to_string());
        for line in content.lines() {
            let ch = line.trim_end_matches(['\r', '\n']);
            if !ch.is_empty() { chars.push(ch.to_string()); }
        }
        chars.push(" ".to_string());
        Ok(Self { chars })
    }
    /// 对logits([N,T,C])贪心解码
    pub fn decode_batch(&self, logits: &Array3<f32>) -> Vec<(String, f32)> {
        let n = logits.dim().0;
        (0..n).into_par_iter().map(|i| self.decode_one(logits.index_axis(Axis(0), i))).collect()
    }
    fn decode_one(&self, seq: ndarray::ArrayView2<f32>) -> (String, f32) {
        let t = seq.dim().0;
        let mut chars_out: Vec<&str> = Vec::new();
        let mut probs: Vec<f32> = Vec::new();
        let mut prev_id = 0usize;
        for t_idx in 0..t {
            let step = seq.index_axis(Axis(0), t_idx);
            let (max_id, max_prob) = step
                .iter()
                .cloned()
                .enumerate()
                .fold((0, f32::NEG_INFINITY), |(bi, bv), (i, v)| if v > bv { (i, v) } else { (bi, bv) });
            if max_id != 0 && max_id != prev_id {
                if let Some(ch) = self.chars.get(max_id) {
                    chars_out.push(ch.as_str());
                    probs.push(max_prob);
                }
            }
            prev_id = max_id;
        }
        let text = chars_out.join("");
        let score = if probs.is_empty() { 0.0 } else { probs.iter().sum::<f32>() / probs.len() as f32 };
        (text, score)
    }
}
