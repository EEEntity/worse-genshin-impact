//! 图像推理入口
//! 
//! - [`session`] ORT Session
//! - [`model`] 模型
//! - [`ocr`] PaddleOCR(DB&CRNN)通用文字识别

pub mod model;
pub mod ocr;
pub mod session;

pub use model::Model;
pub use session::{OrtSession, Provider, SessionOptions};
