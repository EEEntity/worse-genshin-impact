//! 自动钓鱼独立任务
//!
//! - 切换时间
//! - 进入钓鱼界面
//! - 识别鱼群
//! - 切换鱼饵
//! - 执行钓鱼流程并重新识别鱼群

mod assets;
mod fish_types;
mod recognition;
mod rod_net;
mod steps;

pub use fish_types::{BaitType, BigFishType, Fishpond, OneFish, BIG_FISH_TYPES};

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use opencv::core::Mat;

use crate::avatar::avatar::CancelFlag;
use crate::device::simulator::Simulator;
use crate::inference::model::Model;
use crate::inference::yolo::predictor::YoloPredictor;
use crate::inference::grid_icon::GridIconPredictor;
use crate::task::choose_talk_option::OcrHandle;

/// 钓鱼时段选项
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FishingTimePolicy {
    /// 不切换时间
    DontChange,
    /// 仅白天(7:00)
    Daytime,
    /// 仅夜晚(19:00)
    Nighttime,
    /// 白天和夜晚
    Both,
}

impl FishingTimePolicy {
    fn hours(self) -> &'static [i32] {
        match self {
            Self::DontChange => &[],
            Self::Daytime => &[7],
            Self::Nighttime => &[19],
            Self::Both => &[7, 19],
        }
    }
}

/// 自动钓鱼配置
pub struct AutoFishingConfig {
    /// 整轮超时时间，默认600秒
    pub whole_process_timeout_secs: u64,
    /// 抛竿后等待咬钩超时秒数，默认18秒
    pub throw_rod_timeout_secs: u64,
    /// 时段选项
    pub fishing_time_policy: FishingTimePolicy,
    /// 当前装备的鱼饵
    pub equipped_bait: Option<BaitType>,
    /// 鱼饵白名单
    pub bait_whitelist: Option<Vec<BaitType>>,
    /// 鱼类白名单
    pub fish_whitelist: Option<Vec<&'static str>>,
    /// 抛竿失败重试上限
    pub throw_rod_max_no_target: u32,
    /// 抛竿无目标鱼重试上限
    pub throw_rod_max_no_fish_loops: u32,
}

impl AutoFishingConfig {
    /// 默认值
    pub fn new(equipped_bait: Option<BaitType>) -> Self {
        Self {
            whole_process_timeout_secs: 600,
            throw_rod_timeout_secs: 18,
            fishing_time_policy: FishingTimePolicy::DontChange,
            equipped_bait,
            bait_whitelist: None,
            fish_whitelist: None,
            throw_rod_max_no_target: 2,
            throw_rod_max_no_fish_loops: 10,
        }
    }
    /// 鱼饵是否被白名单允许
    pub fn bait_allowed(&self, b: BaitType) -> bool {
        match &self.bait_whitelist {
            None => true,
            Some(v) if v.is_empty() => true,
            Some(v) => v.contains(&b),
        }
    }
    /// 鱼类是否被白名单允许
    pub fn fish_allowed(&self, name: &str) -> bool {
        match &self.fish_whitelist {
            None => true,
            Some(v) if v.is_empty() => true,
            Some(v) => v.iter().any(|n| *n == name),
        }
    }

}

/// 错误
#[derive(Debug)]
pub enum AutoFishingError {
    /// 模型加载失败
    ModelLoad(String),
    /// 截图源返回None/失败
    Capture,
    /// 设备错误
    Device(String),
    /// 导航错误
    Navigate(crate::navigate::error::NavigateError),
    /// 钓鱼任务超时
    Timeout,
    /// 主动取消
    Cancelled,
    /// 进入/退出钓鱼模式失败
    Flow(&'static str),
    /// 子任务(set_time)错误
    SetTime(String),    
}

impl std::fmt::Display for AutoFishingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModelLoad(s) => write!(f, "模型加载失败：{s}"),
            Self::Capture => write!(f, "截图失败"),
            Self::Device(s) => write!(f, "device error: {s}"),
            Self::Navigate(e) => write!(f, "{e}"),
            Self::Timeout => write!(f, "钓鱼任务超时"),
            Self::Cancelled => write!(f, "已取消"),
            Self::Flow(s) => write!(f, "钓鱼流程失败：{s}"),
            Self::SetTime(s) => write!(f, "切换时间失败：{s}"),
        }
    }
}

