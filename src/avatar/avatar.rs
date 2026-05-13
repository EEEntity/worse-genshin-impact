//! 单个角色状态/动作执行

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use opencv::core::Rect;

use crate::config::combat_avatar::CombatAvatar;
use crate::device::action::GIAction;
use crate::device::keybindings::MouseButton;
use crate::device::keytype::KeyType;
use crate::device::simulator::{Simulator, SimulatorError};
use crate::device::constants::{ATTACK_INTERVAL_MS, SWITCH_AVATAR_WAIT_MS};

/// `Avatar`操作失败
#[derive(Debug)]
pub enum AvatarError {
    /// 操作被取消
    Cancelled,
    /// 底层[`Simulator`]失败
    Simulator(SimulatorError),
    /// 切人重试达上限仍未成功
    SwitchFailed { name: String, target_index: u8 },
}

impl std::fmt::Display for AvatarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AvatarError::Cancelled => write!(f, "cancelled"),
            AvatarError::Simulator(e) => write!(f, "simulator: {e}"),
            AvatarError::SwitchFailed { name, target_index } => {
                write!(f, "switch to {name}(idx={target_index}) failed")
            }
        }
    }
}

impl std::error::Error for AvatarError {}

impl From<SimulatorError> for AvatarError {
    fn from(e: SimulatorError) -> Self {
        AvatarError::Simulator(e)
    }
}

/// 取消信号，在异常路径调用[`AtomicBool::store`]
pub type CancelFlag = Arc<AtomicBool>;

/// 新建一个未取消的标记
pub fn cancel_flag() -> CancelFlag {
    Arc::new(AtomicBool::new(false))
}

/// 角色行走方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkDir {
    W,
    A,
    S,
    D,
}

impl WalkDir {
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "w" | "W" => Some(Self::W),
            "a" | "A" => Some(Self::A),
            "s" | "S" => Some(Self::S),
            "d" | "D" => Some(Self::D),
            _ => None,
        }
    }

    pub fn to_action(self) -> GIAction {
        match self {
            Self::W => GIAction::MoveForward,
            Self::S => GIAction::MoveBackward,
            Self::A => GIAction::MoveLeft,
            Self::D => GIAction::MoveRight,
        }
    }
}

/// 队伍中单个角色
/// 和CombatAvatar有点重复
pub struct Avatar {
    /// 静态信息
    pub combat: &'static CombatAvatar,
    /// 队伍内序号
    pub index: u8, // 1..=4有时1..=5
    /// 角色名所在矩形
    pub name_rect: Rect,
    /// 角色名右侧编号块矩形
    pub index_rect: Rect,
    /// 手动配置的E技能CD，<0时换成OCR自动识别
    pub manual_skill_cd: f64,
    /// 最近一次执行E时间
    last_skill_time: Mutex<Option<Instant>>,
    /// 最近一次OCR推算出CD时间(自动模式下)
    ocr_skill_cd: Mutex<Option<Instant>>,
    sim: Arc<Simulator>,
    cancel: CancelFlag,
}

