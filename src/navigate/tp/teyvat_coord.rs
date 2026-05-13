//! 提瓦特大地图坐标/游戏坐标/256-scale大地图坐标转换
//! 
//! - 游戏坐标: 程序中使用的坐标
//! 需要完善，坐标不太一样

use crate::navigate::coord::Point2f;

pub const TEYVAT_ORIGIN_X_2048: f64 = 32768.0; // (15+1)*2048
pub const TEYVAT_ORIGIN_Y_2048: f64 = 16384.0; // (7+1)*2048
pub const TEYVAT_BLOCK_SCALE: f64 = 2.0;       // 2048/1024
pub const BIG_MAP_256_TO_2048: f64 = 8.0;

/// 游戏坐标 -> 2048 image坐标
pub fn game_to_image_2048(p: Point2f) -> (f64, f64) {
    let ix = TEYVAT_ORIGIN_X_2048 - (p.x as f64) * TEYVAT_BLOCK_SCALE;
    let iy = TEYVAT_ORIGIN_Y_2048 - (p.y as f64) * TEYVAT_BLOCK_SCALE;
    (ix, iy)
}
/// 2048 image坐标 -> 游戏坐标
pub fn image_2048_to_game(ix: f64, iy: f64) -> Point2f {
    let gx = (TEYVAT_ORIGIN_X_2048 - ix) / TEYVAT_BLOCK_SCALE;
    let gy = (TEYVAT_ORIGIN_Y_2048 - iy) / TEYVAT_BLOCK_SCALE;
    Point2f { x: gx as f32, y: gy as f32 }
}
/// 256-scale坐标 -> 2048 image坐标
pub fn image_256_to_2048(rect256: Rect2048) -> Rect2048 {
    Rect2048 {
        x: rect256.x * BIG_MAP_256_TO_2048,
        y: rect256.y * BIG_MAP_256_TO_2048,
        w: rect256.w * BIG_MAP_256_TO_2048,
        h: rect256.h * BIG_MAP_256_TO_2048,
    }
}

/// 2048 image坐标下的矩形
#[derive(Debug, Clone, Copy)]
pub struct Rect2048 {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect2048 {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self { Self { x, y, w, h } }
    pub fn left(&self) -> f64 { self.x }
    pub fn top(&self) -> f64 { self.y }
    pub fn right(&self) -> f64 { self.x + self.w }
    pub fn bottom(&self) -> f64 { self.y + self.h }
    pub fn center_x(&self) -> f64 { self.x + self.w * 0.5 }
    pub fn center_y(&self) -> f64 { self.y + self.h * 0.5 }
}

/// 转换为屏幕坐标
/// 传入游戏窗口大小
pub fn game_to_screen_click(
    big_map_in_2048: Rect2048,
    game: Point2f,
    screen_w: i32,
    screen_h: i32,
) -> (i32, i32) {
    let (gx2048, gy2048) = game_to_image_2048(game);
    let click_x = (gx2048 - big_map_in_2048.x) / big_map_in_2048.w * screen_w as f64;
    let click_y = (gy2048 - big_map_in_2048.y) / big_map_in_2048.h * screen_h as f64;
    (click_x.round() as i32, click_y.round() as i32)
}
/// 当前大地图(2048)是否包含`(x,y)`(同2048)
/// 避开顶部UI和四周边框
pub fn is_point_in_big_map_window(
    big_map_in_2048: Rect2048,
    target_2048_x: f64,
    target_2048_y: f64,
    screen_w: i32,
    screen_h: i32,
) -> bool {
    if target_2048_x < big_map_in_2048.left() || target_2048_x > big_map_in_2048.right() {
        return false;
    }
    if target_2048_y < big_map_in_2048.top() || target_2048_y > big_map_in_2048.bottom() {
        return false;
    }
    // 转成屏幕像素
    let sx = (target_2048_x - big_map_in_2048.x) / big_map_in_2048.w * screen_w as f64;
    let sy = (target_2048_y - big_map_in_2048.y) / big_map_in_2048.h * screen_h as f64;
    // 屏幕四周 115px 边框
    if sx < 115.0 || sx > (screen_w as f64 - 115.0) {
        return false;
    }
    if sy < 115.0 || sy > (screen_h as f64 - 115.0) {
        return false;
    }
    // 左上 360×400 国家面板 / 名片区
    if sx < 360.0 && sy < 400.0 {
        return false;
    }
    true
}
