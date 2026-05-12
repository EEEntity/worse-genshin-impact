//! 传送子系统
//! 
//! - [`data`] tp.json数据模型/最近点查询
//! - [`service`] 传送入口
//! - [`ui`] 大地图UI相关

pub mod country;
pub mod data;
pub mod service;
pub mod teyvat_coord;
pub mod ui;

pub use country::{COUNTRY_POSITIONS, nearest_country};
pub use data::{TpDatabase, TpJsonRoot, TpPosition, WorldScene};
pub use service::{ScreenProvider, TpDeps, TpTask};
pub use ui::bgr_mat_to_rgb_bytes;
pub use teyvat_coord::{Rect2048, game_to_image_2048, game_to_screen_click, image_2048_to_game, image_256_to_2048, is_point_in_big_map_window};
