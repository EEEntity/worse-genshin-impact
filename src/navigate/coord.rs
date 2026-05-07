//! 游戏内坐标 <-> 大地图坐标

use crate::navigate::scene::SceneGeom;

/// 块大小相对1024的缩放
pub const TEYVAT_BLOCK_SCALE_TO_1024: f32 = TEYVAT_BLOCK_WIDTH as f32 / 1024.0;
/// 左上角离游戏坐标系原点的列数
pub const TEYVAT_LEFT_COLS: i32 = 15;
/// 左上角离游戏坐标系原点的行数
pub const TEYVAT_UP_ROWS: i32 = 7;
/// 单个区块边长（像素）
pub const TEYVAT_BLOCK_WIDTH: i32 = 2048;
/// 大地图原点在图像坐标系中的位置X(block内右下角)
pub const TEYVAT_ORIGIN_IMAGE_X: f32 = (TEYVAT_LEFT_COLS + 1) as f32 * TEYVAT_BLOCK_WIDTH as f32;
/// 大地图原点在图像坐标系中的位置Y
pub const TEYVAT_ORIGIN_IMAGE_Y: f32 = (TEYVAT_UP_ROWS + 1) as f32 * TEYVAT_BLOCK_WIDTH as f32;

/// 二维坐标点
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
pub struct Point2f {
    pub x: f32,
    pub y: f32,
}

impl Point2f {
    pub const fn new(x: f32, y: f32) -> Self { Self { x, y } }
    pub fn distance_to(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// 游戏坐标 -> 大地图坐标
#[inline]
pub fn game_to_image(geom: &SceneGeom, game: Point2f) -> Point2f {
    let s = geom.block_scale_to_1024();
    Point2f::new(geom.origin_x - game.x * s, geom.origin_y - game.y * s)
}
/// 大地图坐标 -> 游戏坐标
#[inline]
pub fn image_to_game(geom: &SceneGeom, image: Point2f) -> Point2f {
    let s = geom.block_scale_to_1024();
    Point2f::new((geom.origin_x - image.x) / s, (geom.origin_y - image.y) / s)
}
/// 提瓦特游戏坐标 -> 大地图坐标
#[inline]
pub fn teyvat_game_to_image(game: Point2f) -> Point2f {
    Point2f::new(
        TEYVAT_ORIGIN_IMAGE_X - game.x * TEYVAT_BLOCK_SCALE_TO_1024,
        TEYVAT_ORIGIN_IMAGE_Y - game.y * TEYVAT_BLOCK_SCALE_TO_1024,
    )
}
/// 提瓦特大地图坐标 -> 游戏坐标
#[inline]
pub fn teyvat_image_to_game(image: Point2f) -> Point2f {
    Point2f::new(
        (TEYVAT_ORIGIN_IMAGE_X - image.x) / TEYVAT_BLOCK_SCALE_TO_1024,
        (TEYVAT_ORIGIN_IMAGE_Y - image.y) / TEYVAT_BLOCK_SCALE_TO_1024,
    )
}
