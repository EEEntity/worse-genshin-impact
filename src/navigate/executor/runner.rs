//! 执行路线

use evdev_rs::enums::EV_KEY;
use crate::navigate::error::NavigateError;
use crate::navigate::path::PathingTask;

use super::types::{MinimapSource, PathExecutor};

impl<S: MinimapSource> PathExecutor<S> {
    pub async fn run_pathing(&mut self, task: &PathingTask) -> Result<(), NavigateError> {
        const RETRY_TIMES: u32 = 2;
        for attempt in 1..=RETRY_TIMES {
            match self.run_pathing_once(task).await {
                Ok(()) => return Ok(()),
                Err(NavigateError::Retry(msg)) if attempt < RETRY_TIMES => {
                    log::warn!(
                        "[run_pathing] 触发重试（第 {attempt}/{RETRY_TIMES} 次）：{msg}"
                    );
                    let _ = self.device.key_up(EV_KEY::KEY_W);
                }
                Err(e) => return Err(e),
            }
        }
        Err(NavigateError::Retry(format!(
            "[run_pathing] 重试 {RETRY_TIMES} 次仍失败"
        )))
    }
}
