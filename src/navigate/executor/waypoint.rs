//! 路径点执行逻辑

use std::time::{Duration, Instant};
use evdev_rs::enums::EV_KEY;
use opencv::core::Mat;
use tokio::time::sleep;

use crate::navigate::action::ActionPhase;
use crate::navigate::coord::Point2f;
use crate::navigate::error::NavigateError;
use crate::navigate::path::{ActionCode, MoveMode, PathingTask, WaypointType};

use super::types::{MinimapSource, MoveOutcome, PathExecutor};

impl<S: MinimapSource> PathExecutor<S> {
    pub(crate) async fn run_pathing_once(&mut self, task: &PathingTask) -> Result<(), NavigateError> {
        log::info!(
            "开始执行路径 \"{}\"（共 {} 个点位）",
            task.info.name,
            task.positions.len()
        );
        let _ = self.device.key_up(EV_KEY::KEY_W);
        let _ = self.device.key_up(EV_KEY::KEY_A);
        let _ = self.device.key_up(EV_KEY::KEY_S);
        let _ = self.device.key_up(EV_KEY::KEY_D);
        let _ = self.device.key_up(EV_KEY::KEY_LEFTSHIFT);
        let _ = self.device.key_up(EV_KEY::KEY_SPACE);
        self.fly_takeoff_pressed = false;
        self.run_sprint_held = false;
        self.move_frame_num = 0;
        self.consecutive_rotation_count_beyond_angle = 0;
        self.last_sprint_at = Instant::now() - Duration::from_secs(60);
        if let Ok(Some(screen)) = self.source.capture_screen() {
            use opencv::core::MatTraitConst;
            let w = screen.cols();
            let h = screen.rows();
            if w > 0 && h > 0 && (w * 9 != h * 16 || w < 1920) {
                log::warn!("游戏窗口分辨率 {w}x{h} 不是 16:9 或低于 1920x1080，路径可能失败");
            }
        }
        let mut prev_kind: Option<WaypointType> = None;
        let mut prev_action: ActionCode = ActionCode::None;
        let map_name: &str = if task.info.map_name.is_empty() {
            "Teyvat"
        } else {
            task.info.map_name.as_str()
        };
        for (idx, w) in task.positions.iter().enumerate() {
            let action = ActionCode::from_optional(w.action.as_deref());
            let kind_pre = if matches!(action, ActionCode::ForceTp) {
                WaypointType::Teleport
            } else if let Some(forced) = action.enforces_waypoint_type() {
                forced
            } else {
                WaypointType::from_code(&w.waypoint_type)
            };
            if matches!(kind_pre, WaypointType::Teleport) {
                self.recover_when_low_hp(idx, Point2f::new(w.x as f32, w.y as f32)).await?;
            }
            let kind = kind_pre;
            match kind {
                WaypointType::Teleport => {
                    if let Some(pk) = prev_kind {
                        let no_delay = matches!(pk, WaypointType::Teleport)
                            || matches!(
                                prev_action,
                                ActionCode::Fight | ActionCode::NahidaCollect | ActionCode::PickAround
                            );
                        if !no_delay {
                            sleep(Duration::from_millis(1000)).await;
                        }
                    }
                    if self.tp.is_some() {
                        let source_ptr: *mut S = &mut self.source;
                        let mut provider = move || -> Result<Option<Mat>, NavigateError> {
                            let s = unsafe { &mut *source_ptr };
                            s.capture_screen()
                        };
                        let Self { tp, ocr, big_map, device, .. } = self;
                        let tp_ref = tp.as_ref().unwrap();
                        let mut deps = crate::navigate::tp::TpDeps {
                            device,
                            screen: &mut provider,
                            ocr: ocr.as_mut(),
                            big_map: big_map.as_mut(),
                        };
                        match tp_ref
                            .tp_to(&mut deps, Point2f::new(w.x as f32, w.y as f32), map_name)
                            .await
                        {
                            Ok(landed) => {
                                self.locator.set_prev_position_game(map_name, landed);
                            }
                            Err(NavigateError::Unsupported(msg)) => {
                                log::warn!("[{idx}] teleport 跳过：{msg}");
                            }
                            Err(e) => return Err(e),
                        }
                    } else {
                        log::warn!(
                            "[{idx}] teleport ({:.1}, {:.1}) 跳过：PathExecutor.tp 未设置",
                            w.x,
                            w.y
                        );
                    }
                    sleep(Duration::from_millis(500)).await;
                    prev_kind = Some(kind);
                    prev_action = action.clone();
                    continue;
                }
                WaypointType::Orientation => {
                    self.face_to(Point2f::new(w.x as f32, w.y as f32)).await?;
                }
                _ => {}
            }
            let segment_start = matches!(prev_kind, None | Some(WaypointType::Teleport));
            if segment_start {
                self.locator
                    .set_prev_position_game(map_name, Point2f::new(w.x as f32, w.y as f32));
            }
            let mode = MoveMode::from_code(&w.move_mode);
            let target = Point2f::new(w.x as f32, w.y as f32);
            let mis = &w.point_ext_params.misidentification;
            if matches!(action, ActionCode::UpDownGrabLeaf) {
                self.before_move_to_target_up_down_grab_leaf(
                    idx,
                    target,
                    map_name,
                    w.action_params.as_deref(),
                )
                .await?;
                prev_kind = Some(kind);
                prev_action = action.clone();
                continue;
            }
            let is_orientation = matches!(kind, WaypointType::Orientation);
            let outcome = if is_orientation {
                MoveOutcome::Arrived
            } else {
                self.move_to(target, mode, map_name, mis).await?
            };
            if !is_orientation && !matches!(outcome, MoveOutcome::Arrived) {
                let reason = match outcome {
                    MoveOutcome::Timeout => "move_to 超时",
                    MoveOutcome::TooFarRetriesExceeded => "move_to 距离过远重试超限",
                    MoveOutcome::StuckRetriesExceeded => "move_to 卡死重试超限",
                    MoveOutcome::Arrived => unreachable!(),
                };
                return Err(NavigateError::Retry(format!("[{idx}] {reason}，重试本段路线")));
            }
            if !is_orientation
                && matches!(mode, MoveMode::Fly)
                && matches!(action, ActionCode::StopFlying)
            {
                self.before_move_close_to_stop_flying(idx, w.action_params.as_deref())
                    .await?;
            }
            if !is_orientation && matches!(outcome, MoveOutcome::Arrived) && is_target_point(kind, &action) {
                if let Err(e) = self.move_close_to(target, map_name, mis).await {
                    log::warn!("[{idx}] move_close_to 失败：{e}");
                }
            }
            if !matches!(action, ActionCode::None) {
                self.run_action_phase(
                    idx,
                    &action,
                    w.action_params.as_deref(),
                    ActionPhase::AfterMoveToTarget,
                    true,
                )
                .await?;
            }
            prev_kind = Some(kind);
            prev_action = action;
        }
        Ok(())
    }
}

fn is_target_point(kind: WaypointType, action: &ActionCode) -> bool {
    if matches!(kind, WaypointType::Orientation) || matches!(action, ActionCode::UpDownGrabLeaf) {
        return false;
    }
    matches!(kind, WaypointType::Target | WaypointType::Teleport) || !matches!(action, ActionCode::None)
}
