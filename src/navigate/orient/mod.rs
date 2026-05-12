//! 方向识别
//! 
//! 约定向右为0度，顺时针正向
//! - [`character::compute_character_angle`] 人物三角飞镖朝向
//! - [`camera_main::compute_camera_angle`] 极坐标remap/双直方图
//! - [`camera_gia::compute_camera_angle_fallback`] 回退的算法

pub mod camera_fallback;
pub mod camera_main;
pub mod character;

pub use camera_fallback::compute_camera_angle_fallback;
pub use camera_main::{compute_camera_angle, compute_camera_angle_with_confidence};
pub use character::compute_character_angle;
