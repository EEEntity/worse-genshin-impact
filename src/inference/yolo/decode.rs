//! YOLOv8解码后处理

use ndarray::{Array, IxDyn};

use super::letterbox::Letterbox;

/// 单个检测框
#[derive(Debug, Clone, PartialEq)]
pub struct  Detection {
    /// 原图坐标系`(x,y,w,h)`
    pub bbox: Bbox,
    /// 类别索引
    pub class_id: usize,
    /// 类别名(来自onnx metadata)
    pub label: String,
    /// 置信度
    pub score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bbox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Bbox {
    pub fn x1(&self) -> f32 { self.x }
    pub fn y1(&self) -> f32 { self.y }
    pub fn x2(&self) -> f32 { self.x + self.w }
    pub fn y2(&self) -> f32 { self.y + self.h }
    pub fn iou(&self, other: &Bbox) -> f32 {
        let x1 = self.x1().max(other.x1());
        let y1 = self.y1().max(other.y1());
        let x2 = self.x2().min(other.x2());
        let y2 = self.y2().min(other.y2());
        let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
        let a = self.w * self.h;
        let b = other.w * other.h;
        let union = a + b - inter;
        if union <= 0.0 { 0.0 } else { inter / union }
    }
}

/// 解码
/// - `out`: `[1,4+nc,na]`
/// - `letterbox`: 前处理metadata
/// - `labels`: 类别名表(nc)
/// - `conf_thr`: 置信度阈值
/// - `iou_thr`: NMS IoU阈值
pub fn decode_yolov8(
    out: &Array<f32, IxDyn>,
    letterbox: &Letterbox,
    labels: &[String],
    conf_thr: f32,
    iou_thr: f32,
) -> Vec<Detection> {
    let shape = out.shape();
    let (nc_plus_4, na) = match shape.len() {
        3 if shape[0] == 1 => (shape[1], shape[2]),
        2 => (shape[0], shape[1]),
        _ => return  vec![],
    };
    if nc_plus_4 < 5 { return vec![]; }
    let nc = nc_plus_4 - 4;
    let view = out.view().into_shape_with_order((nc_plus_4, nc));
    let mat = match view {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let mut raw: Vec<Detection> = Vec::new();
    let (orig_w, orig_h) = (letterbox.orig_size.0 as f32, letterbox.orig_size.1 as f32);
    for j in 0..na {
        // argmax类别
        let mut best_cls = 0usize;
        let mut best_score = 0.0f32;
        for c in 0..nc {
            let s = mat[[4+c, j]];
            if s > best_score { best_score = s; best_cls = c; }
        }
        if best_score < conf_thr { continue; }
        let cx = mat[[0, j]];
        let cy = mat[[1, j]];
        let w = mat[[2, j]];
        let h = mat[[3, j]];
        // letterbox坐标 -> 原图坐标
        let (x1l, y1l) = (cx - w / 2.0, cy - h / 2.0);
        let (x2l, y2l) = (cx + w / 2.0, cy + h / 2.0);
        let (x1, y1) = letterbox.unproject_xy(x1l, y1l);
        let (x2, y2) = letterbox.unproject_xy(x2l, y2l);
        // 裁到原图边界
        let x1 = x1.clamp(0.0, orig_w);
        let y1 = y1.clamp(0.0, orig_h);
        let x2 = x2.clamp(0.0, orig_w);
        let y2 = y2.clamp(0.0, orig_h);
        if x2 <= x1 || y2 <= y1 { continue; }
        let label = labels.get(best_cls).cloned().unwrap_or_else(|| best_cls.to_string());
        raw.push(Detection {
            bbox: Bbox { x: x1, y: y1, w: x2 - x1, h: y2 - y1 },
            class_id: best_cls,
            label,
            score: best_score,
        });
    }
    nms(raw, iou_thr)
}

/// 类内NMS
fn nms(mut dets: Vec<Detection>, iou_thr: f32) -> Vec<Detection> {
    dets.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut keep: Vec<Detection> = Vec::with_capacity(dets.len());
    'outer: for d in dets {
        for k in &keep {
            if k.class_id == d.class_id && k.bbox.iou(&d.bbox) > iou_thr {
                continue 'outer;
            }
        }
        keep.push(d);
    }
    keep
}
