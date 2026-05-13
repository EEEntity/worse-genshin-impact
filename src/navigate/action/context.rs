//! 动作执行时上下文

use std::sync::Arc;
use opencv::core::Mat;

use crate::avatar::CombatScenes;
use crate::avatar::avatar::CancelFlag;
use crate::device::GIDevice;
use crate::device::simulator::Simulator;
use crate::navigate::error::NavigateError;
use crate::task::choose_talk_option::ScreenCapturer;

/// 全屏图像闭包
pub type ScreenProvider<'a> = &'a mut dyn FnMut() -> Result<Option<Mat>, NavigateError>;

/// Action执行阶段
// 寻路用的
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionPhase {
    BeforeMoveToTarget,
    BeforeMoveCloseToTarget,
    AfterMoveToTarget,
}

/// Action执行上下文
pub struct ActionContext<'a> {
    pub device: &'a GIDevice,
    pub action_params: Option<&'a str>,
    pub screen: Option<ScreenProvider<'a>>,
    pub phase: ActionPhase,
    pub sim: Option<Arc<Simulator>>,
    pub cancel: Option<CancelFlag>,
    pub avatars: Option<&'a CombatScenes>,
    pub screen_capturer: Option<Arc<dyn ScreenCapturer>>,
}

/// Path Action寻路接口
pub trait ActionHandler: Send + Sync {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>>;
}
