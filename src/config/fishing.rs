//! 自动钓鱼全局配置

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::device::KeyBindingsLoadError;
use crate::task::fishing::{BaitType, FishingTimePolicy, BIG_FISH_TYPES};

/// 自动钓鱼全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoFishingGlobalConfig {
    /// 当前装备的鱼饵
    #[serde(default, with = "opt_bait_type_serde")]
    pub equipped_bait: Option<BaitType>,
    /// 时段选项
    #[serde(default)]
    pub fishing_time_policy: FishingTimePolicyDe,
    /// 整轮超时时间，默认600秒
    #[serde(default = "default_whole_timeout")]
    pub whole_process_timeout_secs: u64,
    /// 抛竿后等待咬钩超时秒数，默认18秒
    #[serde(default = "default_throw_timeout")]
    pub throw_rod_timeout_secs: u64,
    /// 鱼饵白名单
    #[serde(default)]
    pub bait_whitelist: Option<Vec<String>>,
    /// 鱼类白名单
    #[serde(default)]
    pub fish_whitelist: Option<Vec<String>>,
}

impl Default for AutoFishingGlobalConfig {
    fn default() -> Self {
        Self {
            equipped_bait: None,
            fishing_time_policy: FishingTimePolicyDe::Both,
            whole_process_timeout_secs: default_whole_timeout(),
            throw_rod_timeout_secs: default_throw_timeout(),
            bait_whitelist: None,
            fish_whitelist: None,
        }
    }
}

fn default_whole_timeout() -> u64 {
    600
}
fn default_throw_timeout() -> u64 {
    18
}

impl AutoFishingGlobalConfig {
    /// 默认路径`$HOME/.config/worse-genshin-impact/fishing_config.json`
    pub fn default_path() -> PathBuf {
        match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(".config/worse-genshin-impact/fishing_config.json"),
            None => PathBuf::from("fishing_config.json"),
        }
    }
    pub fn load_or_default(path: &Path) -> Self {
        match Self::try_load(path) {
            Ok(c) => {
                log::info!("fishing config loaded from {}", path.display());
                c
            }
            Err(e) => {
                log::warn!(
                    "failed to load fishing config from {}: {e}; using defaults",
                    path.display()
                );
                Self::default()
            }
        }
    }
    pub fn try_load(path: &Path) -> Result<Self, KeyBindingsLoadError> {
        let text = fs::read_to_string(path).map_err(KeyBindingsLoadError::Io)?;
        serde_json::from_str(&text).map_err(KeyBindingsLoadError::Parse)
    }
    pub fn save(&self, path: &Path) -> Result<(), KeyBindingsLoadError> {
        if let Some(p) = path.parent() {
            fs::create_dir_all(p).map_err(KeyBindingsLoadError::Io)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(KeyBindingsLoadError::Parse)?;
        fs::write(path, text).map_err(KeyBindingsLoadError::Io)
    }
    /// 转换为任务层的配置
    pub fn to_task_config(&self) -> crate::task::fishing::AutoFishingConfig {
        let mut c = crate::task::fishing::AutoFishingConfig::new(self.equipped_bait);
        c.whole_process_timeout_secs = self.whole_process_timeout_secs;
        c.throw_rod_timeout_secs = self.throw_rod_timeout_secs;
        c.fishing_time_policy = self.fishing_time_policy.into();
        c.bait_whitelist = self.bait_whitelist.as_ref().map(|v| {
            v.iter()
                .filter_map(|s| match BaitType::from_chinese_name(s) {
                    Some(b) => Some(b),
                    None => {
                        log::warn!("auto_fishing: 未知饵中文名 `{s}`，已跳过");
                        None
                    }
                })
                .collect()
        });
        // 鱼类白名单
        c.fish_whitelist = self.fish_whitelist.as_ref().map(|v| {
            v.iter()
                .filter_map(|s| {
                    BIG_FISH_TYPES
                        .iter()
                        .find(|f| f.chinese_name == s)
                        .map(|f| f.name)
                        .or_else(|| {
                            log::warn!("auto_fishing: 未知鱼中文名 `{s}`，已跳过");
                            None
                        })
                })
                .collect()
        });
        c
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FishingTimePolicyDe {
    DontChange,
    #[default]
    Both,
    Daytime,
    Nighttime,
}

impl From<FishingTimePolicyDe> for FishingTimePolicy {
    fn from(v: FishingTimePolicyDe) -> Self {
        match v {
            FishingTimePolicyDe::DontChange => Self::DontChange,
            FishingTimePolicyDe::Daytime => Self::Daytime,
            FishingTimePolicyDe::Nighttime => Self::Nighttime,
            FishingTimePolicyDe::Both => Self::Both,
        }
    }
}

mod opt_bait_type_serde {
    use super::BaitType;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(b: &Option<BaitType>, ser: S) -> Result<S::Ok, S::Error> {
        match b {
            Some(b) => ser.serialize_str(bait_to_str(*b)),
            None => ser.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Option<BaitType>, D::Error> {
        let s: Option<String> = Option::deserialize(de)?;
        match s {
            None => Ok(None),
            Some(s) => str_to_bait(&s)
                .map(Some)
                .ok_or_else(|| serde::de::Error::custom(format!("unknown bait `{s}`"))),
        }
    }
    /// 鱼饵枚举值 -> 字符串
    fn bait_to_str(b: BaitType) -> &'static str {
        match b {
            BaitType::FruitPasteBait => "fruit_paste",
            BaitType::RedrotBait => "redrot",
            BaitType::FalseWormBait => "false_worm",
            BaitType::FakeFlyBait => "fake_fly",
            BaitType::SugardewBait => "sugardew",
            BaitType::SourBait => "sour",
            BaitType::FlashingMaintenanceMekBait => "flashing_maintenance_mek",
            BaitType::SpinelgrainBait => "spinelgrain",
            BaitType::EmberglowBait => "emberglow",
            BaitType::BerryBait => "berry",
            BaitType::RefreshingLakkaBait => "refreshing_lakka",
        }
    }
    /// 字符串(多种写法) -> 鱼饵枚举值
    fn str_to_bait(s: &str) -> Option<BaitType> {
        let s = s.to_ascii_lowercase().replace('-', "_");
        Some(match s.as_str() {
            "fruit" | "fruitpaste" | "fruit_paste" | "fruitpastebait" | "fruit_paste_bait" => {
                BaitType::FruitPasteBait
            }
            "redrot" | "redrotbait" | "redrot_bait" => BaitType::RedrotBait,
            "worm" | "falseworm" | "false_worm" | "false_worm_bait" => BaitType::FalseWormBait,
            "fly" | "fakefly" | "fake_fly" | "fake_fly_bait" => BaitType::FakeFlyBait,
            "sugardew" | "sugardew_bait" => BaitType::SugardewBait,
            "sour" | "sour_bait" => BaitType::SourBait,
            "mek" | "flashing" | "flashing_maintenance_mek" | "flashing_maintenance_mek_bait" => {
                BaitType::FlashingMaintenanceMekBait
            }
            "spinelgrain" | "spinelgrain_bait" => BaitType::SpinelgrainBait,
            "emberglow" | "emberglow_bait" => BaitType::EmberglowBait,
            "berry" | "berry_bait" => BaitType::BerryBait,
            "lakka" | "refreshing_lakka" | "refreshing_lakka_bait" => BaitType::RefreshingLakkaBait,
            _ => return None,
        })
    }
}
