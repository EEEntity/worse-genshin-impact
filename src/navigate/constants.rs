//! 导航模块常量

/// 导航模块地图SIFT缓存资源目录
pub const MAP_ASSETS_DIR: &str = "assets/maps";

/// 1920x1080小地图ROI
pub const MIMI_MAP_RECT_1080P: (i32, i32, i32, i32) = (62, 19, 212, 212);

// 固定分辨率
#[deprecated(note = "适配不同分辨率")]
pub const SCREEN_WIDTH: i32 = 1920;
#[deprecated(note = "适配不同分辨率")]
pub const SCREEN_HEIGHT: i32 = 1080;

/// 大地图区块行数
pub const TEYVAT_MAP_ROWS: i32 = 15;
/// 大地图区块列数
pub const TEYVAT_MAP_COLS: i32 = 22;
/// 左上角离游戏坐标系原点的行数
pub const TEYVAT_UP_ROWS: i32 = 7;
/// 左上角离游戏坐标系原点的列数
pub const TEYVAT_LEFT_COLS: i32 = 15;
/// 单个区块边长(pixel)
pub const TEYVAT_BLOCK_WIDTH: i32 = 2048;
/// 大地图总宽
pub const TEYVAT_MAP_WIDTH: i32 = TEYVAT_MAP_COLS * TEYVAT_BLOCK_WIDTH;
/// 大地图总高
pub const TEYVAT_MAP_HEIGHT: i32 = TEYVAT_MAP_ROWS * TEYVAT_BLOCK_WIDTH;
/// 大地图原点在图像坐标系中的位置X(block内右下角)
pub const TEYVAT_ORIGIN_IMAGE_X: f32 = (TEYVAT_LEFT_COLS + 1) as f32 * TEYVAT_BLOCK_WIDTH as f32;
/// 大地图原点在图像坐标系中的位置Y
pub const TEYVAT_ORIGIN_IMAGE_Y: f32 = (TEYVAT_UP_ROWS + 1) as f32 * TEYVAT_BLOCK_WIDTH as f32;
/// 块大小相对1024的缩放(用于游戏坐标 <-> 图像坐标)
pub const TEYVAT_BLOCK_SCALE_TO_1024: f32 = TEYVAT_BLOCK_WIDTH as f32 / 1024.0;
/// 切块行数
pub const TEYVAT_SPLIT_ROW: i32 = TEYVAT_MAP_ROWS * 2;
/// 切块列数
pub const TEYVAT_SPLIT_COL: i32 = TEYVAT_MAP_COLS * 2;

// SIFT匹配参数
/// Lowe ratio test阈值
pub const SIFT_RATIO: f32 = 0.75;
/// RANSAC重投影阈值(pixel)
pub const SIFT_RANSAC_REPROJ: f64 = 3.0;
/// good matches最少数量
pub const SIFT_MIN_GOOD: usize = 7;
/// 局部匹配时邻接块扩展数(9块)
pub const LOCAL_MATCH_NEIGHBORS: i32 = 1;

// 备用定位
/// 稳定定位时局部匹配的更高阈值
pub const STABLE_LOCAL_MIN_GOOD: usize = 15;
/// 稳定定位时新坐标相对上次的最大跳跃距离(pixel)
pub const STABLE_JUMP_THRESHOLD: f32 = 150.0;

// 视角控制
/// 鼠标dx与摄像机水平旋转角度转换
pub const DEFAULT_DEG_TO_DX: f32 = SCREEN_WIDTH as f32 / 360.0;

// 主循环
/// 到达点位的判定距离
pub const ARRIVAL_DISTANCE: f64 = 4.0;
/// 距离过远阈值
pub const TOO_FAR_DISTANCE: f64 = 500.0;
/// 卡死检测
/// 连续N帧位移小于阈值
pub const STUCK_FRAMES: usize = 8;
/// 卡死位移阈值
pub const STUCK_DELTA: f64 = 3.0;
/// 主循环帧间隔
/// 
/// 或许用线程阻塞更合适
pub const FRAME_INTERVAL_MS: u64 = 100;
