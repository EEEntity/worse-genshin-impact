//! 钓鱼动作

use std::path::PathBuf;
use std::sync::Arc;
use opencv::core::Mat;

use crate::config::AutoFishingGlobalConfig;
use crate::navigate::error::NavigateError;
use crate::task::fishing::{AutoFishingTask, ScreenFn as FishScreenFn};
use crate::task::choose_talk_option::OcrHandle;

use super::context::{ActionContext, ActionHandler};

pub struct FishingHandler {
    pub ocr: OcrHandle,
    pub config: AutoFishingGlobalConfig,
    pub model_path: PathBuf,
    pub set_time_cap: Option<Arc<dyn crate::task::choose_talk_option::ScreenCapturer>>,
}

impl ActionHandler for FishingHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let Some(sim) = ctx.sim.clone() else {
                return Err(NavigateError::Unsupported(
                    "fishing 需要 Simulator (ctx.sim) 未注入".into(),
                ));
            };
            let Some(cancel) = ctx.cancel.clone() else {
                return Err(NavigateError::Unsupported(
                    "fishing 需要 CancelFlag (ctx.cancel) 未注入".into(),
                ));
            };
            let Some(screen_provider) = ctx.screen else {
                return Err(NavigateError::Unsupported(
                    "fishing 需要 ScreenProvider (ctx.screen) 未注入".into(),
                ));
            };
            type ProviderObj = dyn FnMut() -> Result<Option<Mat>, NavigateError> + 'static;
            let provider_ptr: *mut ProviderObj = unsafe {
                std::mem::transmute::<
                    *mut (dyn FnMut() -> Result<Option<Mat>, NavigateError> + 'a),
                    *mut ProviderObj,
                >(screen_provider as *mut _)
            };
            struct SendPtr(*mut ProviderObj);
            unsafe impl Send for SendPtr {}
            let send_ptr = SendPtr(provider_ptr);
            let screen_fn: FishScreenFn = Box::new(move || {
                let SendPtr(p) = &send_ptr;
                unsafe { (**p)().ok().flatten() }
            });
            let task_cfg = self.config.to_task_config();
            let mut task = AutoFishingTask::with_model(
                sim,
                cancel,
                self.ocr.clone(),
                task_cfg,
                &self.model_path,
            )
            .map_err(|e|NavigateError::Other(format!("AutoFishingTask init: {e}")))?;
            let cap_ref = self
                .set_time_cap
                .as_deref()
                .map(|c| c as &dyn crate::task::choose_talk_option::ScreenCapturer);
            task.run(screen_fn, cap_ref)
                .await
                .map_err(|e|NavigateError::Other(format!("AutoFishingTask: {e}")))?;
            Ok(())
        })
    }
}
