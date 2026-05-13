//! 角色静态信息

use std::collections::HashMap;
use std::sync::OnceLock;
use serde::{Deserialize, Serialize};

const COMBAT_AVATAR_JSON: &str = include_str!("../../assets/configs/combat_avatar.json");

/// 单个角色信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatAvatar {
    /// ID
    #[serde(default)]
    pub id: String,
    /// 中文名
    #[serde(default)]
    pub name: String,
    /// 英文名(for YOLO model)
    #[serde(rename = "nameEn", default)]
    pub name_en: String,
    /// 武器类型
    #[serde(default)]
    pub weapon: String,
    /// 元素战技CD
    #[serde(rename = "skillCD", default)]
    pub skill_cd: f64,
    /// 长按元素战技CD
    #[serde(rename = "skillHoldCD", default)]
    pub skill_hold_cd: f64,
    /// 元素爆发CD
    #[serde(rename = "burstCD", default)]
    pub burst_cd: f64,
    /// 别名
    #[serde(default)]
    pub alias: Vec<String>,
}

pub struct CombatAvatarRegistry {
    all: Vec<CombatAvatar>,
    by_id: HashMap<String, usize>,
    by_name: HashMap<String, usize>,
    by_name_en: HashMap<String, usize>,
    /// 别名 -> 角色索引
    by_alias: HashMap<String, usize>,
}

impl CombatAvatarRegistry {
    fn from_json(text: &str) -> Self {
        let all: Vec<CombatAvatar> = serde_json::from_str(text).expect("combat_avatar.json malformed");
        let mut by_id = HashMap::new();
        let mut by_name = HashMap::new();
        let mut by_name_en = HashMap::new();
        let mut by_alias = HashMap::new();
        for (i, a) in all.iter().enumerate() {
            if !a.id.is_empty() {
                by_id.insert(a.id.clone(), i);
            }
            if !a.name.is_empty() {
                by_name.insert(a.name.clone(), i);
            }
            if !a.name_en.is_empty() {
                // 小写
                by_name_en.insert(a.name_en.clone(), i);
                by_name_en.entry(a.name_en.to_lowercase()).or_insert(i);
            }
            for alias in &a.alias {
                if let Some(prev) = by_alias.insert(alias.clone(), i) {
                    if prev != i {
                        log::warn!(
                            "combat_avatar.json: alias {alias:?} maps to both {:?} and {:?}",
                            all[prev].name,
                            a.name
                        );
                    }
                }
            }
        }
        Self {
            all,
            by_id,
            by_name,
            by_name_en,
            by_alias,
        }
    }
    /// 全部角色
    pub fn all(&self) -> &[CombatAvatar] {
        &self.all
    }
    pub fn by_id(&self, id: &str) -> Option<&CombatAvatar> {
        self.by_id.get(id).map(|&i| &self.all[i])
    }
    pub fn by_name(&self, name: &str) -> Option<&CombatAvatar> {
        self.by_name.get(name).map(|&i| &self.all[i])
    }
    pub fn by_name_en(&self, name_en: &str) -> Option<&CombatAvatar> {
        self.by_name_en
            .get(name_en)
            .or_else(||self.by_name_en.get(&name_en.to_lowercase()))
            .map(|&i|&self.all[i])
    }
    pub fn by_alias(&self, alias: &str) -> Option<&CombatAvatar> {
        self.by_alias.get(alias).map(|&i| &self.all[i])
    }
    pub fn lookup(&self, query: &str) -> Option<&CombatAvatar> {
        self.by_name(query)
            .or_else(||self.by_name_en(query))
            .or_else(||self.by_alias(query))
    }
}

/// 静态注册，首次访问时解析json
pub fn registry() -> &'static CombatAvatarRegistry {
    static REG: OnceLock<CombatAvatarRegistry> = OnceLock::new();
    REG.get_or_init(||CombatAvatarRegistry::from_json(COMBAT_AVATAR_JSON))
}
