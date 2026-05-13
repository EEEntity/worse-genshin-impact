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
//! - [`tp`] 传送子系统
//! - [`action`] 动作处理
//! - [`executor`] 执行器

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
pub mod tp;
pub mod action;
pub mod constants;
pub mod executor;

pub use action::{ActionContext, ActionHandler, ActionRegistry};
pub use coord::Point2f;
pub use error::NavigateError;
pub use executor::{MinimapSource, MoveOutcome, PathExecutor, target_orientation_deg};
pub use locate::{LocateInfo, Locator, MultiSceneLocator};
pub use orient::{compute_camera_angle, compute_character_angle};
pub use rotate::RotateController;
pub use scene::{ALL_SCENES, Scene, SceneGeom};
pub use tp::{TpDatabase, TpTask};
pub use trap::TrapEscaper;
