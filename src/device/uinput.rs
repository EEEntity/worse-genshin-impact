//! 虚拟输入设备封装
//! 基于`uinput`注入输入事件

use std::sync::atomic::{AtomicI32, Ordering};
use std::thread;
use std::time::Duration;
use evdev_rs::{
    enums::{EventCode, EV_ABS, EV_KEY, EV_REL, EV_SYN},
    AbsInfo, DeviceWrapper, EnableCodeData, InputEvent, TimeVal, UInputDevice, UninitDevice,
};
use super::constants::{
    DEVICE_INIT_DELAY, DEVICE_NAME, DEVICE_PHYS, GI_ABS_AXES, GI_KEYS, GI_REL_AXES,
};

#[derive(Debug)]
pub enum DeviceError {
    Init(String),
    Write(String),
}

/// 鼠标指针REL越界保护
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelMouseGuardMode {
    /// ABS重锚定
    Recenter,
    /// 软限制
    Clamp,
}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceError::Init(msg) => write!(f, "[DeviceError::Init] {msg}"),
            DeviceError::Write(msg) => write!(f, "[DeviceError::Write] {msg}"),
        }
    }
}

impl std::error::Error for DeviceError {}

pub struct GIDevice {
    uinput: UInputDevice,
    /// 窗口在桌面上的左上角偏移(桌面坐标-窗口坐标)
    /// 所有传入[`Self::teleport_mouse`]的坐标应该以游戏窗口左上角为远点
    /// 写入uinput ABS事件时需要加上偏移量
    /// 默认(0,0)
    screen_offset: (i32, i32),
    /// 游戏窗口尺寸
    /// `(0,0)`表示未设置 -> 关闭
    /// [`Self::mouse_move_rel`]的越界保护
    /// 启动时根据捕获分辨率调用[`Self::set_window_size`]注入
    window_size: (i32, i32),
    /// 鼠标指针REL越界保护
    rel_guard_mode: RelMouseGuardMode,
    /// 模拟跟踪的桌面鼠标指针绝对位置
    /// 由[`Self::teleport_mouse`]设置，[`Self::mouse_move_rel`]累加
    /// 用于在REL事件将光标推出游戏窗口前ABS重锚至窗口中心
    /// Windows有cursor lock(?)，但KDE下测试时只能限制一下了
    cursor_x: AtomicI32,
    cursor_y: AtomicI32,
}

