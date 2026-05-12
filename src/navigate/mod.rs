//! 路径导航模块
//! 
//! 兼容BGI路径与地图坐标系，用tokio异步
//! 
//! 子模块:
//! - [`path`] 路径json解析
//! - [`orient`] 角色/摄像机朝向识别
//! - [`rotate`] 视角控制
//! - [`map`] 大地图/SIFT缓存
//! - [`locate`] 小地图定位
//! - [`coord`] 坐标转换
//! - [`bv`] 视觉辅助
//! - [`trap`] 卡死脱困

pub mod path;
pub mod orient;
pub mod error;
pub mod rotate;
pub mod scene;
pub mod map;
pub mod locate;
pub mod coord;
pub mod bv;
pub mod trap;

pub use error::NavigateError;
// 公开模块功能
