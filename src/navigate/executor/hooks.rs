//! 自动导航执行器hooks

use std::time::Duration;
use opencv::core::Mat;
use tokio::time::sleep;

use crate::navigate::action::{ActionContext, ActionPhase};
use crate::navigate::coord::Point2f;
use crate::navigate::error::NavigateError;
use crate::navigate::path::ActionCode;

use super::types::{MinimapSource, PathExecutor, target_orientation_deg};

impl<S: MinimapSource> PathExecutor<S> {
    pub(crate) async fn run_action_phase(
        &mut self,
        idx: usize,
        action: &ActionCode,
        action_params: Option<&str>,
        phase: ActionPhase,
        warn_if_missing: bool,
    ) -> Result<(), NavigateError> {
        let Some(handler) = self.actions.get_for_phase(action, phase) else {
            if warn_if_missing {
                log::warn!("[{idx}] {:?} {:?} 跳过：未注册处理器", phase, action);
            }
            return Ok(());
        };
        let source_ptr: *mut S = &mut self.source;
        let mut provider = move || -> Result<Option<Mat>, NavigateError> {
            let s = unsafe { &mut *source_ptr };
            s.capture_screen()
        };
        let ctx = ActionContext {
            device: &self.device,
            action_params,
            screen: Some(&mut provider),
            phase,
            sim: self.sim.clone(),
            cancel: self.cancel.clone(),
            avatars: self.avatars.as_deref(),
            screen_capturer: self.screen_capturer.clone(),
        };
        match handler.run(ctx).await {
            Ok(()) => Ok(()),
            Err(NavigateError::Unsupported(msg)) => {
                log::warn!("[{idx}] {:?} {:?} 跳过: {msg}", phase, action);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub(crate) async fn before_move_close_to_stop_flying(
        &mut self,
        idx: usize,
        action_params: Option<&str>,
    ) -> Result<(), NavigateError> {
        let action = ActionCode::StopFlying;
        self.run_action_phase(
            idx,
            &action,
            action_params,
            ActionPhase::BeforeMoveCloseToTarget,
            true,
        )
        .await
    }

    pub(crate) async fn before_move_to_target_up_down_grab_leaf(
        &mut self,
        idx: usize,
        target: Point2f,
        _map_name: &str,
        action_params: Option<&str>,
    ) -> Result<(), NavigateError> {
        let map_err = |e: crate::device::DeviceError| NavigateError::Device(e.to_string());
        self.device
            .mouse_click(
                evdev_rs::enums::EV_KEY::BTN_MIDDLE,
                Duration::ZERO,
                Duration::from_millis(40),
                Duration::ZERO,
            )
            .map_err(map_err)?;
        sleep(Duration::from_millis(300)).await;

        let pos = match self.try_locate_once().await? {
            Some(p) => p,
            None => Point2f::new(0.0, 0.0),
        };
        let target_orient = target_orientation_deg(target, pos);
        let _ = self.rotate_until(target_orient, 10.0, Duration::from_secs(3)).await;
        let action = ActionCode::UpDownGrabLeaf;
        self.run_action_phase(
            idx,
            &action,
            action_params,
            ActionPhase::BeforeMoveToTarget,
            true,
        )
        .await
    }
}
