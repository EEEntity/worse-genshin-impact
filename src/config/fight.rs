//! 战斗配置

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::device::KeyBindingsLoadError;
use crate::device::constants::{FIGHT_FINISH_DELAY_MS, FIGHT_FINISH_DETECT_DELAY_MS};

/// 战斗结束读条颜色默认值(BGR)
pub const DEFAULT_END_BAR_COLOR: (u8, u8, u8) = (95, 235, 255);
/// 战斗结束读条颜色容差默认值(BGR)
pub const DEFAULT_END_BAR_COLOR_TOLERANCE: (u8, u8, u8) = (6, 6, 6);

/// 战斗结束检测配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FightFinishDetectConfig {
    /// 读条颜色(BGR)
    #[serde(default)]
    pub battle_end_progress_bar_color: String,
    /// 读条颜色容差(BGR)
    #[serde(default)]
    pub battle_end_progress_bar_color_tolerance: String,
    /// 快速检查战斗结束
    #[serde(default)]
    pub fast_check_enabled: bool,
    /// 旋转寻找敌人
    #[serde(default)]
    pub rotate_find_enemy_enabled: bool,
    /// 快速检查参数mini-DSL
    /// 由fight模块解析
    #[serde(default)]
    pub fast_check_params: String,
    /// 检查战斗结束的延时mini-DSL
    #[serde(default)]
    pub check_end_delay: String,
    /// 切队检测前的色块延时mini-DSL
    #[serde(default)]
    pub before_detect_delay: String,
    /// 旋转因子
    #[serde(default = "default_rotary_factor")]
    pub rotary_factor: i32,
    /// 是否首次检查
    #[serde(default)]
    pub is_first_check: bool,
    /// 元素爆发前是否检查战斗结束
    #[serde(default)]
    pub check_before_burst: bool,
}

fn default_rotary_factor() -> i32 {
    10
}

impl Default for FightFinishDetectConfig {
    fn default() -> Self {
        Self {
            battle_end_progress_bar_color: String::new(),
            battle_end_progress_bar_color_tolerance: String::new(),
            fast_check_enabled: false,
            rotate_find_enemy_enabled: false,
            fast_check_params: String::new(),
            check_end_delay: String::new(),
            before_detect_delay: String::new(),
            rotary_factor: default_rotary_factor(),
            is_first_check: false,
            check_before_burst: false,
        }
    }
}

impl FightFinishDetectConfig {
    /// 解析读条颜色字符串
    pub fn parsed_end_bar_color(&self) -> (u8, u8, u8) {
        parse_bgr(&self.battle_end_progress_bar_color).unwrap_or(DEFAULT_END_BAR_COLOR)
    }
    /// 解析读条颜色容差
    pub fn parsed_end_bar_color_tolerance(&self) -> (u8, u8, u8) {
        let s = self.battle_end_progress_bar_color_tolerance.trim();
        if s.is_empty() {
            return DEFAULT_END_BAR_COLOR_TOLERANCE;
        }
        if let Some(t) = parse_bgr(s) {
            return t;
        }
        // 单数字 -> BGR相同值
        if let Ok(n) = s.parse::<u8>() {
            return (n, n, n);
        }
        DEFAULT_END_BAR_COLOR_TOLERANCE
    }
}

fn parse_bgr(s: &str) -> Option<(u8, u8, u8)> {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    if parts.len() != 3 {
        return None;
    }
    Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
}

