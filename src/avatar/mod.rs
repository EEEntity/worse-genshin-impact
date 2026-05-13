//! 队伍/角色/战斗场景层

pub mod assets;
pub mod avatar;
pub mod predictor;
pub mod scenes;

pub use avatar::{Avatar, AvatarError, CancelFlag, WalkDir, cancel_flag};
pub use predictor::{AvatarPredictor, ConfigPredictor, StubPredictor};
pub use scenes::{CombatScenes, get_active_avatar_index_from_indices};
