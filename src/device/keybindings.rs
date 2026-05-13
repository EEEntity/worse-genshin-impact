//! 键位绑定: ['GIAction'] -> evdev

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use evdev_rs::enums::EV_KEY;
use serde::{Deserialize, Serialize};
use super::action::GIAction;
use super::constants::GI_KEYS;

// 鼠标按键
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    /// 转换为uinput的`EV_KEY`
    pub const fn to_ev_key(self) -> EV_KEY {
        match self {
            MouseButton::Left => EV_KEY::BTN_LEFT,
            MouseButton::Right => EV_KEY::BTN_RIGHT,
            MouseButton::Middle => EV_KEY::BTN_MIDDLE,
        }
    }
}

/// 统一输入
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputKey {
    Key(EV_KEY),
    Mouse(MouseButton),
}

impl InputKey {
    /// 转换为uinput的`EV_KEY`
    pub fn to_ev_key(self) -> EV_KEY {
        match self {
            InputKey::Key(k) => k,
            InputKey::Mouse(b) => b.to_ev_key(),
        }
    }
}

// "Key:KEY_W"/"Mouse:Left"
impl Serialize for InputKey {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let text = match self {
            InputKey::Key(k) => format!("Key:{}", ev_key_to_str(*k)),
            InputKey::Mouse(b) => format!("Mouse:{:?}", b),
        };
        s.serialize_str(&text)
    }
}

impl<'de> Deserialize<'de> for InputKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let (kind, name) = s
            .split_once(':')
            .ok_or_else(|| serde::de::Error::custom(format!("bad InputKey: {s:?}")))?;
        match kind {
            "Key" => str_to_ev_key(name)
                .map(InputKey::Key)
                .ok_or_else(|| serde::de::Error::custom(format!("unknown EV_KEY name: {name:?}"))),
            "Mouse" => match name {
                "Left" => Ok(InputKey::Mouse(MouseButton::Left)),
                "Right" => Ok(InputKey::Mouse(MouseButton::Right)),
                "Middle" => Ok(InputKey::Mouse(MouseButton::Middle)),
                _ => Err(serde::de::Error::custom(format!("unknown MouseButton: {name:?}"))),
            },
            _ => Err(serde::de::Error::custom(format!("unknown InputKey kind: {kind:?}"))),
        }
    }
}

pub fn ev_key_to_str(k: EV_KEY) -> String {
    format!("{:?}", k)
}

pub fn str_to_ev_key(name: &str) -> Option<EV_KEY> {
    GI_KEYS
        .iter()
        .copied()
        .find(|k| format!("{:?}", k) == name)
}

/// 键位映射
/// JSON形式
/// ```json
/// {
///     "MoveForward": "Key:KEY_W",
///     "NormalAttack": "Mouse:Left",
///     "SwitchMember1": "Key:KEY_1",
/// }
/// ```
/// 需要做改键时，把这个扔到config模块
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeyBindingsConfig {
    bindings: HashMap<GIAction, InputKey>,
}

impl KeyBindingsConfig {
    /// 查询[`GIAction`]当前绑定
    pub fn get(&self, action: GIAction) -> Option<InputKey> {
        self.bindings.get(&action).copied()
    }
    /// 修改/新增绑定
    pub fn set(&mut self, action: GIAction, key: InputKey) {
        self.bindings.insert(action, key);
    }
    /// 取消绑定
    pub fn unset(&mut self, action: GIAction) {
        self.bindings.remove(&action);
    }
    /// 默认配置文件路径
    /// `$HOME/.config/worse-genshin-impact/keybindings.json`
    /// `HOME`未设置时返回`./keybindings.json`
    pub fn default_path() -> PathBuf {
        match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(".config/worse-genshin-impact/keybindings.json"),
            None => PathBuf::from("./keybindings.json"),
        }
    }
    /// 从文件加载配置，失败时warn并回退默认配置
    #[deprecated(note = "TODO: 迁移到 config 模块统一管理")]
    pub fn load_or_default(path: &Path) -> Self {
        match Self::try_load(path) {
            Ok(c) => {
                log::info!("keybindings loaded from {}", path.display());
                c
            }
            Err(e) => {
                log::warn!(
                    "failed to load keybindings from {}: {e}; falling back to default",
                    path.display());
                Self::default()
            }
        }
    }
    /// 无回退加载，用于测试
    pub fn try_load(path: &Path) -> Result<Self, KeyBindingsLoadError> {
        let text = fs::read_to_string(path).map_err(KeyBindingsLoadError::Io)?;
        serde_json::from_str(&text).map_err(KeyBindingsLoadError::Parse)
    }
    /// 写入JSON文件(pretty)
    pub fn save(&self, path: &Path) -> Result<(), KeyBindingsLoadError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(KeyBindingsLoadError::Io)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(KeyBindingsLoadError::Parse)?;
        fs::write(path, text).map_err(KeyBindingsLoadError::Io)
    }
}

