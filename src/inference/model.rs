//! 模型列表
//! 模型文件可以自己修改

/// 模型目录路径
pub const MODELS_DIR: &str = "assets/models";
/// ONNX Runtime共享对象路径
pub const ORT_LIB_PATH: &str = "/usr/lib/libonnxruntime.so";
// RapidOCR模型路径
pub const MODEL_PPOCR_DET_V4: &str = "ch_PP-OCRv4_det_mobile.onnx";
pub const MODEL_PPOCR_REC_V4: &str = "ch_PP-OCRv4_rec_mobile.onnx";
pub const LABEL_PPOCR_KEYS_V1: &str = "ppocr_keys_v1.txt";

fn in_models_dir(path: &str) -> String {
    format!("{MODELS_DIR}/{path}")
}

/// ONNX模型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Model {
    PaddleOCRDetV4,
    PaddleOCRRecV4,
}

impl Model {
    pub const fn relative_path(self) -> &'static str {
        match self {
            Model::PaddleOCRDetV4 => MODEL_PPOCR_DET_V4,
            Model::PaddleOCRRecV4 => MODEL_PPOCR_REC_V4,
        }
    }
    /// 模型`.onnx`文件对应路径
    pub fn model_path(self) -> String {
        in_models_dir(self.relative_path())
    }
    /// 模型标签路径
    pub const fn label_path(self) -> Option<&'static str> {
        match self {
            Model::PaddleOCRRecV4 => Some(LABEL_PPOCR_KEYS_V1),
            _ => None,
        }
    }
    pub fn label_full_path(self) -> Option<String> {
        self.label_path().map(in_models_dir)
    }
    /// 模型名
    pub const fn name(self) -> &'static str {
        match self {
            Model::PaddleOCRDetV4 => "PaddleOCRDetV4",
            Model::PaddleOCRRecV4 => "PaddleOCRRecV4",
        }
    }
}
