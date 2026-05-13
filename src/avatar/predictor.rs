//! 队伍角色识别接口
//! WIP: 接yolo模型

use std::path::Path;
use std::sync::Mutex;
use opencv::core::Mat;

use crate::config::combat_avatar::{CombatAvatar, registry};

/// 从侧边头像截图识别角色
pub trait AvatarPredictor: Send + Sync {
    fn predict(&self, icon: &Mat, slot: u8) -> Option<&'static CombatAvatar>;
}

/// 预配置来源队伍名称
pub struct ConfigPredictor {
    names: Vec<String>,
}

impl ConfigPredictor {
    /// 用`,`/`;`/` `分隔字符串构造
    pub fn from_team_names(team_names: &str) -> Self {
        let names = team_names
            .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        Self { names }
    }
    /// 直接给定名字列表
    pub fn from_list(names: Vec<String>) -> Self {
        Self { names }
    }
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

impl AvatarPredictor for ConfigPredictor {
    fn predict(&self, _icon: &Mat, slot: u8) -> Option<&'static CombatAvatar> {
        let i = slot.checked_sub(1)? as usize;
        let name = self.names.get(i)?;
        registry().lookup(name)
    }
}

/// 占位
pub struct StubPredictor;

impl AvatarPredictor for StubPredictor {
    fn predict(&self, _icon: &Mat, _slot: u8) -> Option<&'static CombatAvatar> {
        None
    }
}

/// 角色识别
pub struct OrtAvatarPredictor {
    classifier: Mutex<crate::inference::yolo::classifier::OrtClassifier>,
}

impl OrtAvatarPredictor {
    pub fn load_default() -> anyhow::Result<Self> {
        Self::load(crate::inference::model::Model::AvatarSide.model_path())
    }
    pub fn load(model_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        use crate::inference::session::{OrtSession, SessionOptions};
        use crate::inference::yolo::predictor::read_labels_from_metadata;
        let mut session = OrtSession::with_options(
            model_path.as_ref(),
            &SessionOptions::cpu(),
        )?;
        let labels = read_labels_from_metadata(&mut session)?;
        if labels.is_empty() {
            anyhow::bail!(
                "BgiAvatarSide 模型 {} 没有 names metadata",
                model_path.as_ref().display()
            );
        }
        let classifier = crate::inference::yolo::classifier::OrtClassifier::load_auto(
            model_path.as_ref(),
            labels,
        )?;
        drop(session);
        Ok(Self { classifier: Mutex::new(classifier) })
    }
}

impl AvatarPredictor for OrtAvatarPredictor {
    fn predict(&self, icon: &Mat, slot: u8) -> Option<&'static CombatAvatar> {
        let mut cls = self.classifier.lock().ok()?;
        let res = match cls.classify(icon) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("第 {slot} 位侧边头像分类失败：{e:#}");
                return None;
            }
        };
        let label = res.label.as_str();
        let threshold = if label.starts_with("Qin") || label.contains("Costume") {
            0.51
        } else {
            0.70
        };
        if (res.score as f64) < threshold {
            log::warn!(
                "第 {slot} 位角色识别置信度过低：{} ({:.2})",
                label,
                res.score
            );
            return None;
        }
        // 剥掉"Costume*"后缀
        let name_en = match label.find("Costume") {
            Some(i) => &label[..i],
            None => label,
        };
        match registry().by_name_en(name_en) {
            Some(a) => {
                log::debug!(
                    "第 {slot} 位角色识别：{} -> {} (conf={:.2})",
                    label, a.name, res.score
                );
                Some(a)
            }
            None => {
                log::warn!("识别到未知 nameEn={name_en:?} (label={label})");
                None
            }
        }
    }
}
