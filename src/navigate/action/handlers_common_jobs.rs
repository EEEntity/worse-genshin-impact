//! 通用动作

use crate::navigate::error::NavigateError;
use crate::task::common_jobs;

use super::context::{ActionContext, ActionHandler};

pub struct SetTimeHandler;
pub struct ExitAndReloginHandler;
pub struct WonderlandCycleHandler;

impl ActionHandler for SetTimeHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let sim = ctx
                .sim
                .as_deref()
                .ok_or_else(|| NavigateError::Unsupported("set_time: 缺少 Simulator".into()))?;
            let cap = ctx
                .screen_capturer
                .as_deref()
                .ok_or_else(|| NavigateError::Unsupported("set_time: 缺少 ScreenCapturer".into()))?;
            let (hour, minute, skip_animation) = parse_set_time_params(ctx.action_params)?;
            common_jobs::set_time(sim, cap, hour, minute, skip_animation)
                .await
                .map_err(|e| NavigateError::Other(format!("set_time: {e}")))
        })
    }
}

impl ActionHandler for ExitAndReloginHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let sim = ctx.sim.as_deref().ok_or_else(|| {
                NavigateError::Unsupported("exit_and_relogin: 缺少 Simulator".into())
            })?;
            let cap = ctx.screen_capturer.as_deref().ok_or_else(|| {
                NavigateError::Unsupported("exit_and_relogin: 缺少 ScreenCapturer".into())
            })?;
            common_jobs::exit_and_relogin(sim, cap)
                .await
                .map_err(|e| NavigateError::Other(format!("exit_and_relogin: {e}")))
        })
    }
}

impl ActionHandler for WonderlandCycleHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let sim = ctx.sim.as_deref().ok_or_else(|| {
                NavigateError::Unsupported("wonderland_cycle: 缺少 Simulator".into())
            })?;
            let cap = ctx.screen_capturer.as_deref().ok_or_else(|| {
                NavigateError::Unsupported("wonderland_cycle: 缺少 ScreenCapturer".into())
            })?;
            common_jobs::wonderland_cycle(sim, cap)
                .await
                .map_err(|e| NavigateError::Other(format!("wonderland_cycle: {e}")))
        })
    }
}

fn parse_set_time_params(action_params: Option<&str>) -> Result<(i32, i32, bool), NavigateError> {
    let raw = action_params.unwrap_or("").trim();
    if raw.is_empty() {
        return Ok((18, 0, true));
    }
    let mut hour: Option<i32> = None;
    let mut minute: Option<i32> = None;
    let mut skip_animation = true;
    for pair in raw.split(',') {
        let seg = pair.trim();
        if seg.is_empty() {
            continue;
        }
        let Some((k, v)) = seg.split_once('=') else {
            return Err(NavigateError::Unsupported(
                "set_time 参数格式错误，应为: h=<0-23>,m=<0-59>[,skip=true|false]".into(),
            ));
        };
        let key = k.trim().to_ascii_lowercase();
        let val = v.trim();
        match key.as_str() {
            "h" => {
                let parsed = val.parse::<i32>().map_err(|_| {
                    NavigateError::Unsupported("set_time: h 必须是 0-23 的整数".into())
                })?;
                if !(0..=23).contains(&parsed) {
                    return Err(NavigateError::Unsupported(
                        "set_time: h 必须是 0-23 的整数".into(),
                    ));
                }
                hour = Some(parsed);
            }
            "m" => {
                let parsed = val.parse::<i32>().map_err(|_| {
                    NavigateError::Unsupported("set_time: m 必须是 0-59 的整数".into())
                })?;
                if !(0..=59).contains(&parsed) {
                    return Err(NavigateError::Unsupported(
                        "set_time: m 必须是 0-59 的整数".into(),
                    ));
                }
                minute = Some(parsed);
            }
            "skip" | "skip_animation" => {
                skip_animation = parse_bool(val)?;
            }
            _ => {
                return Err(NavigateError::Unsupported(format!(
                    "set_time: 不支持的参数 `{}`，仅支持 h,m,skip",
                    key
                )));
            }
        }
    }
    let h = hour.ok_or_else(|| {
        NavigateError::Unsupported("set_time: 缺少参数 h，应为 0-23 的整数".into())
    })?;
    let m = minute.ok_or_else(|| {
        NavigateError::Unsupported("set_time: 缺少参数 m，应为 0-59 的整数".into())
    })?;
    Ok((h, m, skip_animation))
}

fn parse_bool(v: &str) -> Result<bool, NavigateError> {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "y" => Ok(true),
        "false" | "0" | "no" | "n" => Ok(false),
        _ => Err(NavigateError::Unsupported(
            "set_time: skip 必须是 true/false（或 1/0）".into(),
        )),
    }
}
