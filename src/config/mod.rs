//! 全局配置

pub mod combat_avatar;
pub mod fight;
pub mod fishing;
pub mod keybindings;

pub use fight::AutoFightConfig;
pub use fishing::AutoFishingGlobalConfig;
pub use combat_avatar::{CombatAvatar, CombatAvatarRegistry, registry};
pub use keybindings::keybindings_path;
// 上层只需要use crate::config::*
pub use crate::device::{KeyBindingsConfig, KeyBindingsLoadError};

/// 全部配置
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub keybindings: KeyBindingsConfig,
    pub autofight: AutoFightConfig,
    pub auto_fishing: AutoFishingGlobalConfig,
}

impl AppConfig {
    /// 按默认路径加载所有配置
    pub fn load_or_default() -> Self {
        Self {
            keybindings: KeyBindingsConfig::load_or_default(&keybindings_path()),
            autofight: AutoFightConfig::load_or_default(&fight::default_path()),
            auto_fishing: AutoFishingGlobalConfig::load_or_default(
                &AutoFishingGlobalConfig::default_path(),
            ),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            keybindings: KeyBindingsConfig::default(),
            autofight: AutoFightConfig::default(),
            auto_fishing: AutoFishingGlobalConfig::default(),
        }
    }
}
