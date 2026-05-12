//! YOLO目标检测/分类器
//! 
//! - [`letterbox`]: YOLOv8保比resize/灰边padding
//! - [`decode`]: YOLOv8 解码/NMS
//! - [`predictor`]: [`YoloPredictor`] 检测器
//! - [`classifier`]: [`OrtClassifier`] 分类器

pub mod classifier;
pub mod decode;
pub mod letterbox;
pub mod predictor;

pub use classifier::{Classification, OrtClassifier};
pub use decode::{Bbox, Detection};
pub use letterbox::{letterbox_bgr, Letterbox};
pub use predictor::YoloPredictor;
