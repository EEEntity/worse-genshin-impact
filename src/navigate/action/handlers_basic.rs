//! 基础操作实现
//! 
//! 不太灵活，基本是跟着具体任务写的

use std::time::Duration;
use evdev_rs::enums::EV_KEY;
use opencv::core::Mat;
use tokio::time::sleep;

use crate::navigate::error::NavigateError;

use super::context::{ActionContext, ActionHandler};

const INITIAL_VERTICAL_MOVEMENT: i32 = 1000;
const MOVE_DIR_CHANGE_INTERVAL: i32 = 10;
const TOTAL_CYCLES: i32 = 40;
const DELAY_BETWEEN_CYCLES_MS: u64 = 100;
const CONSECUTIVE_DETECTIONS_REQUIRED: i32 = 2;
const DETECTION_DELAY_MS: u64 = 150;

macro_rules! key_press_handler {
    ($name:ident, $log:literal, $key:expr, $wait_ms:literal) => {
        pub struct $name;
        impl ActionHandler for $name {
            fn run<'a>(
                &'a self,
                ctx: ActionContext<'a>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>,
            > {
                Box::pin(async move {
                    log::info!("action: {}", $log);
                    ctx.device
                        .press_keys(&[$key], Duration::from_millis(40))
                        .map_err(|e| NavigateError::Device(e.to_string()))?;
                    sleep(Duration::from_millis($wait_ms)).await;
                    Ok(())
                })
            }
        }
    };
}

pub struct NormalAttackHandler;
impl ActionHandler for NormalAttackHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            log::info!("action: 普通攻击");
            ctx.device
                .mouse_click(
                    EV_KEY::BTN_LEFT,
                    Duration::ZERO,
                    Duration::from_millis(40),
                    Duration::ZERO,
                )
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(1000)).await;
            Ok(())
        })
    }
}

key_press_handler!(ElementalSkillHandler, "元素战技 E", EV_KEY::KEY_E, 1000);
key_press_handler!(UseGadgetHandler, "使用小道具 Z", EV_KEY::KEY_Z, 800);

pub struct MiningHandler;
impl ActionHandler for MiningHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let count: u32 = ctx.action_params.and_then(|s| s.parse().ok()).unwrap_or(6);
            log::info!("action: mining x{count}");
            for _ in 0..count {
                ctx.device
                    .press_keys(&[EV_KEY::KEY_F], Duration::from_millis(60))
                    .map_err(|e| NavigateError::Device(e.to_string()))?;
                sleep(Duration::from_millis(120)).await;
            }
            sleep(Duration::from_millis(500)).await;
            Ok(())
        })
    }
}

pub struct PickUpCollectHandler;
impl ActionHandler for PickUpCollectHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let count: u32 = ctx.action_params.and_then(|s| s.parse().ok()).unwrap_or(8);
            log::info!("action: pick_up_collect x{count}");
            for _ in 0..count {
                ctx.device
                    .press_keys(&[EV_KEY::KEY_F], Duration::from_millis(50))
                    .map_err(|e| NavigateError::Device(e.to_string()))?;
                sleep(Duration::from_millis(160)).await;
            }
            Ok(())
        })
    }
}

pub struct PickAroundHandler;
impl ActionHandler for PickAroundHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let turns: u32 = ctx.action_params.and_then(|s| s.parse().ok()).unwrap_or(8);
            log::info!("action: pick_around turns={turns}");
            let dx = (1024 / turns.max(1) as i32).max(60);
            for i in 0..turns {
                ctx.device
                    .mouse_move(dx, 0, Duration::from_millis(120), 8)
                    .map_err(|e| NavigateError::Device(e.to_string()))?;
                sleep(Duration::from_millis(80)).await;
                ctx.device
                    .press_keys(&[EV_KEY::KEY_F], Duration::from_millis(50))
                    .map_err(|e| NavigateError::Device(e.to_string()))?;
                sleep(Duration::from_millis(120)).await;
                if i % 2 == 0 {
                    ctx.device
                        .press_keys(&[EV_KEY::KEY_W], Duration::from_millis(100))
                        .map_err(|e| NavigateError::Device(e.to_string()))?;
                    sleep(Duration::from_millis(60)).await;
                }
            }
            Ok(())
        })
    }
}

