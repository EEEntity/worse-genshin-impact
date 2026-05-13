//! [`Simulator`]: 输入模拟
//! - 接受语义化[`GIAction`]
//! - 跟踪当前按下的按键，`release_all_keys()`一次性释放
//! - 异步API

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use evdev_rs::enums::EV_KEY;
use super::action::GIAction;
use super::keybindings::{InputKey, KeyBindingsConfig, MouseButton};
use super::keytype::KeyType;
use super::constants::{HOLD_DURATION, KEY_PRESS_DURATION};
use super::uinput::{DeviceError, GIDevice};

// 错误
#[derive(Debug)]
pub enum SimulatorError {
    /// `GIAction`未在[`KeyBindingsConfig`]绑定
    Unbound(GIAction),
    /// `uinput`写入失败
    Device(DeviceError),
}

impl std::fmt::Display for SimulatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimulatorError::Unbound(a) => write!(f, "GIAction {a:?} is not bound to any key"),
            SimulatorError::Device(e) => write!(f, "device: {e}"),
        }
    }
}
impl std::error::Error for SimulatorError {}
impl From<DeviceError> for SimulatorError {
    fn from(e: DeviceError) -> Self {
        SimulatorError::Device(e)
    }
}

/// 输入模拟器
pub struct Simulator {
    device: Arc<GIDevice>,
    bindings: KeyBindingsConfig,
    held: Mutex<HashSet<EV_KEY>>,
}

impl Simulator {
    /// 用给定设备和按键绑定构造
    pub fn new(device: Arc<GIDevice>, bindings: KeyBindingsConfig) -> Self {
        Self {
            device,
            bindings: bindings,
            held: Mutex::new(HashSet::new()),
        }
    }
    /// 默认配置
    pub fn with_default_bindings(device: Arc<GIDevice>) -> Self {
        Self::new(device, KeyBindingsConfig::default())
    }
    /// 直接访问设备
    pub fn device(&self) -> &Arc<GIDevice> {
        &self.device
    }
    /// 当前按键配置
    pub fn bindings(&self) -> &KeyBindingsConfig {
        &self.bindings
    }
    /// 替换键位(热更新)
    pub fn set_bindings(&mut self, bindings: KeyBindingsConfig) {
        self.bindings = bindings;
    }
    /// 模拟一个[`GIAction`]操作
    pub async fn simulate(&self, action: GIAction, kt: KeyType) -> Result<(), SimulatorError> {
        match kt {
            KeyType::KeyDown => self.do_down(action),
            KeyType::KeyUp => self.do_up(action),
            KeyType::KeyPress => {
                self.do_down(action)?;
                tokio::time::sleep(KEY_PRESS_DURATION).await;
                self.do_up(action)
            }
            KeyType::Hold => {
                self.do_down(action)?;
                tokio::time::sleep(HOLD_DURATION).await;
                self.do_up(action)
            }
        }
    }
    /// 按住并持续
    pub async fn hold_for(
        &self,
        action: GIAction,
        duration: Duration,
    ) -> Result<(), SimulatorError> {
        self.do_down(action)?;
        tokio::time::sleep(duration).await;
        self.do_up(action)
    }
    /// 鼠标点击
    pub async fn mouse_click(&self, btn: MouseButton) -> Result<(), SimulatorError> {
        self.mouse_down(btn)?;
        tokio::time::sleep(KEY_PRESS_DURATION).await;
        self.mouse_up(btn)
    }
    /// 鼠标按下不松开
    pub fn mouse_down(&self, btn: MouseButton) -> Result<(), SimulatorError> {
        let ev = btn.to_ev_key();
        self.device.mouse_button_down(ev)?;
        self.held.lock().expect("held lock poisoned").insert(ev);
        Ok(())
    }
    /// 鼠标松开
    pub fn mouse_up(&self, btn: MouseButton) -> Result<(), SimulatorError> {
        let ev = btn.to_ev_key();
        self.device.mouse_button_up(ev)?;
        self.held.lock().expect("held lock poisoned").remove(&ev);
        Ok(())
    }
    /// 鼠标点击
    pub fn left_button_click(&self) -> Result<(), SimulatorError> {
        self.device.mouse_click(
            EV_KEY::BTN_LEFT,
            Duration::ZERO,
            KEY_PRESS_DURATION,
            Duration::ZERO,
        )?;
        Ok(())
    }
    pub fn left_button_down(&self) -> Result<(), SimulatorError> {
        self.mouse_down(MouseButton::Left)
    }
    pub fn left_button_up(&self) -> Result<(), SimulatorError> {
        self.mouse_up(MouseButton::Left)
    }
    pub fn right_button_down(&self) -> Result<(), SimulatorError> {
        self.mouse_down(MouseButton::Right)
    }
    pub fn right_button_up(&self) -> Result<(), SimulatorError> {
        self.mouse_up(MouseButton::Right)
    }
    /// 中键点击，可以回正视角
    pub fn middle_button_click(&self) -> Result<(), SimulatorError> {
        self.device.mouse_click(
            EV_KEY::BTN_MIDDLE,
            Duration::ZERO,
            KEY_PRESS_DURATION,
            Duration::ZERO,
        )?;
        Ok(())
    }
    /// 鼠标相对移动
    pub fn move_mouse_by(&self, dx: i32, dy: i32) -> Result<(), SimulatorError> {
        self.device.mouse_move_rel(dx, dy)?;
        Ok(())
    }
    /// 鼠标滚轮
    pub fn scroll(&self, dy: i32) -> Result<(), SimulatorError> {
        self.device.mouse_scroll(dy)?;
        Ok(())
    }
    /// 一次性释放所有按键
    pub fn release_all_keys(&self) {
        // 持锁取出列表后立即释放
        let keys: Vec<EV_KEY> = {
            let mut held = self.held.lock().expect("held mutex poisoned");
            let v = held.iter().copied().collect();
            held.clear();
            v
        };
        for k in keys {
            if let Err(e) = self.device.key_up(k) {
                log::warn!("release_all_keys: key_up({k:?}) failed: {e}");
            }
        }
    }
    /// 同步按键，跳过async
    pub fn key_down(&self, action: GIAction) -> Result<(), SimulatorError> {
        self.do_down(action)
    }
    /// 同步松开，配合[`Simulator::key_down`]使用
    pub fn key_up(&self, action: GIAction) -> Result<(), SimulatorError> {
        self.do_up(action)
    }
    /// 同步按一次，等价`simulate(action, KeyPress)`但不await
    pub fn key_press(&self, action: GIAction) -> Result<(), SimulatorError> {
        self.do_down(action)?;
        std::thread::sleep(KEY_PRESS_DURATION);
        self.do_up(action)
    }
    fn do_down(&self, action: GIAction) -> Result<(), SimulatorError> {
        let key = self.lookup(action)?;
        let ev = key.to_ev_key();
        self.device.key_down(ev)?;
        self.held.lock().expect("held mutex poisoned").insert(ev);
        Ok(())
    }
    fn do_up(&self, action: GIAction) -> Result<(), SimulatorError> {
        let key = self.lookup(action)?;
        let ev = key.to_ev_key();
        self.device.key_up(ev)?;
        self.held.lock().expect("held mutex poisoned").remove(&ev);
        Ok(())
    }
    fn lookup(&self, action: GIAction) -> Result<InputKey, SimulatorError> {
        self.bindings
            .get(action)
            .ok_or(SimulatorError::Unbound(action))
    }
}

/// 自动释放
impl Drop for Simulator {
    fn drop(&mut self) {
        self.release_all_keys();
    }
}