impl std::error::Error for AutoFishingError {}

impl From<crate::navigate::error::NavigateError> for AutoFishingError {
    fn from(e: crate::navigate::error::NavigateError) -> Self {
        Self::Navigate(e)
    }
}

/// 截图回调
pub type ScreenFn = Box<dyn FnMut() -> Option<Mat> + Send>;

/// 自动钓鱼任务
pub struct AutoFishingTask {
    pub(crate) sim: Arc<Simulator>,
    pub(crate) cancel: CancelFlag,
    pub(crate) predictor: YoloPredictor,
    /// 鱼饵图标识别器(GridIcons)
    pub(crate) grid_icon: Option<GridIconPredictor>,
    pub(crate) ocr: OcrHandle,
    pub(crate) config: AutoFishingConfig,
    /// 阶段间共享的运行状态
    pub(crate) bb: steps::Blackboard,
}

impl AutoFishingTask {
    /// 默认模型路径
    pub fn new(
        sim: Arc<Simulator>,
        cancel: CancelFlag,
        ocr: OcrHandle,
        config: AutoFishingConfig,
    ) -> Result<Self, AutoFishingError> {
        Self::with_model(
            sim,
            cancel,
            ocr,
            config,
            Model::Fish.model_path(),
        )
    }
    /// 指定模型路径
    pub fn with_model(
        sim: Arc<Simulator>,
        cancel: CancelFlag,
        ocr: OcrHandle,
        config: AutoFishingConfig,
        model_path: impl AsRef<Path>,
    ) -> Result<Self, AutoFishingError> {
        let predictor = YoloPredictor::load(model_path.as_ref())
            .map_err(|e|AutoFishingError::ModelLoad(e.to_string()))?;
        // GridIcons失败时只有warn
        // `choose_bait`会自动跳过UI操作
        let onnx = Model::GridIcon.model_path();
        let csv = Model::GridIcon
            .label_full_path()
            .expect("GridIcon must have items.csv label path");
        let grid_icon = match GridIconPredictor::load(&onnx, &csv) {
            Ok(p) => Some(p),
            Err(e) => {
                log::warn!(
                    "GridIcons 加载失败 ({onnx} / {csv}): {e}；将跳过自动换饵 UI 操作"
                );
                None
            }
        };
        Ok(Self {
            sim,
            cancel,
            predictor,
            grid_icon,
            ocr,
            config,
            bb: steps::Blackboard::default(),
        })
    }
    pub(crate) fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    /// 运行任务
    pub async fn run<C>(&mut self, mut screen: ScreenFn, set_time_cap: Option<&C>) -> Result<(), AutoFishingError>
    where
        C: crate::task::choose_talk_option::ScreenCapturer + ?Sized,
    {
        log::info!("自动钓鱼，启动！");
        log::warn!("请不要携带任何跟宠，极有可能会误识别导致拖慢速度！");
        let bait_wl = match &self.config.bait_whitelist {
            None => "全部".to_string(),
            Some(v) if v.is_empty() => "全部".to_string(),
            Some(v) => v.iter().map(|b| b.chinese_name()).collect::<Vec<_>>().join(","),
        };
        let fish_wl = match &self.config.fish_whitelist {
            None => "全部".to_string(),
            Some(v) if v.is_empty() => "全部".to_string(),
            Some(v) => v.join(","),
        };
        log::info!(
            "当前参数：{}s/{}s, 时段={:?}, 当前饵={}, 饵白名单=[{}], 鱼白名单=[{}]",
            self.config.whole_process_timeout_secs,
            self.config.throw_rod_timeout_secs,
            self.config.fishing_time_policy,
            self.config
                .equipped_bait
                .map(|b| b.chinese_name())
                .unwrap_or("(未指定，运行时识别)"),
            bait_wl,
            fish_wl,
        );
        let hours = self.config.fishing_time_policy.hours();
        if hours.is_empty() {
            self.run_round(&mut screen).await?;
        } else {
            for &h in hours {
                if self.cancelled() {
                    return Err(AutoFishingError::Cancelled);
                }
                if let Some(cap) = set_time_cap {
                    log::info!("切换游戏内时间到 {h} 点");
                    crate::task::set_time::set_time(&self.sim, cap, h, 0, true)
                        .await
                        .map_err(|e| AutoFishingError::SetTime(e.to_string()))?;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                } else {
                    log::warn!(
                        "FishingTimePolicy={:?} 但未提供 set_time_cap，跳过时段切换",
                        self.config.fishing_time_policy
                    );
                }
                self.run_round(&mut screen).await?;
            }
        }
        log::info!("钓鱼任务结束");
        Ok(())
    }
    /// 单轮(半天)
    async fn run_round(&mut self, screen: &mut ScreenFn) -> Result<(), AutoFishingError> {
        self.bb.reset();
        // None: 等`EnterFishingMode`末尾用`GridIcons`识别后回填
        // Some: 调用方指定，跳过识别
        self.bb.selected_bait = self.config.equipped_bait;
        let deadline = std::time::Instant::now()
            + Duration::from_secs(self.config.whole_process_timeout_secs.max(1));
        // 调整视角到鱼塘
        steps::adjust_view_down(self).await?;
        let found = steps::find_fish(self, screen, deadline, Duration::from_secs(20)).await?;
        if !found {
            self.bb.abort = true;
            return self.try_quit_mode(screen).await;
        }
        // 进入钓鱼
        if !steps::enter_fishing_mode(self, screen, Duration::from_secs(10)).await? {
            log::warn!("进入钓鱼模式失败");
            return Ok(());
        }
        // 一直没有鱼/异常/整体超时
        loop {
            if self.cancelled() {
                self.try_quit_mode(screen).await.ok();
                return Err(AutoFishingError::Cancelled);
            }
            if std::time::Instant::now() >= deadline {
                log::info!("整体超时已到，强制结束本轮");
                break;
            }
            // 调整视角找鱼塘
            steps::adjust_view_down(self).await?;
            let inner_found = steps::find_fish_with_initial_check(
                self,
                screen,
                Duration::from_secs(10),
            )
            .await?;
            if !inner_found {
                log::info!("内层 10s 没找到鱼，结束本轮");
                self.bb.abort = true;
                break;
            }
            // 选择鱼饵
            let _ = steps::choose_bait(self, screen).await?;
            // 抛竿
            let throw_ok = steps::throw_rod_until_success(self, screen).await?;
            if self.bb.abort {
                break;
            }
            if !throw_ok {
                continue;
            }
            // 检查抛竿结果
            let cast_ok = steps::check_throw_rod(self, screen, Duration::from_secs(3)).await?;
            if !cast_ok {
                log::info!("抛竿检查失败，重新找鱼后重试");
                continue;
            }
            // 等咬钩 -> 自动提竿
            steps::wait_for_bite(
                self,
                screen,
                Duration::from_secs(self.config.throw_rod_timeout_secs),
            )
            .await?;
            // 找钓鱼条
            let fish_box_ok = steps::get_fish_box(self, screen, Duration::from_secs(5)).await?;
            if !fish_box_ok {
                log::warn!("钓鱼框识别失败，跳过本条");
                continue;
            }
            // 拉条
            steps::pulling(self, screen).await?;
        }
        // 退出钓鱼模式
        self.try_quit_mode(screen).await
    }
    /// 退出钓鱼模式
    async fn try_quit_mode(&mut self, screen: &mut ScreenFn) -> Result<(), AutoFishingError> {
        if let Err(e) = steps::quit_fishing_mode(self, screen, Duration::from_secs(15)).await {
            log::warn!("退出钓鱼模式失败：{e}");
        }
        // 松开所有键
        let _ = self.sim.release_all_keys();
        Ok(())
    }
}