pub struct UpDownGrabLeafHandler;

impl ActionHandler for UpDownGrabLeafHandler {
    fn run<'a>(
        &'a self,
        mut ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let mut direction: i32 = 1;
            if let Some(p) = ctx.action_params {
                if !p.is_empty() {
                    direction = if p == "up" { 1 } else { -1 };
                }
            }
            let mut vertical_movement: i32 = direction * INITIAL_VERTICAL_MOVEMENT;
            let mut remaining_cycles: i32 = TOTAL_CYCLES;
            let mut consecutive_detections: i32 = 0;
            if ctx.screen.is_none() {
                ctx.device
                    .mouse_click(
                        EV_KEY::BTN_MIDDLE,
                        Duration::ZERO,
                        Duration::from_millis(40),
                        Duration::from_millis(300),
                    )
                    .map_err(|e| NavigateError::Device(e.to_string()))?;
                return Ok(());
            }
            while remaining_cycles > 0 {
                if remaining_cycles % MOVE_DIR_CHANGE_INTERVAL == 0 {
                    vertical_movement = -vertical_movement;
                }
                let detected = detect_leaf(&mut ctx)?;
                if detected {
                    consecutive_detections += 1;
                    if consecutive_detections >= CONSECUTIVE_DETECTIONS_REQUIRED {
                        interact_with_leaf(&mut ctx).await?;
                        return Ok(());
                    }
                    sleep(Duration::from_millis(DETECTION_DELAY_MS)).await;
                } else {
                    consecutive_detections = 0;
                    ctx.device
                        .mouse_move_rel(0, vertical_movement)
                        .map_err(|e| NavigateError::Device(e.to_string()))?;
                    sleep(Duration::from_millis(DELAY_BETWEEN_CYCLES_MS)).await;
                    remaining_cycles -= 1;
                }
            }
            ctx.device
                .mouse_click(
                    EV_KEY::BTN_MIDDLE,
                    Duration::ZERO,
                    Duration::from_millis(40),
                    Duration::from_millis(300),
                )
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            Ok(())
        })
    }
}

fn detect_leaf(ctx: &mut ActionContext<'_>) -> Result<bool, NavigateError> {
    let provider = match ctx.screen.as_mut() {
        Some(p) => p,
        None => return Ok(false),
    };
    let screen = match provider()? {
        Some(s) => s,
        None => return Ok(false),
    };
    {
        use opencv::core::MatTraitConst;
        if screen.empty() {
            return Ok(false);
        }
    }
    const GROUP1: [(i32, i32); 4] = [(1500, 1000), (1508, 1041), (1500, 987), (1500, 1010)];
    const GROUP2: [(i32, i32); 4] = [(1620, 1000), (1628, 1041), (1620, 987), (1620, 1010)];
    const GROUP3: [(i32, i32); 4] = [(1396, 1000), (1404, 1041), (1396, 987), (1396, 1010)];
    Ok(check_points_in_range(&screen, &GROUP1)?
        || check_points_in_range(&screen, &GROUP2)?
        || check_points_in_range(&screen, &GROUP3)?)
}

