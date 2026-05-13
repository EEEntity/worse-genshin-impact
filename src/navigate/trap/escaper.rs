//! 在卡住时尝试脱困

use std::time::{Duration, Instant};
use evdev_rs::enums::EV_KEY;
use rand::Rng;

use crate::device::GIDevice;
use crate::navigate::error::NavigateError;

/// 可能有线程安全问题
pub struct TrapEscaper {
    /// 偏移角度
    random_offset_deg: f32,
    /// 上次动作时间
    last_action_time: Instant,
    last_action_index: i32,
    /// 偏移累计容忍时间
    pub reset_after: Duration,
    /// `move_to`单次脱困最长持续时间
    pub max_escape: Duration,
    /// `move_to`期间无动作的退出阈值
    pub idle_timeout: Duration,
    /// `move_to`迭代间隔
    pub move_iter_delay: Duration,
}

impl Default for TrapEscaper {
    fn default() -> Self {
        Self {
            random_offset_deg: 0.0,
            last_action_time: Instant::now() - Duration::from_secs(60),
            last_action_index: 0,
            reset_after: Duration::from_millis(1500),
            max_escape: Duration::from_secs(25),
            idle_timeout: Duration::from_secs(5),
            move_iter_delay: Duration::from_millis(100),
        }
    }
}

impl TrapEscaper {
    pub fn new() -> Self {
        Self::default()
    }
    /// 偏移角度
    pub fn random_offset_deg(&self) -> f32 {
        self.random_offset_deg
    }
    /// 上次动作时间
    pub fn last_action_time(&self) -> Instant {
        self.last_action_time
    }
    /// 到达`last_action_time`
    pub fn touch_action_time(&mut self) {
        self.last_action_time = Instant::now();
    }
    /// 累加30..=45度
    pub fn bump_random(&mut self) {
        let mut rng = rand::thread_rng();
        let mag: f32 = rng.gen_range(30.0..45.0);
        self.random_offset_deg += mag;
        self.random_offset_deg %= 360.0;
        self.last_action_time = Instant::now();
    }
    /// 反向累加
    pub fn reduce_random(&mut self) {
        let mut rng = rand::thread_rng();
        let mag: f32 = rng.gen_range(30.0..45.0);
        self.random_offset_deg -= mag;
        self.random_offset_deg %= 360.0;
        self.last_action_time = Instant::now();
    }
    /// 没有新偏移一段时间后归零
    pub fn maybe_reset(&mut self) {
        if self.random_offset_deg != 0.0 {
            self.random_offset_deg %= 360.0;
            if self.last_action_time.elapsed() > self.reset_after {
                self.random_offset_deg = 0.0;
            }
        }
    }
    /// 脱困流程
    pub fn rotate_and_move(&mut self, device: &GIDevice) -> Result<(), NavigateError> {
        let map_err = |e: crate::device::DeviceError| NavigateError::Device(e.to_string());
        self.bump_random();
        // 脱离攀爬状态
        device.key_up(EV_KEY::KEY_W).map_err(map_err)?;
        device
            .press_keys(&[EV_KEY::KEY_X], Duration::from_millis(40))
            .map_err(map_err)?;
        std::thread::sleep(Duration::from_millis(75));
        device
            .mouse_click(
                EV_KEY::BTN_LEFT,
                Duration::ZERO,
                Duration::from_millis(40),
                Duration::ZERO,
            )
            .map_err(map_err)?;
        std::thread::sleep(Duration::from_millis(500));
        if self.last_action_time.elapsed() >= Duration::from_secs(10) {
            self.last_action_index = 0;
        } else {
            self.last_action_index = self.last_action_index.saturating_add(1);
        }
        let difference = (self.last_action_index as i64) * 1000;
        match self.last_action_index.rem_euclid(3) {
            0 => self.move_backward_with_jump(device, 1000 + difference)?,
            1 => self.move_left_with_jump(device, 700 + difference)?,
            2 => self.move_right_with_jump(device, 700 + difference)?,
            _ => unreachable!(),
        }
        self.last_action_time = Instant::now();
        log::warn!(
            "TrapEscaper.RotateAndMove: index={}, offset={:.1}°",
            self.last_action_index,
            self.random_offset_deg
        );
        Ok(())
    }
    fn move_backward_with_jump(
        &self,
        device: &GIDevice,
        delay_ms: i64,
    ) -> Result<(), NavigateError> {
        let map_err = |e: crate::device::DeviceError| NavigateError::Device(e.to_string());
        device.key_down(EV_KEY::KEY_S).map_err(map_err)?;
        std::thread::sleep(Duration::from_millis(500));
        device
            .press_keys(&[EV_KEY::KEY_SPACE], Duration::from_millis(40))
            .map_err(map_err)?;
        std::thread::sleep(Duration::from_millis(delay_ms.max(0) as u64));
        device.key_up(EV_KEY::KEY_S).map_err(map_err)?;
        Ok(())
    }
    fn move_left_with_jump(
        &self,
        device: &GIDevice,
        delay_ms: i64,
    ) -> Result<(), NavigateError> {
        let map_err = |e: crate::device::DeviceError| NavigateError::Device(e.to_string());
        device.key_down(EV_KEY::KEY_A).map_err(map_err)?;
        std::thread::sleep(Duration::from_millis(300));
        device
            .press_keys(&[EV_KEY::KEY_SPACE], Duration::from_millis(40))
            .map_err(map_err)?;
        std::thread::sleep(Duration::from_millis(delay_ms.max(0) as u64));
        device.key_up(EV_KEY::KEY_A).map_err(map_err)?;
        device
            .press_keys(&[EV_KEY::KEY_X], Duration::from_millis(40))
            .map_err(map_err)?;
        Ok(())
    }
    fn move_right_with_jump(
        &self,
        device: &GIDevice,
        delay_ms: i64,
    ) -> Result<(), NavigateError> {
        let map_err = |e: crate::device::DeviceError| NavigateError::Device(e.to_string());
        device.key_down(EV_KEY::KEY_D).map_err(map_err)?;
        std::thread::sleep(Duration::from_millis(300));
        device
            .press_keys(&[EV_KEY::KEY_SPACE], Duration::from_millis(40))
            .map_err(map_err)?;
        std::thread::sleep(Duration::from_millis(delay_ms.max(0) as u64));
        device.key_up(EV_KEY::KEY_D).map_err(map_err)?;
        device
            .press_keys(&[EV_KEY::KEY_X], Duration::from_millis(40))
            .map_err(map_err)?;
        Ok(())
    }
    /// 攀爬时脱困
    pub fn climb_recover(&mut self, device: &GIDevice) -> Result<(), NavigateError> {
        let map_err = |e: crate::device::DeviceError| NavigateError::Device(e.to_string());
        device.key_up(EV_KEY::KEY_W).map_err(map_err)?;
        device
            .press_keys(&[EV_KEY::KEY_X], Duration::from_millis(40))
            .map_err(map_err)?;
        std::thread::sleep(Duration::from_millis(75));
        device.key_down(EV_KEY::KEY_S).map_err(map_err)?;
        std::thread::sleep(Duration::from_millis(700));
        device.key_up(EV_KEY::KEY_S).map_err(map_err)?;
        self.bump_random();
        Ok(())
    }
}