impl GIDevice {
    fn init(delay: Duration) -> Result<Self, DeviceError> {
        let u = UninitDevice::new()
            .ok_or_else(||DeviceError::Init("UninitDevice::new() returned None".to_string()))?;
        u.set_name(DEVICE_NAME);
        u.set_phys(DEVICE_PHYS);
        for &key in GI_KEYS {
            u.enable(EventCode::EV_KEY(key))
                .map_err(|e|DeviceError::Init(format!("enable {key:?}: {e}")))?;
        }
        for &rel in GI_REL_AXES {
            u.enable(EventCode::EV_REL(rel))
                .map_err(|e|DeviceError::Init(format!("enable {rel:?}: {e}")))?;
        }
        for &(axis, min, max) in GI_ABS_AXES {
            u.enable_event_code(
                &EventCode::EV_ABS(axis),
                Some(EnableCodeData::AbsInfo(AbsInfo {
                    value: 0,
                    minimum: min,
                    maximum: max,
                    fuzz: 0,
                    flat: 0,
                    resolution: 0,
                })),
            )
            .map_err(|e|DeviceError::Init(format!("enable {axis:?}: {e}")))?;
        }
        u.enable(EventCode::EV_SYN(EV_SYN::SYN_REPORT))
            .map_err(|e| DeviceError::Init(format!("enable SYN_REPORT: {e}")))?;
        let uinput = UInputDevice::create_from_device(&u)
            .map_err(|e| DeviceError::Init(format!("create uinput device: {e}")))?;
        if !delay.is_zero() {
            thread::sleep(delay);
        }
        Ok(Self {
            uinput,
            screen_offset: (0, 0),
            window_size: (0, 0),
            rel_guard_mode: RelMouseGuardMode::Recenter,
            cursor_x: AtomicI32::new(0),
            cursor_y: AtomicI32::new(0),
        })
    }
    /// 创建虚拟输入设备
    pub fn new() -> Result<Self, DeviceError> {
        Self::init(DEVICE_INIT_DELAY)
    }
    pub fn devnode(&self) -> Option<&str> {
        self.uinput.devnode()
    }
    /// 设置游戏窗口偏移量
    pub fn set_screen_offset(&mut self, offset_x: i32, offset_y: i32) {
        self.screen_offset = (offset_x, offset_y);
    }
    /// 返回当前ABS_X和ABS_Y最大值
    /// 用于计算居中窗口的偏移量
    pub fn abs_max() -> (i32, i32) {
        let mut x = 0;
        let mut y = 0;
        for &(axis, _min, max) in GI_ABS_AXES {
            match axis {
                EV_ABS::ABS_X => x = max,
                EV_ABS::ABS_Y => y = max,
                _ => {}
            }
        }
        (x, y)
    }
    /// 返回当前生效的窗口偏移量
    pub fn screen_offset(&self) -> (i32, i32) {
        self.screen_offset
    }
    /// 设置游戏窗口尺寸数值
    pub fn set_window_size(&mut self, w: i32, h: i32) {
        self.window_size = (w, h);
    }
    /// 设置鼠标指针REL越界保护策略
    pub fn set_rel_mouse_guard_mode(&mut self, mode: RelMouseGuardMode) {
        self.rel_guard_mode = mode;
    }
    /// 返回当前已配置的游戏窗口尺寸数值
    pub fn window_size(&self) -> (i32, i32) {
        self.window_size
    }
    /// 鼠标指针传送回窗口中心
    /// 未设置[`Self::set_window_size`]时无操作
    pub fn center_cursor(&self) -> Result<(), DeviceError> {
        let (w, h) = self.window_size;
        if w <= 0 || h <= 0 {
            return Ok(());
        }
        self.teleport_mouse(w / 2, h / 2)
    }
    /// 拖动越界保护
    /// `(sx,sy) -> (ex,ey)`应该完全位于游戏窗口内
    fn fit_drag_inside_window(
        &self,
        sx: i32,
        sy: i32,
        ex: i32,
        ey: i32,
    ) -> (i32, i32, i32, i32) {
        const MARGIN: i32 = 10;
        let (w, h) = self.window_size;
        if w <= 0 || h <= 0 {
            return (sx, sy, ex, ey);
        }
        fn fit_axis(s: i32, e: i32, lo: i32, hi: i32) -> (i32, i32) {
            let lo_se = s.min(e);
            let hi_se = s.max(e);
            let shift = if lo_se < lo {
                lo - lo_se
            } else if hi_se > hi {
                hi - hi_se
            } else {
                0
            };
            let s2 = s + shift;
            let e2 = e + shift;
            (s2.clamp(lo, hi), e2.clamp(lo, hi))
        }
        let (sx2, ex2) = fit_axis(sx, ex, MARGIN, w - MARGIN);
        let (sy2, ey2) = fit_axis(sy, ey, MARGIN, h - MARGIN);
        (sx2, sy2, ex2, ey2)
    }

    // 内部事件操作
    fn write(&self, code: EventCode, value: i32) -> Result<(), DeviceError> {
        self.uinput
            .write_event(&InputEvent {
                time: TimeVal { tv_sec: 0, tv_usec: 0 },
                event_code: code,
                value,
            })
            .map_err(|e| DeviceError::Write(format!("{code:?} = {value}: {e}")))
    }
    fn syn(&self) -> Result<(), DeviceError> {
        self.write(EventCode::EV_SYN(EV_SYN::SYN_REPORT), 0)
    }

