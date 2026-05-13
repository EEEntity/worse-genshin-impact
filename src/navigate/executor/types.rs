//! 导航执行器类型

use std::time::{Duration, Instant};
use opencv::core::Mat;

use crate::avatar::CombatScenes;
use crate::avatar::avatar::CancelFlag;
use crate::device::GIDevice;
use crate::device::simulator::Simulator;
use crate::inference::ocr::OcrEngine;
use crate::navigate::action::ActionRegistry;
use crate::navigate::constants::FRAME_INTERVAL_MS;
use crate::navigate::coord::Point2f;
use crate::navigate::error::NavigateError;
use crate::navigate::locate::MultiSceneLocator;
use crate::navigate::map::BigMapMatcher;
use crate::navigate::rotate::RotateController;
use crate::task::choose_talk_option::ScreenCapturer;
use crate::navigate::tp::TpTask;
use crate::navigate::trap::TrapEscaper;

pub trait MinimapSource {
    fn capture_minimap(&mut self) -> Result<Mat, NavigateError>;
    fn capture_screen(&mut self) -> Result<Option<Mat>, NavigateError> {
        Ok(None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveOutcome {
    Arrived,
    Timeout,
    TooFarRetriesExceeded,
    StuckRetriesExceeded,
}

pub struct PathExecutor<S: MinimapSource> {
    pub locator: MultiSceneLocator,
    pub rotate: RotateController,
    pub device: GIDevice,
    pub source: S,
    pub trap: TrapEscaper,
    pub tp: Option<TpTask>,
    pub ocr: Option<OcrEngine>,
    pub big_map: Option<BigMapMatcher>,
    pub actions: ActionRegistry,
    pub sim: Option<std::sync::Arc<Simulator>>,
    pub cancel: Option<CancelFlag>,
    pub avatars: Option<std::sync::Arc<CombatScenes>>,
    pub screen_capturer: Option<std::sync::Arc<dyn ScreenCapturer>>,
    pub frame_interval: Duration,
    pub waypoint_timeout: Duration,
    pub too_far_retry_limit: u32,
    pub stuck_retry_limit: u32,
    pub(crate) fly_takeoff_pressed: bool,
    pub(crate) run_sprint_held: bool,
    pub(crate) last_sprint_at: Instant,
    pub(crate) move_frame_num: u32,
    pub(crate) consecutive_rotation_count_beyond_angle: u32,
    pub auto_run_enabled: bool,
}

impl<S: MinimapSource> PathExecutor<S> {
    pub fn new(locator: MultiSceneLocator, device: GIDevice, source: S) -> Self {
        Self {
            locator,
            rotate: RotateController::default(),
            device,
            source,
            trap: TrapEscaper::new(),
            tp: None,
            ocr: None,
            big_map: None,
            actions: ActionRegistry::with_defaults(),
            sim: None,
            cancel: None,
            avatars: None,
            screen_capturer: None,
            frame_interval: Duration::from_millis(FRAME_INTERVAL_MS),
            waypoint_timeout: Duration::from_secs(240),
            too_far_retry_limit: 50,
            stuck_retry_limit: 2,
            fly_takeoff_pressed: false,
            run_sprint_held: false,
            last_sprint_at: Instant::now() - Duration::from_secs(60),
            move_frame_num: 0,
            consecutive_rotation_count_beyond_angle: 0,
            auto_run_enabled: false,
        }
    }

    pub fn with_tp(mut self, tp: TpTask) -> Self {
        self.tp = Some(tp);
        self
    }

    pub fn with_ocr(mut self, ocr: OcrEngine) -> Self {
        self.ocr = Some(ocr);
        self
    }

    pub fn with_big_map(mut self, big_map: BigMapMatcher) -> Self {
        self.big_map = Some(big_map);
        self
    }

    pub fn with_sim(mut self, sim: std::sync::Arc<Simulator>) -> Self {
        self.sim = Some(sim);
        self
    }

    pub fn with_cancel(mut self, cancel: CancelFlag) -> Self {
        self.cancel = Some(cancel);
        self
    }

    pub fn with_avatars(mut self, avatars: std::sync::Arc<CombatScenes>) -> Self {
        self.avatars = Some(avatars);
        self
    }

    pub fn with_screen_capturer(mut self, screen_capturer: std::sync::Arc<dyn ScreenCapturer>) -> Self {
        self.screen_capturer = Some(screen_capturer);
        self
    }
}

pub fn target_orientation_deg(target: Point2f, position: Point2f) -> f32 {
    let dx = (position.x - target.x) as f64;
    let dy = (position.y - target.y) as f64;
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return 0.0;
    }
    let mut angle = (dx / len).clamp(-1.0, 1.0).acos();
    if dy < 0.0 {
        angle = 2.0 * std::f64::consts::PI - angle;
    }
    (angle.to_degrees()) as f32
}
