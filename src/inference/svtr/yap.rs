//! Yap拾取文字识别

use anyhow::{Context, Result};
use ndarray::Array4;
use opencv::{
    core::{Mat, Rect, Scalar, Size, CV_8UC1},
    imgproc,
    prelude::*,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::inference::session::{OrtSession, SessionOptions};

/// Yap拾取识别器
pub struct YapRecognizer {
    session: OrtSession,
    /// index -> word dict(包含分隔符"|")
    pub word_dict: HashMap<u32, String>,
    /// 字典最大index+1(vocab size)
    vocab_size: usize,
}

impl YapRecognizer {
    /// 加载模型/字典
    pub fn load(model_path: impl AsRef<Path>, dict_path: impl AsRef<Path>) -> Result<Self> {
        let session = OrtSession::with_options(model_path.as_ref(), &SessionOptions::cpu())
            .with_context(||format!("load yap model {}", model_path.as_ref().display()))?;
        let dict = load_dict(dict_path.as_ref())
            .with_context(||format!("load yap dict {}", dict_path.as_ref().display()))?;
        let vocab_size = dict.keys().max().copied().map(|m|m as usize + 1).unwrap_or(0);
        Ok(Self { session, word_dict: dict, vocab_size })
    }
    pub fn recognize(&mut self, roi: &Mat) -> Result<String> {
        let tensor = preprocess_yap(roi).context("yap preprocess")?;
        let out = self.session.run(&tensor).context("yap session.run")?;
        Ok(decode_ctc(&out, &self.word_dict, self.vocab_size))
    }
}

/// 解析`index_2_word.json`
#[derive(Deserialize)]
#[serde(transparent)]
struct DictRaw(HashMap<String, String>);

fn load_dict(path: &Path) -> Result<HashMap<u32, String>> {
    let text = std::fs::read_to_string(path).context("read dict file")?;
    let raw: DictRaw = serde_json::from_str(&text).context("parse dict json")?;
    let mut map = HashMap::new();
    for (k, v) in raw.0 {
        let idx: u32 = k.parse().with_context(||format!("dict key not int: {k}"))?;
        map.insert(idx, v);
    }
    Ok(map)
}

/// 前处理
/// 
/// BGR/灰度 -> 灰度221x32 -> 右侧黑边padding到384x32 -> /255
fn preprocess_yap(input: &Mat) -> Result<Array4<f32>> {
    // 转灰度
    let gray = if input.channels() == 1 {
        input.clone() // !as ref
    } else {
        let mut g = Mat::default();
        imgproc::cvt_color(input, &mut g, imgproc::COLOR_BGR2GRAY, 0, opencv::core::AlgorithmHint::ALGO_HINT_DEFAULT)
            .context("cvt to gray")?;
        g
    };
    // resize 221x32
    let mut resized = Mat::default();
    imgproc::resize(&gray, &mut resized, Size::new(221,32), 0.0, 0.0, imgproc::INTER_LINEAR)
        .context("resize 221x32")?;
    // 黑底384x32
    let mut padded = Mat::new_rows_cols_with_default(32, 384, CV_8UC1, Scalar::all(0.0))
        .context("padded canvas")?;
    let roi_rect = Rect::new(0, 0, 221, 32);
    let mut roi = Mat::roi_mut(&mut padded, roi_rect).context("padded roi")?;
    resized.copy_to(&mut roi).context("copy resized into padded")?;
    // [1,1,32,384] f32 / 255
    let mut tensor = Array4::<f32>::zeros((1, 1, 32, 384));
    for y in 0..32 {
        for x in 0..384 {
            let v = *padded.at_2d::<u8>(y, x).context("at_2d gray")?;
            tensor[[0, 0, y as usize, x as usize]] = v as f32 / 255.0;
        }
    }
    Ok(tensor)
}

/// 对SVTR输出做CTC解码
fn decode_ctc(
    out: &ndarray::Array<f32, ndarray::IxDyn>,
    dict: &HashMap<u32, String>,
    vocab_size: usize,
) -> String {
    let shape = out.shape();
    let (seq_len, vocab) = match shape.len() {
        3 => (shape[0], shape[2]), // [seq, 1, vocab]
        2 => (shape[0], shape[1]), // [seq, vocab]
        _ => return String::new(),
    };
    let vocab = vocab.min(vocab_size.max(vocab));
    let mut last: Option<String> = None;
    let mut buf = String::new();
    for i in 0..seq_len{
        let mut best_idx = 0usize;
        let mut best_val = f32::NEG_INFINITY;
        for j in 0..vocab {
            let v = match shape.len() {
                3 => out[[i, 0, j]],
                _ => out[[i, j]]
            };
            if v > best_val { best_val = v; best_idx = j; }
        }
        let word = dict.get(&(best_idx as u32)).cloned().unwrap_or_default();
        if word != "|" {
            if Some(&word) != last.as_ref() {
                buf.push_str(&word);
            }
        }
        last = Some(word);
    }
    buf
}