    // 公开操作
    /// 同时按下一组键，等待 delay 后松开
    pub fn press_keys(&self, keys: &[EV_KEY], delay: Duration) -> Result<(), DeviceError> {
        for &key in keys {
            self.write(EventCode::EV_KEY(key), 1)?;
        }
        self.syn()?;
        if !delay.is_zero() {
            thread::sleep(delay);
        }
        for &key in keys {
            self.write(EventCode::EV_KEY(key), 0)?;
        }
        self.syn()?;
        Ok(())
    }
    /// 通过ABS事件调整鼠标指针坐标
    /// `(x,y)`以窗口左上角为原点，需要加上[`Self::screen_offset`]
    pub fn teleport_mouse(&self, x: i32, y: i32) -> Result<(), DeviceError> {
        let (ox, oy) = self.screen_offset;
        let abs_x = x + ox;
        let abs_y = y + oy;
        self.write(EventCode::EV_ABS(EV_ABS::ABS_X), abs_x)?;
        self.write(EventCode::EV_ABS(EV_ABS::ABS_Y), abs_y)?;
        self.syn()?;
        self.cursor_x.store(abs_x, Ordering::Relaxed);
        self.cursor_y.store(abs_y, Ordering::Relaxed);
        Ok(())
    }
    /// 鼠标按键
    /// - `button`: `EV_KEY::BTN_LEFT`/`BTN_RIGHT`/`BTN_MIDDLE`
    /// - `pre_delay`: 按下前等待
    /// - `duration`: 按住时长
    /// - `post_delay`: 松开后等待
    pub fn mouse_click(
        &self,
        button: EV_KEY,
        pre_delay: Duration,
        duration: Duration,
        post_delay: Duration,
    ) -> Result<(), DeviceError> {
        if !pre_delay.is_zero() {
            thread::sleep(pre_delay);
        }
        self.write(EventCode::EV_KEY(button), 1)?;
        self.syn()?;
        if !duration.is_zero() {
            thread::sleep(duration);
        }
        self.write(EventCode::EV_KEY(button), 0)?;
        self.syn()?;
        if !post_delay.is_zero() {
            thread::sleep(post_delay);
        }
        Ok(())
    }
    /// 移动鼠标指针(累积差分REL)
    pub fn mouse_move(
        &self,
        delta_x: i32,
        delta_y: i32,
        duration: Duration,
        steps: u32,
    ) -> Result<(), DeviceError> {
        let steps = steps.max(1) as i32;
        let step_delay = if duration.is_zero() {
            Duration::ZERO
        } else {
            duration / steps as u32
        };
        for step in 1..=steps {
            let raw_x = delta_x * step / steps - delta_x * (step - 1) / steps;
            let raw_y = delta_y * step / steps - delta_y * (step - 1) / steps;
            let (inc_x, inc_y) = self.guard_rel_delta(raw_x, raw_y)?;
            self.write(EventCode::EV_REL(EV_REL::REL_X), inc_x)?;
            self.write(EventCode::EV_REL(EV_REL::REL_Y), inc_y)?;
            self.syn()?;
            self.cursor_x.fetch_add(inc_x, Ordering::Relaxed);
            self.cursor_y.fetch_add(inc_y, Ordering::Relaxed);
            if !step_delay.is_zero() {
                thread::sleep(step_delay);
            }
        }
        Ok(())
    }
    /// 按住左键，通过ABS事件拖拽
    pub fn mouse_drag(
        &self,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        duration: Duration,
        steps: u32,
    ) -> Result<(), DeviceError> {
        // Linux+Wine 防越界：见 [`Self::fit_drag_inside_window`]
        let (start_x, start_y, end_x, end_y) =
            self.fit_drag_inside_window(start_x, start_y, end_x, end_y);
        self.teleport_mouse(start_x, start_y)?;
        self.write(EventCode::EV_KEY(EV_KEY::BTN_LEFT), 1)?;
        self.syn()?;
        let steps = steps.max(1) as i32;
        let step_delay = if duration.is_zero() {
            Duration::ZERO
        } else {
            duration / steps as u32
        };
        for step in 1..=steps {
            let x = start_x + (end_x - start_x) * step / steps;
            let y = start_y + (end_y - start_y) * step / steps;
            self.teleport_mouse(x, y)?;
            if !step_delay.is_zero() {
                thread::sleep(step_delay);
            }
        }
        self.write(EventCode::EV_KEY(EV_KEY::BTN_LEFT), 0)?;
        self.syn()?;
        Ok(())
    }
    /// 滚动鼠标滚轮
    /// `delta`向上为正
    pub fn mouse_scroll(&self, delta: i32) -> Result<(), DeviceError> {
        self.write(EventCode::EV_REL(EV_REL::REL_WHEEL), delta)?;
        self.syn()?;
        Ok(())
    }

