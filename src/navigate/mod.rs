//! 路径导航模块
//! 
//! 兼容BGI路径与地图坐标系，用tokio异步
//! 
//! 子模块:
//! - [`path`] 路径json解析
//! - [`orient`] 角色/摄像机朝向识别
//! - [`rotate`] 视角控制

pub mod path;
pub mod orient;
pub mod error;
pub mod rotate;

pub use error::NavigateError;