/// 错误类型
#[derive(Debug)]
pub enum KeyBindingsLoadError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for KeyBindingsLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyBindingsLoadError::Io(e) => write!(f, "IO error: {e}"),
            KeyBindingsLoadError::Parse(e) => write!(f, "Parse error: {e}"),
        }
    }
}
impl std::error::Error for KeyBindingsLoadError {}

// 默认键位
impl Default for KeyBindingsConfig {
    /// PC默认键位
    /// 只提供单键绑定，组合/序列在高层实现
    fn default() -> Self {
        use GIAction as A;
        use MouseButton::*;
        let k = |x|InputKey::Key(x);
        let m = |x|InputKey::Mouse(x);
        let mut b = HashMap::new();
        b.insert(A::MoveForward, k(EV_KEY::KEY_W));
        b.insert(A::MoveBackward, k(EV_KEY::KEY_S));
        b.insert(A::MoveLeft, k(EV_KEY::KEY_A));
        b.insert(A::MoveRight, k(EV_KEY::KEY_D));
        b.insert(A::NormalAttack, m(Left));
        b.insert(A::ElementalSkill, k(EV_KEY::KEY_E));
        b.insert(A::ElementalBurst, k(EV_KEY::KEY_Q));
        b.insert(A::SprintKeyboard, k(EV_KEY::KEY_LEFTSHIFT));
        b.insert(A::SprintMouse, m(Right));
        b.insert(A::SwitchToWalkOrRun, k(EV_KEY::KEY_LEFTCTRL));
        b.insert(A::SwitchAimingMode, k(EV_KEY::KEY_R));
        b.insert(A::Jump, k(EV_KEY::KEY_SPACE));
        b.insert(A::Drop, k(EV_KEY::KEY_X));
        b.insert(A::PickUpOrInteract, k(EV_KEY::KEY_F));
        b.insert(A::QuickUseGadget, k(EV_KEY::KEY_Z));
        b.insert(A::InteractionInSomeMode, k(EV_KEY::KEY_T));
        b.insert(A::QuestNavigation, k(EV_KEY::KEY_V));
        b.insert(A::AbandonChallenge, k(EV_KEY::KEY_P));
        b.insert(A::SwitchMember1, k(EV_KEY::KEY_1));
        b.insert(A::SwitchMember2, k(EV_KEY::KEY_2));
        b.insert(A::SwitchMember3, k(EV_KEY::KEY_3));
        b.insert(A::SwitchMember4, k(EV_KEY::KEY_4));
        b.insert(A::SwitchMember5, k(EV_KEY::KEY_5));
        b.insert(A::ShortcutWheel, k(EV_KEY::KEY_TAB));
        b.insert(A::OpenInventory, k(EV_KEY::KEY_B));
        b.insert(A::OpenCharacterScreen, k(EV_KEY::KEY_C));
        b.insert(A::OpenMap, k(EV_KEY::KEY_M));
        b.insert(A::OpenPaimonMenu, k(EV_KEY::KEY_ESC));
        b.insert(A::OpenTheSettingsMenu, k(EV_KEY::KEY_F6));
        b.insert(A::OpenAdventurerHandbook, k(EV_KEY::KEY_F1));
        b.insert(A::OpenCoOpScreen, k(EV_KEY::KEY_F2));
        b.insert(A::OpenSpecialEnvironmentInformation, k(EV_KEY::KEY_U));
        b.insert(A::OpenWishScreen, k(EV_KEY::KEY_F3));
        b.insert(A::OpenBattlePassScreen, k(EV_KEY::KEY_F4));
        b.insert(A::OpenTheEventsMenu, k(EV_KEY::KEY_F5));
        b.insert(A::OpenQuestMenu, k(EV_KEY::KEY_J));
        b.insert(A::OpenTheFurnishingScreen, k(EV_KEY::KEY_F7));
        b.insert(A::OpenStellarReunion, k(EV_KEY::KEY_F8));
        b.insert(A::OpenNotificationDetails, k(EV_KEY::KEY_Y));
        b.insert(A::HideUI, k(EV_KEY::KEY_SLASH));
        b.insert(A::OpenChatScreen, k(EV_KEY::KEY_ENTER));
        b.insert(A::OpenPartySetupScreen, k(EV_KEY::KEY_L));
        b.insert(A::CheckTutorialDetails, k(EV_KEY::KEY_G));
        b.insert(A::ElementalSight, m(Middle));
        b.insert(A::ShowCursor, k(EV_KEY::KEY_LEFTALT));
        b.insert(A::OpenFriendsScreen, k(EV_KEY::KEY_O));
        Self { bindings: b }
    }
}
