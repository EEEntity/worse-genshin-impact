//! 路径导航模块
//! 
//! 兼容BGI路径与地图坐标系，用tokio异步
//! 
//! 子模块:
//! - [`path`] 路径json解析

pub mod path;
pub mod error;

pub use error::NavigateError;
