//! 输入设备模块

mod uinput;

pub mod action;
pub mod keybindings;
pub mod keytype;
pub mod simulator;
pub mod constants;

pub use action::GIAction;
pub use keybindings::{
    InputKey, KeyBindingsConfig, KeyBindingsLoadError, MouseButton, ev_key_to_str, str_to_ev_key,
};
pub use keytype::KeyType;
pub use simulator::{Simulator, SimulatorError};
pub use uinput::{DeviceError, GIDevice, RelMouseGuardMode};