impl Avatar {
    pub fn new(
        combat: &'static CombatAvatar,
        index: u8,
        name_rect: Rect,
        index_rect: Rect,
        manual_skill_cd: f64,
        sim: Arc<Simulator>,
        cancel: CancelFlag,
    ) -> Self {
        Self {
            combat,
            index,
            name_rect,
            index_rect,
            manual_skill_cd,
            last_skill_time: Mutex::new(None),
            ocr_skill_cd: Mutex::new(None),
            sim,
            cancel,
        }
    }
    /// 中文名访问
    pub fn name(&self) -> &str {
        &self.combat.name
    }
    /// 是否被要求取消
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    fn check_cancel(&self) -> Result<(), AvatarError> {
        if self.is_cancelled() {
            Err(AvatarError::Cancelled)
        } else {
            Ok(())
        }
    }
    async fn sleep_ct(&self, ms: u64) -> Result<(), AvatarError> {
        if self.is_cancelled() {
            return Err(AvatarError::Cancelled);
        }
        tokio::time::sleep(Duration::from_millis(ms)).await;
        Ok(())
    }
    /// 不响应取消的sleep
    async fn sleep_uninterruptible(&self, ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }
    /// 切人
    pub async fn switch<F>(
        &self,
        fight_status: bool,
        mut get_active_index: F,
    ) -> Result<(), AvatarError>
    where
        F: FnMut() -> Option<u8>,
    {
        for i in 0..30 {
            self.check_cancel()?;
            if get_active_index() == Some(self.index) {
                return Ok(());
            }
            self.simulate_switch_action()?;
            if i == 10 && fight_status {
                self.perform_unstuck().await?;
            }
            self.sleep_ct(SWITCH_AVATAR_WAIT_MS).await?;
        }
        Err(AvatarError::SwitchFailed {
            name: self.name().to_string(),
            target_index: self.index,
        })
    }
    /// 尝试切换
    pub async fn try_switch<F>(
        &self,
        try_times: u32,
        fight_status: bool,
        mut get_active_index: F,
    ) -> Result<bool, AvatarError>
    where
        F: FnMut() -> Option<u8>,
    {
        for i in 0..try_times {
            self.check_cancel()?;
            if get_active_index() == Some(self.index) {
                return Ok(true);
            }
            if i == 9 && fight_status {
                self.perform_unstuck().await?;
            }
            self.simulate_switch_action()?;
            self.sleep_ct(SWITCH_AVATAR_WAIT_MS).await?;
        }
        log::warn!("切换角色失败: {}", self.name());
        Ok(false)
    }
    fn simulate_switch_action(&self) -> Result<(), SimulatorError> {
        self.sim.key_press(GIAction::Drop)?;
        if let Some(act) = GIAction::switch_member_for(self.index) {
            self.sim.key_press(act)?;
        }
        Ok(())
    }
    /// 切人时脱困: 跳 -> 随机移动 -> 切换 -> A -> A
    pub async fn perform_unstuck(&self) -> Result<(), AvatarError> {
        use rand::Rng;
        let dirs = [
            GIAction::MoveForward,
            GIAction::MoveBackward,
            GIAction::MoveLeft,
            GIAction::MoveRight,
        ];
        let dir = dirs[rand::thread_rng().gen_range(0..dirs.len())];
        log::warn!("切换角色卡住，执行脱困（方向：{dir:?}）");
        self.sim.key_press(GIAction::Jump)?;
        self.sleep_ct(200).await?;
        self.sim.key_down(dir)?;
        if let Some(act) = GIAction::switch_member_for(self.index) {
            self.sim.key_press(act)?;
        }
        self.sleep_ct(1000).await?;
        self.sim.key_press(GIAction::NormalAttack)?;
        self.sim.release_all_keys();
        Ok(())
    }
    /// 平A
    pub async fn attack(&self, ms: i32) -> Result<(), AvatarError> {
        let mut left = ms;
        loop {
            self.check_cancel()?;
            self.sim.key_press(GIAction::NormalAttack)?;
            if left <= 0 {
                return Ok(());
            }
            left -= ATTACK_INTERVAL_MS as i32 * 2;
            self.sleep_ct(200).await?;
        }
    }
    /// E技能
    pub async fn use_skill(&self, hold: bool) -> Result<(), AvatarError> {
        self.check_cancel()?;
        if hold {
            // TODO: 纳西妲长按+鼠标sweep；坎蒂丝固定3s长按
            self.sim.simulate(GIAction::ElementalSkill, KeyType::Hold).await?;
        } else {
            self.sim.key_press(GIAction::ElementalSkill)?;
        }
        self.sleep_ct(200).await?;
        // 标记最近一次释放时间
        *self.last_skill_time.lock().unwrap() = Some(Instant::now());
        Ok(())
    }
    /// Q技能
    pub async fn use_burst(&self) -> Result<(), AvatarError> {
        self.check_cancel()?;
        self.sim.key_press(GIAction::ElementalBurst)?;
        self.sleep_ct(200).await?;
        self.sleep_ct(1500).await // 播片
    }
    /// 重击
    pub async fn charge(&self, ms: i32) -> Result<(), AvatarError> {
        self.check_cancel()?;
        let dur = if ms == 0 { 1000 } else { ms as u64 };
        // TODO: 那维莱特/恰斯卡的鼠标sweep模式
        self.sim.key_down(GIAction::NormalAttack)?;
        self.sleep_uninterruptible(dur).await;
        self.sim.key_up(GIAction::NormalAttack)?;
        Ok(())
    }
    /// 冲刺
    pub async fn dash(&self, ms: i32) -> Result<(), AvatarError> {
        self.check_cancel()?;
        let dur = if ms == 0 { 200 } else { ms as u64 };
        self.sim.key_down(GIAction::SprintMouse)?;
        self.sleep_uninterruptible(dur).await;
        self.sim.key_up(GIAction::SprintMouse)?;
        Ok(())
    }
    /// 行走
    pub async fn walk(&self, dir: WalkDir, ms: i32) -> Result<(), AvatarError> {
        self.check_cancel()?;
        let action = dir.to_action();
        self.sim.key_down(action)?;
        self.sleep_uninterruptible(ms.max(0) as u64).await;
        self.sim.key_up(action)?;
        Ok(())
    }
    /// 跳跃
    pub fn jump(&self) -> Result<(), AvatarError> {
        self.sim.key_press(GIAction::Jump)?;
        Ok(())
    }
    /// 等待
    pub async fn wait(&self, ms: i32) {
        self.sleep_uninterruptible(ms.max(0) as u64).await;
    }
    /// 视角移动
    pub fn move_camera(&self, dx: i32, dy: i32) -> Result<(), AvatarError> {
        self.sim.move_mouse_by(dx, dy)?;
        Ok(())
    }
    /// 等待角色编号块出现
    pub async fn ready<F>(&self, mut has_index_rect: F) -> Result<(), AvatarError>
    where
        F: FnMut() -> bool,
    {
        self.sleep_ct(10).await?;
        for _ in 0..20 {
            self.check_cancel()?;
            if has_index_rect() {
                return Ok(());
            }
            self.sleep_ct(150).await?;
        }
        Ok(())
    }
    // 原始操作
    pub fn mouse_down(&self, key: &str) -> Result<(), AvatarError> {
        let btn = parse_mouse(key);
        self.sim.mouse_down(btn)?;
        Ok(())
    }
    pub fn mouse_up(&self, key: &str) -> Result<(), AvatarError> {
        let btn = parse_mouse(key);
        self.sim.mouse_up(btn)?;
        Ok(())
    }
    pub async fn mouse_click(&self, key: &str) -> Result<(), AvatarError> {
        self.sim.mouse_click(parse_mouse(key)).await?;
        Ok(())
    }
    pub fn move_by(&self, dx: i32, dy: i32) -> Result<(), AvatarError> {
        self.sim.move_mouse_by(dx, dy)?;
        Ok(())
    }
    pub fn scroll(&self, clicks: i32) -> Result<(), AvatarError> {
        self.sim.scroll(clicks)?;
        Ok(())
    }
    /// E技能是否就绪
    /// 屏幕对接[`create::navigate::bv::skill::read_e_cooldown_ready`]
    pub fn is_skill_ready(&self) -> bool {
        self.get_skill_cd_seconds() <= 0.0
    }
    /// E技能冷却
    pub fn get_skill_cd_seconds(&self) -> f64 {
        let now = Instant::now();
        let last = *self.last_skill_time.lock().unwrap();
        let ocr = *self.ocr_skill_cd.lock().unwrap();
        if self.manual_skill_cd < 0.0 {
            // 自动模式: 取max(skill_cd,skill_hold_cd)作为最大上限
            let max_cd = self.combat.skill_cd.max(self.combat.skill_hold_cd);
            let target = match (last, ocr) {
                (Some(l), Some(o)) if l >= o => l + Duration::from_secs_f64(max_cd),
                (None, Some(o)) => o,
                (Some(l), None) => l + Duration::from_secs_f64(max_cd),
                (None, None) => return 0.0,
                (_, Some(o)) => o,
            };
            if now >= target {
                return 0.0;
            }
            let result = target.saturating_duration_since(now).as_secs_f64();
            if result > max_cd {
                log::warn!("{}的当前技能CD大于其最大技能CD{}", self.name(), max_cd);
                return max_cd;
            }
            result
        } else if self.manual_skill_cd > 0.0 {
            let last = match last {
                Some(t) => t,
                None => return 0.0,
            };
            let elapsed = now.saturating_duration_since(last).as_secs_f64();
            if self.manual_skill_cd > elapsed {
                self.manual_skill_cd - elapsed
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
    /// OCR识别CD
    pub fn record_ocr_cd(&self, cd_seconds: f64) {
        if cd_seconds > 0.0 && cd_seconds <= self.combat.skill_cd {
            *self.ocr_skill_cd.lock().unwrap() = Some(Instant::now() + Duration::from_secs_f64(cd_seconds));
        }
    }
}

fn parse_mouse(key: &str) -> MouseButton {
    match key.to_ascii_lowercase().as_str() {
        "right" => MouseButton::Right,
        "middle" => MouseButton::Middle,
        _ => MouseButton::Left,
    }
}
