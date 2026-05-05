//! `KeyType`: 按键操作类型

use serde::{Deserialize, Serialize};
/// 通过该枚举区分四种操作：
/// - [`KeyType::KeyPress`]：按下并立即（约 40ms）松开
/// - [`KeyType::KeyDown`]：仅按下，不松开
/// - [`KeyType::KeyUp`]：仅松开
/// - [`KeyType::Hold`]：按下并保持 [`crate::device::constants::HOLD_DURATION_MS`] 后松开
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyType {
    KeyPress,
    KeyDown,
    KeyUp,
    Hold,
}
