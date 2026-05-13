//! 键位绑定
//! 
//! 也许很久之后会做改建功能吧

use std::path::PathBuf;

use crate::device::KeyBindingsConfig;

pub fn keybindings_path() -> PathBuf {
    KeyBindingsConfig::default_path()
}
