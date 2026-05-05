/// Det短边限制
pub const DET_LIMIT_SIDE_LEN: i32 = 400;
/// 全局图像最大边长
pub const MAX_SIDE_LEN: i32 = 2000;
/// 全局图像最小边长
pub const MIN_SIDE_LEN: i32 = 30;
/// 宽高比超过此值时补竖向padding
pub const WIDTH_HEIGHT_RATIO: f64 = 8.0;
/// 补padding的最小高度
pub const MIN_HEIGHT: i32 = 30;
/// 每批识别最大crop数
pub const REC_BATCH_NUM: usize = 6;
/// 识别置信度阈值
pub const TEXT_SCORE: f32 = 0.5;
