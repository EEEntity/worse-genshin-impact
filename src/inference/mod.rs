//! 图像推理入口
//! 
//! - [`session`] ORT Session
//! - [`model`] 模型
//! - [`ocr`] PaddleOCR(DB&CRNN)通用文字识别
//! - [`svtr`] Yap拾取文字识别

pub mod model;
pub mod ocr;
pub mod svtr;
pub mod yolo;
pub mod session;
pub mod grid_icon;

pub use model::Model;
pub use svtr::YapRecognizer;
pub use session::{OrtSession, Provider, SessionOptions};