fn check_points_in_range(screen: &Mat, points: &[(i32, i32)]) -> Result<bool, NavigateError> {
    use opencv::core::{MatTraitConst, Vec3b, Vec4b};
    let ch = screen.channels();
    let h = screen.rows();
    let w = screen.cols();
    for &(x, y) in points {
        if x < 0 || y < 0 || x >= w || y >= h {
            return Ok(false);
        }
        let (b, g, r) = match ch {
            3 => {
                let p: Vec3b = *screen
                    .at_2d::<Vec3b>(y, x)
                    .map_err(|e| NavigateError::Device(format!("at_2d Vec3b: {e}")))?;
                (p[0], p[1], p[2])
            }
            4 => {
                let p: Vec4b = *screen
                    .at_2d::<Vec4b>(y, x)
                    .map_err(|e| NavigateError::Device(format!("at_2d Vec4b: {e}")))?;
                (p[0], p[1], p[2])
            }
            n => {
                return Err(NavigateError::Device(format!(
                    "check_points_in_range: 不支持的通道数 {n}"
                )));
            }
        };
        if !(b >= 245 && g >= 245 && r >= 245) {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn interact_with_leaf(ctx: &mut ActionContext<'_>) -> Result<(), NavigateError> {
    let map_err = |e: crate::device::DeviceError| NavigateError::Device(e.to_string());
    ctx.device
        .press_keys(&[EV_KEY::KEY_T], Duration::from_millis(40))
        .map_err(map_err)?;
    sleep(Duration::from_millis(200)).await;
    ctx.device
        .mouse_click(
            EV_KEY::BTN_MIDDLE,
            Duration::ZERO,
            Duration::from_millis(40),
            Duration::ZERO,
        )
        .map_err(map_err)?;
    if let Some(provider) = ctx.screen.as_mut() {
        for _ in 0..20 {
            let screen = match provider()? {
                Some(s) => s,
                None => break,
            };
            let is_flying = matches!(
                crate::navigate::bv::get_motion_status(&screen),
                Ok(crate::navigate::bv::MotionStatus::Fly)
            );
            if !is_flying {
                ctx.device
                    .press_keys(&[EV_KEY::KEY_SPACE], Duration::from_millis(40))
                    .map_err(map_err)?;
                sleep(Duration::from_millis(500)).await;
            } else {
                break;
            }
        }
    }
    sleep(Duration::from_millis(200)).await;
    Ok(())
}

pub struct StopFlyingHandler;
impl ActionHandler for StopFlyingHandler {
    fn run<'a>(
        &'a self,
        mut ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            if let Some(s) = ctx.action_params
                && let Ok(wait_ms) = s.parse::<u64>()
            {
                ctx.device
                    .press_keys(&[EV_KEY::KEY_SPACE], Duration::from_millis(40))
                    .map_err(|e| NavigateError::Device(e.to_string()))?;
                sleep(Duration::from_millis(wait_ms)).await;
                ctx.device
                    .press_keys(&[EV_KEY::KEY_SPACE], Duration::from_millis(40))
                    .map_err(|e| NavigateError::Device(e.to_string()))?;
                sleep(Duration::from_millis(300)).await;
            }
            ctx.device
                .mouse_click(
                    EV_KEY::BTN_LEFT,
                    Duration::ZERO,
                    Duration::from_millis(40),
                    Duration::ZERO,
                )
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            if let Some(provider) = ctx.screen.as_deref_mut() {
                use crate::navigate::bv::{MotionStatus, get_motion_status};
                let mut i = 0u32;
                while i < 50 {
                    let Some(screen) = provider().ok().flatten() else {
                        sleep(Duration::from_millis(300)).await;
                        i += 1;
                        continue;
                    };
                    match get_motion_status(&screen) {
                        Ok(MotionStatus::Fly) => sleep(Duration::from_millis(300)).await,
                        Ok(_) => return Ok(()),
                        Err(_) => sleep(Duration::from_millis(300)).await,
                    }
                    i += 1;
                }
                return Ok(());
            }
            sleep(Duration::from_millis(2000)).await;
            Ok(())
        })
    }
}

pub struct LogOutputHandler;
impl ActionHandler for LogOutputHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            log::info!("action: log_output {}", ctx.action_params.unwrap_or("(no params)"));
            Ok(())
        })
    }
}
