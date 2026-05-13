//! 导航中途复苏
//! 
//! 只是传送到神像，但还需要走到附近等待恢复

use std::time::Duration;
use opencv::core::Mat;
use tokio::time::sleep;

use crate::navigate::coord::Point2f;
use crate::navigate::error::NavigateError;

use super::types::{MinimapSource, PathExecutor};

impl<S: MinimapSource> PathExecutor<S> {
    async fn wait_for_main_ui(&mut self, retry_times: u32) -> Result<bool, NavigateError> {
        for _ in 0..retry_times {
            sleep(Duration::from_millis(1000)).await;
            let screen = match self.source.capture_screen()? {
                Some(s) => s,
                None => continue,
            };
            if crate::navigate::bv::is_in_main_ui(&screen)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn tp_statue_of_the_seven(&mut self, current_pos: Point2f) -> Result<Point2f, NavigateError> {
        if self.tp.is_none() {
            return Err(NavigateError::Unsupported(
                "tp_statue_of_the_seven: TpTask 未注册".into(),
            ));
        }
        let source_ptr: *mut S = &mut self.source;
        let mut provider = move || -> Result<Option<Mat>, NavigateError> {
            let s = unsafe { &mut *source_ptr };
            s.capture_screen()
        };
        let Self {
            tp,
            ocr,
            big_map,
            device,
            ..
        } = self;
        let tp_ref = tp.as_ref().unwrap();
        let mut deps = crate::navigate::tp::TpDeps {
            device,
            screen: &mut provider,
            ocr: ocr.as_mut(),
            big_map: big_map.as_mut(),
        };
        tp_ref.tp_to_statue_of_the_seven(&mut deps, current_pos).await
    }

    pub(crate) async fn recover_when_low_hp(
        &mut self,
        idx: usize,
        next_target: Point2f,
    ) -> Result<(), NavigateError> {
        let screen = match self.source.capture_screen()? {
            Some(s) => s,
            None => {
                log::warn!("[{idx}] recover_when_low_hp: 截图失败，跳过检测");
                return Ok(());
            }
        };
        if crate::navigate::bv::current_avatar_is_low_hp(&screen)? {
            let cur = self
                .try_locate_once()
                .await
                .ok()
                .flatten()
                .unwrap_or(next_target);
            self.tp_statue_of_the_seven(cur).await?;
            return Err(NavigateError::Retry(format!(
                "[{idx}] 低血量回血完成，重试本段路线"
            )));
        }
        let revive_pt = if let Some(ocr) = self.ocr.as_mut() {
            crate::navigate::bv::find_revive_modal(&screen, ocr)?
        } else {
            None
        };
        if let Some((cx, cy)) = revive_pt {
            self.device
                .teleport_mouse(cx, cy)
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(50)).await;
            self.device
                .mouse_click(
                    evdev_rs::enums::EV_KEY::BTN_LEFT,
                    Duration::ZERO,
                    Duration::from_millis(40),
                    Duration::from_millis(120),
                )
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            if !self.wait_for_main_ui(10).await? {
                log::warn!("[{idx}] 复苏后等待主界面超时（10s），继续后续流程");
            }
            sleep(Duration::from_millis(4000)).await;
            let cur = self
                .try_locate_once()
                .await
                .ok()
                .flatten()
                .unwrap_or(Point2f::new(0.0, 0.0));
            self.tp_statue_of_the_seven(cur).await?;
            return Err(NavigateError::Retry(format!("[{idx}] 复苏完成后重试本段路线")));
        }
        Ok(())
    }
}
