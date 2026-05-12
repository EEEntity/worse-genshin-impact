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
// 角色侧栏
pub const MODEL_AVATAR_SIDE: &str = "Common/avatar_side_classify_sim.onnx";
// Q图标分类
pub const MODEL_Q_CLASSIFY: &str = "Common/q_classify_sim.onnx";
// 大世界
pub const MODEL_WORLD: &str = "World/bgi_world.onnx";
// 秘境古树
pub const MODEL_TREE: &str = "Domain/bgi_tree.onnx";
// 鱼类
pub const MODEL_FISH: &str = "Fish/bgi_fish.onnx";
// Yap
pub const MODEL_YAP: &str = "Yap/model_training.onnx";
pub const LABEL_YAP: &str = "Yap/index_2_word.json";
// 物品grid图标分类
pub const MODEL_GRID_ICON: &str = "Item/gridIcon.onnx";
pub const LABEL_GRID_ICON: &str = "Item/items.csv";

fn in_models_dir(path: &str) -> String {
    format!("{MODELS_DIR}/{path}")
}

/// ONNX模型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Model {
    PaddleOCRDetV4,
    PaddleOCRRecV4,
    AvatarSide,
    QClassify,
    World,
    Tree,
    Fish,
    Yap,
    GridIcon,
}

impl Model {
    pub const fn relative_path(self) -> &'static str {
        match self {
            Model::PaddleOCRDetV4 => MODEL_PPOCR_DET_V4,
            Model::PaddleOCRRecV4 => MODEL_PPOCR_REC_V4,
            Model::AvatarSide => MODEL_AVATAR_SIDE,
            Model::QClassify => MODEL_Q_CLASSIFY,
            Model::World => MODEL_WORLD,
            Model::Tree => MODEL_TREE,
            Model::Fish => MODEL_FISH,
            Model::Yap => MODEL_YAP,
            Model::GridIcon => MODEL_GRID_ICON,
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
            Model::Yap => Some(LABEL_YAP),
            Model::GridIcon => Some(LABEL_GRID_ICON),
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
            Model::AvatarSide => "AvatarSide",
            Model::QClassify => "QClassify",
            Model::World => "World",
            Model::Tree => "Tree",
            Model::Fish => "Fish",
            Model::Yap => "Yap",
            Model::GridIcon => "GridIcon",
        }
    }
}