    // 按键原语
    /// 按下按键
    pub fn key_down(&self, key: EV_KEY) -> Result<(), DeviceError> {
        self.write(EventCode::EV_KEY(key), 1)?;
        self.syn()
    }
    /// 松开按键
    pub fn key_up(&self, key: EV_KEY) -> Result<(), DeviceError> {
        self.write(EventCode::EV_KEY(key), 0)?;
        self.syn()
    }
    /// 按住鼠标按键(应该合并?)
    pub fn mouse_button_down(&self, button: EV_KEY) -> Result<(), DeviceError> {
        self.write(EventCode::EV_KEY(button), 1)?;
        self.syn()
    }
    /// 松开鼠标按键
    pub fn mouse_button_up(&self, button: EV_KEY) -> Result<(), DeviceError> {
        self.write(EventCode::EV_KEY(button), 0)?;
        self.syn()
    }
    /// 单步鼠标移动，用于高频触发视角控制
    pub fn mouse_move_rel(&self, dx: i32, dy: i32) -> Result<(), DeviceError> {
        if dx == 0 && dy == 0 {
            return Ok(());
        }
        let (adj_x, adj_y) = self.guard_rel_delta(dx, dy)?;
        if adj_x != 0 {
            self.write(EventCode::EV_REL(EV_REL::REL_X), adj_x)?;
        }
        if adj_y != 0 {
            self.write(EventCode::EV_REL(EV_REL::REL_Y), adj_y)?;
        }
        self.syn()?;
        self.cursor_x.fetch_add(adj_x, Ordering::Relaxed);
        self.cursor_y.fetch_add(adj_y, Ordering::Relaxed);
        Ok(())
    }
    /// 根据当前[`RelMouseGuardMode`]计算可写入REL增量
    fn guard_rel_delta(&self, dx: i32, dy: i32) -> Result<(i32, i32), DeviceError> {
        match self.rel_guard_mode {
            RelMouseGuardMode::Recenter => {
                self.guard_cursor_inside_window(dx, dy)?;
                Ok((dx, dy))
            }
            RelMouseGuardMode::Clamp => Ok(self.clamp_rel_inside_window(dx, dy)),
        }
    }
    /// 鼠标指针REL越界时，调整回窗口中心
    /// 必须先设置[`Self::set_window_size`]
    fn guard_cursor_inside_window(&self, dx: i32, dy: i32) -> Result<(), DeviceError> {
        let (w, h) = self.window_size;
        if w <= 0 || h <= 0 {
            return Ok(());
        }
        let (ox, oy) = self.screen_offset;
        let cx = self.cursor_x.load(Ordering::Relaxed);
        let cy = self.cursor_y.load(Ordering::Relaxed);
        let next_wx = cx - ox + dx;
        let next_wy = cy - oy + dy;
        let margin = 100;
        if next_wx < margin
            || next_wx > w - margin
            || next_wy < margin
            || next_wy > h - margin
        {
            self.teleport_mouse(w / 2, h / 2)?;
        }
        Ok(())
    }
    /// REL增量软限制
    /// 必须先设置[`Self::set_window_size`]
    fn clamp_rel_inside_window(&self, dx: i32, dy: i32) -> (i32, i32) {
        let (w, h) = self.window_size;
        if w <= 0 || h <= 0 {
            return (dx, dy);
        }
        let (ox, oy) = self.screen_offset;
        let cx = self.cursor_x.load(Ordering::Relaxed);
        let cy = self.cursor_y.load(Ordering::Relaxed);
        let wx = cx - ox;
        let wy = cy - oy;
        // 留 100 px 边距，避免贴边触发游戏菜单或 cursor leave 事件
        let margin = 100;
        let min_x = margin;
        let max_x = (w - margin).max(min_x);
        let min_y = margin;
        let max_y = (h - margin).max(min_y);

        let mut adj_x = dx;
        let mut adj_y = dy;

        let next_x = wx + adj_x;
        if next_x < min_x {
            adj_x = min_x - wx;
        } else if next_x > max_x {
            adj_x = max_x - wx;
        }

        let next_y = wy + adj_y;
        if next_y < min_y {
            adj_y = min_y - wy;
        } else if next_y > max_y {
            adj_y = max_y - wy;
        }

        (adj_x, adj_y)
    }
}
