//! 移动相关

use std::time::{Duration, Instant};
use evdev_rs::enums::EV_KEY;
use tokio::time::sleep;

use crate::navigate::constants::{ARRIVAL_DISTANCE, STUCK_DELTA, STUCK_FRAMES, TOO_FAR_DISTANCE};
use crate::navigate::coord::Point2f;
use crate::navigate::error::NavigateError;
use crate::navigate::orient::{compute_camera_angle, compute_character_angle};
use crate::navigate::path::{Misidentification, MoveMode};
use crate::navigate::scene::scene_by_name;

use super::types::{MinimapSource, MoveOutcome, PathExecutor, target_orientation_deg};

impl<S: MinimapSource> PathExecutor<S> {
    pub async fn face_to(&mut self, target_game: Point2f) -> Result<(), NavigateError> {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if Instant::now() > deadline {
                return Err(NavigateError::Timeout("face_to 超时".into()));
            }
            let mm = self.source.capture_minimap()?;
            let pos = match self.locator.locate(&mm)? {
                Some(info) => Point2f::new(info.game_pos.x, info.game_pos.y),
                None => {
                    sleep(self.frame_interval).await;
                    continue;
                }
            };
            let target_orient = target_orientation_deg(target_game, pos);
            let cam = compute_camera_angle(&mm)?;
            let aligned = self
                .rotate
                .step(&self.device, cam, target_orient)
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            if aligned {
                return Ok(());
            }
            sleep(self.frame_interval).await;
        }
    }

    pub async fn move_to(
        &mut self,
        target_game: Point2f,
        move_mode: MoveMode,
        map_name: &str,
        mis: &Misidentification,
    ) -> Result<MoveOutcome, NavigateError> {
        let scale = scene_by_name(map_name)
            .map(|s| s.geom.block_scale_to_1024() as f64)
            .unwrap_or(1.0);
        if let Some(start_pos) = self.try_locate_once().await? {
            let target_orient = target_orientation_deg(target_game, start_pos);
            let _ = self.rotate_until(target_orient, 5.0, Duration::from_secs(3)).await;
        }
        let start = Instant::now();
        self.fly_takeoff_pressed = false;
        self.run_sprint_held = false;
        self.move_frame_num = 0;
        self.consecutive_rotation_count_beyond_angle = 0;
        self.last_sprint_at = Instant::now() - Duration::from_secs(60);
        self.device
            .key_down(EV_KEY::KEY_W)
            .map_err(|e| NavigateError::Device(e.to_string()))?;
        let mut last_sample = Instant::now();
        let mut last_diag = Instant::now() - Duration::from_secs(1);
        let mut samples: Vec<Point2f> = Vec::with_capacity(STUCK_FRAMES + 4);
        let mut too_far_retry: u32 = 0;
        let mut stuck_count: u32 = 0;
        let mut prev_pos: Option<Point2f> = None;
        let mut prev_not_too_far: Option<Point2f> = None;
        let unrecognized_handling = mis.kinds.iter().any(|s| s == "unrecognized");
        let path_too_far_handling = mis.kinds.iter().any(|s| s == "pathTooFar");
        let result = loop {
            if start.elapsed() > self.waypoint_timeout {
                break MoveOutcome::Timeout;
            }
            let mm = match self.source.capture_minimap() {
                Ok(m) => m,
                Err(_) => {
                    sleep(self.frame_interval).await;
                    continue;
                }
            };
            let pos = match self.locator.locate(&mm)? {
                Some(info) => {
                    let p = Point2f::new(info.game_pos.x, info.game_pos.y);
                    prev_pos = Some(p);
                    p
                }
                None => {
                    if unrecognized_handling {
                        match mis.handling_mode.as_str() {
                            "previousDetectedPoint" => {
                                if let Some(p) = prev_pos {
                                    p
                                } else {
                                    sleep(self.frame_interval).await;
                                    continue;
                                }
                            }
                            _ => {
                                if let Some(p) = prev_pos {
                                    p
                                } else {
                                    sleep(self.frame_interval).await;
                                    continue;
                                }
                            }
                        }
                    } else {
                        sleep(self.frame_interval).await;
                        continue;
                    }
                }
            };
            let mut distance = (pos.distance_to(target_game) as f64) * scale;
            if distance > TOO_FAR_DISTANCE
                && path_too_far_handling
                && mis.handling_mode == "previousDetectedPoint"
            {
                if let Some(p) = prev_pos {
                    let prev_dist = (p.distance_to(target_game) as f64) * scale;
                    if prev_dist <= TOO_FAR_DISTANCE {
                        distance = prev_dist;
                    }
                }
            }
            if distance < ARRIVAL_DISTANCE {
                break MoveOutcome::Arrived;
            }
            if distance > TOO_FAR_DISTANCE {
                too_far_retry += 1;
                if too_far_retry % 10 == 0 {
                    if let Some(p) = prev_not_too_far {
                        self.locator.set_prev_position_game(map_name, p);
                    }
                    sleep(Duration::from_millis(500)).await;
                }
                if too_far_retry > self.too_far_retry_limit {
                    break MoveOutcome::TooFarRetriesExceeded;
                }
                sleep(Duration::from_millis(50)).await;
                continue;
            } else {
                prev_not_too_far = Some(pos);
            }
            if last_sample.elapsed() > Duration::from_millis(1000) {
                last_sample = Instant::now();
                samples.push(pos);
                if samples.len() > STUCK_FRAMES {
                    let head = samples[samples.len() - STUCK_FRAMES];
                    let tail = samples[samples.len() - 1];
                    let delta = (tail.x - head.x).abs() + (tail.y - head.y).abs();
                    if (delta as f64) * scale < STUCK_DELTA && move_mode != MoveMode::Climb {
                        stuck_count += 1;
                        if stuck_count > self.stuck_retry_limit {
                            break MoveOutcome::StuckRetriesExceeded;
                        }
                        if let Err(e) = self.trap.rotate_and_move(&self.device) {
                            log::warn!("RotateAndMove 失败: {e}");
                        }
                        if let Err(e) = self.escape_move_to(target_game, move_mode, map_name).await {
                            log::warn!("escape_move_to 失败: {e}");
                        }
                        if let Err(e) = self.device.key_down(EV_KEY::KEY_W) {
                            log::warn!("卡死脱离后重新按 W 失败: {e}");
                        }
                        last_sample = Instant::now();
                        continue;
                    }
                }
            }
            self.trap.maybe_reset();
            let target_orient = target_orientation_deg(target_game, pos) + self.trap.random_offset_deg();
            let cam = compute_camera_angle(&mm)?;
            self.rotate
                .step(&self.device, cam, target_orient)
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            self.move_frame_num = self.move_frame_num.saturating_add(1);
            if self.move_frame_num > 20 {
                let diff_abs = crate::navigate::rotate::RotateController::shortest_diff(target_orient, cam).abs();
                if diff_abs > 5.0 {
                    self.consecutive_rotation_count_beyond_angle =
                        self.consecutive_rotation_count_beyond_angle.saturating_add(1);
                } else {
                    self.consecutive_rotation_count_beyond_angle = 0;
                }
                if self.consecutive_rotation_count_beyond_angle > 10 {
                    let _ = self.rotate_until(target_orient, 2.0, Duration::from_secs(3)).await;
                    self.consecutive_rotation_count_beyond_angle = 0;
                }
            }
            self.handle_move_mode(move_mode, distance).await?;
            if last_diag.elapsed() > Duration::from_millis(500) {
                last_diag = Instant::now();
                let diff = crate::navigate::rotate::RotateController::shortest_diff(target_orient, cam);
                log::debug!(
                    "pos=({:.1},{:.1}) dist={:.1} cam={:.1} target={:.1} diff={:+.1}",
                    pos.x,
                    pos.y,
                    distance,
                    cam,
                    target_orient,
                    diff
                );
            }
            if log::log_enabled!(log::Level::Trace) {
                let ch = compute_character_angle(&mm)?.unwrap_or(f32::NAN);
                log::trace!(
                    "pos=({:.1},{:.1}) dist={:.1} cam={:.1} target={:.1} char={:.1}",
                    pos.x,
                    pos.y,
                    distance,
                    cam,
                    target_orient,
                    ch
                );
            }
        };
        self.device
            .key_up(EV_KEY::KEY_W)
            .map_err(|e| NavigateError::Device(e.to_string()))?;
        Ok(result)
    }

    pub(crate) async fn try_locate_once(&mut self) -> Result<Option<Point2f>, NavigateError> {
        let mm = self.source.capture_minimap()?;
        Ok(self
            .locator
            .locate(&mm)?
            .map(|i| Point2f::new(i.game_pos.x, i.game_pos.y)))
    }

    async fn escape_move_to(
        &mut self,
        target_game: Point2f,
        move_mode: MoveMode,
        _map_name: &str,
    ) -> Result<(), NavigateError> {
        let map_err = |e: crate::device::DeviceError| NavigateError::Device(e.to_string());
        let start = Instant::now();
        let mm0 = self.source.capture_minimap()?;
        let pos0 = self
            .locator
            .locate(&mm0)?
            .map(|i| Point2f::new(i.game_pos.x, i.game_pos.y));
        self.trap.touch_action_time();
        if let Some(p) = pos0 {
            let target_orient = target_orientation_deg(target_game, p) + self.trap.random_offset_deg();
            let _ = self.rotate_until(target_orient, 5.0, Duration::from_secs(3)).await;
        }
        self.device.key_down(EV_KEY::KEY_W).map_err(map_err)?;
        let result: Result<(), NavigateError> = loop {
            let now = Instant::now();
            if now.duration_since(self.trap.last_action_time()) > self.trap.idle_timeout {
                break Ok(());
            }
            if now.duration_since(start) > self.trap.max_escape {
                break Ok(());
            }
            let mm = match self.source.capture_minimap() {
                Ok(m) => m,
                Err(_) => {
                    sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };
            let position = match self.locator.locate(&mm)? {
                Some(info) => Point2f::new(info.game_pos.x, info.game_pos.y),
                None => Point2f::new(0.0, 0.0),
            };
            let target_orient = target_orientation_deg(target_game, position) + self.trap.random_offset_deg();
            let _ = self.rotate_until(target_orient, 5.0, Duration::from_secs(3)).await;
            self.device.key_down(EV_KEY::KEY_W).map_err(map_err)?;
            self.trap.maybe_reset();
            if move_mode != MoveMode::Climb && move_mode != MoveMode::Fly {
                if let Some(screen) = self.source.capture_screen()? {
                    match crate::navigate::bv::get_motion_status(&screen) {
                        Ok(crate::navigate::bv::MotionStatus::Climb) => {
                            self.trap.climb_recover(&self.device)?;
                            continue;
                        }
                        Ok(_) => {}
                        Err(_) => {}
                    }
                }
            }
            sleep(Duration::from_millis(100)).await;
        };

        let _ = self.device.key_up(EV_KEY::KEY_W);
        result
    }

    pub(crate) async fn rotate_until(
        &mut self,
        target_orient: f32,
        tolerance_deg: f32,
        budget: Duration,
    ) -> Result<bool, NavigateError> {
        let dl = Instant::now() + budget;
        loop {
            if Instant::now() > dl {
                return Ok(false);
            }
            let mm = self.source.capture_minimap()?;
            let cam = compute_camera_angle(&mm)?;
            let diff = crate::navigate::rotate::RotateController::shortest_diff(target_orient, cam).abs();
            if diff < tolerance_deg {
                return Ok(true);
            }
            self.rotate
                .step(&self.device, cam, target_orient)
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(50)).await;
        }
    }

    async fn handle_move_mode(&mut self, mode: MoveMode, distance: f64) -> Result<(), NavigateError> {
        if matches!(mode, MoveMode::Fly) {
            let screen = self.source.capture_screen()?;
            let is_flying = match screen {
                Some(s) => matches!(
                    crate::navigate::bv::get_motion_status(&s),
                    Ok(crate::navigate::bv::MotionStatus::Fly)
                ),
                None => self.fly_takeoff_pressed,
            };
            if !is_flying {
                self.device
                    .press_keys(&[EV_KEY::KEY_SPACE], Duration::from_millis(40))
                    .map_err(|e| NavigateError::Device(e.to_string()))?;
                self.fly_takeoff_pressed = true;
                sleep(Duration::from_millis(200)).await;
            }
            sleep(Duration::from_millis(100)).await;
            return Ok(());
        }
        if matches!(mode, MoveMode::Jump) {
            self.device
                .press_keys(&[EV_KEY::KEY_SPACE], Duration::from_millis(40))
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(200)).await;
            return Ok(());
        }
        if matches!(mode, MoveMode::Run) {
            let want_fast = distance > 20.0;
            if want_fast != self.run_sprint_held {
                if self.run_sprint_held {
                    self.device
                        .mouse_button_up(EV_KEY::BTN_RIGHT)
                        .map_err(|e| NavigateError::Device(e.to_string()))?;
                } else {
                    self.device
                        .mouse_button_down(EV_KEY::BTN_RIGHT)
                        .map_err(|e| NavigateError::Device(e.to_string()))?;
                }
                self.run_sprint_held = !self.run_sprint_held;
            }
            sleep(Duration::from_millis(100)).await;
            return Ok(());
        }
        if matches!(mode, MoveMode::Dash) {
            if distance > 20.0 && self.last_sprint_at.elapsed() > Duration::from_millis(1000) {
                self.last_sprint_at = Instant::now();
                self.device
                    .mouse_click(
                        EV_KEY::BTN_RIGHT,
                        Duration::ZERO,
                        Duration::from_millis(40),
                        Duration::ZERO,
                    )
                    .map_err(|e| NavigateError::Device(e.to_string()))?;
            }
            sleep(Duration::from_millis(100)).await;
            return Ok(());
        }
        if matches!(mode, MoveMode::Climb) {
            sleep(Duration::from_millis(100)).await;
            return Ok(());
        }
        if self.auto_run_enabled
            && distance > 20.0
            && self.last_sprint_at.elapsed() > Duration::from_millis(2500)
        {
            self.last_sprint_at = Instant::now();
            self.device
                .mouse_click(
                    EV_KEY::BTN_RIGHT,
                    Duration::ZERO,
                    Duration::from_millis(40),
                    Duration::ZERO,
                )
                .map_err(|e| NavigateError::Device(e.to_string()))?;
        }
        sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    pub async fn move_close_to(
        &mut self,
        target_game: Point2f,
        map_name: &str,
        mis: &Misidentification,
    ) -> Result<(), NavigateError> {
        let scale = scene_by_name(map_name)
            .map(|s| s.geom.block_scale_to_1024() as f64)
            .unwrap_or(1.0);
        let unrecognized_handling = mis.kinds.iter().any(|s| s == "unrecognized");
        let mut prev_pos: Option<Point2f> = None;
        let mut steps_taken: u32 = 0;
        loop {
            steps_taken += 1;
            if steps_taken > 25 {
                break;
            }
            let mm = match self.source.capture_minimap() {
                Ok(m) => m,
                Err(_) => {
                    sleep(Duration::from_millis(50)).await;
                    continue;
                }
            };
            let pos = match self.locator.locate(&mm)? {
                Some(info) => {
                    let p = Point2f::new(info.game_pos.x, info.game_pos.y);
                    prev_pos = Some(p);
                    p
                }
                None => {
                    if unrecognized_handling
                        && mis.handling_mode == "previousDetectedPoint"
                        && let Some(p) = prev_pos
                    {
                        p
                    } else {
                        sleep(Duration::from_millis(50)).await;
                        continue;
                    }
                }
            };
            let distance = (pos.distance_to(target_game) as f64) * scale;
            if distance < 2.0 {
                self.locator.set_prev_position_game(map_name, pos);
                break;
            }
            let target_orient = target_orientation_deg(target_game, pos);
            let _ = self.rotate_until(target_orient, 2.0, Duration::from_secs(3)).await;
            self.device
                .press_keys(&[EV_KEY::KEY_W], Duration::from_millis(60))
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(20)).await;
        }
        let _ = self.device.key_up(EV_KEY::KEY_W);
        sleep(Duration::from_millis(1000)).await;
        Ok(())
    }
}