/// 自动战斗配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoFightConfig {
    #[serde(default)]
    pub strategy_name: String,
    /// 强制指定队伍角色(','分隔)
    #[serde(default)]
    pub team_names: String,
    /// 是否启用战斗结束检测
    #[serde(default = "default_true")]
    pub fight_finish_detect_enabled: bool,
    /// CD调度mini-DSL
    #[serde(default)]
    pub action_scheduler_by_cd: String,
    /// 只拾取精英掉落模式
    #[serde(default = "default_only_pick_mode")]
    pub only_pick_elite_drops_mode: String,
    /// 战斗结束子配置
    #[serde(default)]
    pub finish_detect: FightFinishDetectConfig,
    /// 战斗结束后是否触发拾取掉落
    #[serde(default)]
    pub pick_drops_after_fight_enabled: bool,
    /// 拾取等待秒数
    #[serde(default = "default_pick_drops_seconds")]
    pub pick_drops_after_fight_seconds: i32,
    /// 拾取战斗人次阈值
    #[serde(default)]
    pub battle_threshold_for_loot: Option<i32>,
    /// 启用万叶拾取
    #[serde(default = "default_true")]
    pub kazuha_pickup_enabled: bool,
    #[serde(default)]
    pub qin_double_pick_up: bool,
    #[serde(default)]
    pub guardian_avatar: String,
    #[serde(default)]
    pub guardian_combat_skip: bool,
    #[serde(default)]
    pub skip_model: bool,
    #[serde(default)]
    pub guardian_avatar_hold: bool,
    #[serde(default)]
    pub burst_enabled: bool,
    /// 万叶替补队伍名称
    #[serde(default)]
    pub kazuha_party_name: String,
    /// 启用游泳脱困
    #[serde(default)]
    pub swimming_enabled: bool,
    #[serde(default)]
    pub exp_based_pickup_enabled: bool,
    /// 战斗超时
    #[serde(default = "default_timeout")]
    pub timeout: i32,
}

fn default_true() -> bool {
    true
}
fn default_only_pick_mode() -> String {
    "Closed".to_string()
}
fn default_pick_drops_seconds() -> i32 {
    15
}
fn default_timeout() -> i32 {
    120
}

impl Default for AutoFightConfig {
    fn default() -> Self {
        Self {
            strategy_name: String::new(),
            team_names: String::new(),
            fight_finish_detect_enabled: true,
            action_scheduler_by_cd: String::new(),
            only_pick_elite_drops_mode: default_only_pick_mode(),
            finish_detect: FightFinishDetectConfig::default(),
            pick_drops_after_fight_enabled: false,
            pick_drops_after_fight_seconds: default_pick_drops_seconds(),
            battle_threshold_for_loot: None,
            kazuha_pickup_enabled: true,
            qin_double_pick_up: false,
            guardian_avatar: String::new(),
            guardian_combat_skip: false,
            skip_model: false,
            guardian_avatar_hold: false,
            burst_enabled: false,
            kazuha_party_name: String::new(),
            swimming_enabled: false,
            exp_based_pickup_enabled: false,
            timeout: default_timeout(),
        }
    }
}

impl AutoFightConfig {
    /// 解析`check_end_delay`战斗结束等待时长
    pub fn check_end_delay_ms(&self) -> u64 {
        first_seconds_segment(&self.finish_detect.check_end_delay)
            .map(|s| (s * 1000.0) as u64)
            .unwrap_or(FIGHT_FINISH_DELAY_MS)
    }
    /// 切队检测前色块延时
    pub fn before_detect_delay_ms(&self) -> u64 {
        first_seconds_segment(&self.finish_detect.before_detect_delay)
            .map(|s| (s * 1000.0) as u64)
            .unwrap_or(FIGHT_FINISH_DETECT_DELAY_MS)
    }
    /// 默认配置路径`$HOME/.config/worse-genshin-impact/fight_config.json`
    pub fn default_path() -> PathBuf {
        default_path()
    }
    /// 失败时回退默认
    pub fn load_or_default(path: &Path) -> Self {
        match Self::try_load(path) {
            Ok(c) => {
                log::info!("autofight config loaded from {}", path.display());
                c
            }
            Err(e) => {
                log::warn!(
                    "failed to load autofight config from {}: {e}; falling back to defaults",
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
}

pub fn default_path() -> PathBuf {
    match std::env::var_os("HOME") {
        Some(home) => PathBuf::from(home).join(".config/worse-genshin-impact/fight_config.json"),
        None => PathBuf::from("fight_config.json"),
    }
}

/// mini-DSL第一段时间
fn first_seconds_segment(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let head = s.split(';').next().unwrap_or("").trim();
    head.parse::<f64>().ok()
}
