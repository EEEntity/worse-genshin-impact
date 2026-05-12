//! 旋转寻敌
//! 
//! 这里补充一下实现思路

use std::sync::Arc;
use std::time::Duration;
use opencv::core::{Mat, Rect, Scalar};
use opencv::imgproc::connected_components_with_stats;
use opencv::prelude::MatTraitConst;

use crate::device::action::GIAction;
// use crate::device::keybindings::MouseButton;
use crate::device::keytype::KeyType;
use crate::device::simulator::Simulator;
use crate::navigate::error::NavigateError;

use super::task::ScreenFn;

/// 旋转因子
const ROTARY_FACTOR_MAPPING: &[(i32, i32)] = &[
    (1, 100), (2, 90), (3, 80), (4, 70), (5, 60), (6, 45),
    (7, 30), (8, 15), (9, 6), (10, 1), (11, -10), (12, -50), (13, -60),
];

fn lookup_rotary(factor: i32) -> i32 {
    let f = factor.clamp(1, 13);
    ROTARY_FACTOR_MAPPING
        .iter()
        .find_map(|(k, v)| if *k == f { Some(*v) } else { None })
        .unwrap_or(45)
}

/// 红色条阈值
const BLOOD_BGR: (f64, f64, f64) = (90.0, 90.0, 255.0);
/// 寻敌ROI(1920x1080)
const SEEK_ROI_1080P: Rect = Rect { x: 0, y: 0, width: 1500, height: 900 };
/// 旋转上限
pub const ROTATION_LIMIT: u32 = 6;

/// 寻敌结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekResult {
    /// 找到敌人/已尝试拉近 -> 上层应"继续战斗"
    EnemyFound,
    /// 检测到队伍菜单的"白+黄"像素 -> 战斗确实结束
    BattleEnded,
    /// 旋转一圈仍无敌人 -> 不确定，可以用队伍读条兜底
    NoEnemy,
}
/// 寻敌状态
#[derive(Debug, Default)]
pub struct SeekState {
    /// 连续未找到敌人的次数
    pub rotation_count: u32,
}

impl SeekState {
    pub fn reset(&mut self) {
        self.rotation_count = 0;
    }
}

/// 寻敌并战斗
pub async fn seek_and_fight(
    sim: Arc<Simulator>,
    screen: &mut Option<ScreenFn>,
    state: &mut SeekState,
    detect_delay_ms: u64,
    delay_ms: u64,
    is_end_check: bool,
    rotary_factor: i32,
) -> Result<SeekResult, NavigateError> {
    let provider = match screen.as_mut() {
        Some(p) => p,
        None => {
            // 没截图源就没法寻敌，按"无敌人"反馈
            log::warn!("seek_and_fight: 无截图源，跳过寻敌");
            return Ok(SeekResult::NoEnemy);
        }
    };
    let adjusted_x = lookup_rotary(rotary_factor);
    let adjusted_divisor = if rotary_factor <= 12 { 2.0 } else { 1.3 };
    let max_retry = 25 + (adjusted_x / 5);
    let mut retry = if is_end_check { 1 } else { 0 };
    while retry < max_retry {
        // 第一次采样
        let img = match provider() {
            Some(m) => m,
            None => return Ok(SeekResult::NoEnemy),
        };
        let (img_w, img_h) = (img.cols(), img.rows());
        if let Some((_x, height)) = first_blood_stats(&img)? {
            return Ok(react_to_enemy(&sim, height, is_end_check, state).await);
        }
        // 首次重试前先开菜单像素采样，命中即真结束
        if retry == 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            log::info!("打开编队界面检查战斗是否结束");
            let _ = sim.simulate(GIAction::OpenPartySetupScreen, KeyType::KeyPress).await;
            tokio::time::sleep(Duration::from_millis(detect_delay_ms)).await;
            let menu_img = provider();
            let _ = sim.simulate(GIAction::Drop, KeyType::KeyPress).await;
            if let Some(m) = menu_img {
                if menu_white_yellow_hit(&m) {
                    log::info!("识别到战斗结束-s");
                    let _ = sim.simulate(GIAction::OpenPartySetupScreen, KeyType::KeyPress).await;
                    state.reset();
                    return Ok(SeekResult::BattleEnded);
                }
            }
        }
        // 连续3次找不到 -> 中键复位视角
        if state.rotation_count == 3 && retry == 0 {
            let _ = sim.middle_button_click();
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        // 旋转
        let (dx, dy) = if retry <= 2 {
            let offset_index = if state.rotation_count < 2 {
                0
            } else if state.rotation_count == 2 {
                1
            } else {
                2
            };
            let offsets: [(i32, i32); 4] = [
                (img_w / 6, img_h / 7),
                (img_w / 6, 0),
                (img_w / 6, -img_h / 5),
                (img_w / 6, -img_h),
            ];
            offsets[offset_index]
        } else {
            (img_w / 6, 0)
        };
        let _ = sim.move_mouse_by(dx, dy);
        let wait_ms = 50 + ((adjusted_x as f64) / adjusted_divisor) as i64;
        let wait_ms = wait_ms.max(0) as u64;
        tokio::time::sleep(Duration::from_millis(wait_ms)).await;
        // 第二次采样
        let img2 = match provider() {
            Some(m) => m,
            None => return Ok(SeekResult::NoEnemy),
        };
        if let Some((_x, height2)) = first_blood_stats(&img2)? {
            return Ok(react_to_enemy(&sim, height2, is_end_check, state).await);
        }
        retry += 1;
    }

    log::info!("寻找敌人：无");
    state.rotation_count = state.rotation_count.saturating_add(1);
    Ok(SeekResult::NoEnemy)
}

/// 红条ROI
fn first_blood_stats(img: &Mat) -> Result<Option<(i32, i32)>, NavigateError> {
    let h = img.rows();
    let w = img.cols();
    if h <= 0 || w <= 0 {
        return Ok(None);
    }
    // 按截图分辨率缩放ROI
    let scale = (h as f64 / 1080.0).min(w as f64 / 1920.0);
    let roi = Rect {
        x: 0,
        y: 0,
        width: ((SEEK_ROI_1080P.width as f64) * scale) as i32,
        height: ((SEEK_ROI_1080P.height as f64) * scale) as i32,
    };
    if roi.width <= 0 || roi.height <= 0 || roi.width > w || roi.height > h {
        return Ok(None);
    }
    let cropped = Mat::roi(img, roi).map_err(|e|NavigateError::Cv(e.to_string()))?;
    let cropped_owned = cropped.clone_pointee();
    let lower = Scalar::new(BLOOD_BGR.0, BLOOD_BGR.1, BLOOD_BGR.2, 0.0);
    let mut mask = Mat::default();
    opencv::core::in_range(&cropped_owned, &lower, &lower, &mut mask)
        .map_err(|e|NavigateError::Cv(e.to_string()))?;
    let mut labels = Mat::default();
    let mut stats = Mat::default();
    let mut centroids = Mat::default();
    let n = connected_components_with_stats(
        &mask,
        &mut labels,
        &mut stats,
        &mut centroids,
        4,
        opencv::core::CV_32S,
    )
    .map_err(|e|NavigateError::Cv(e.to_string()))?;
    if n <= 1 {
        return Ok(None);
    }
    let x = *stats.at_2d::<i32>(1, 0).map_err(|e|NavigateError::Cv(e.to_string()))?;
    let height = *stats.at_2d::<i32>(1, 3).map_err(|e|NavigateError::Cv(e.to_string()))?;
    Ok(Some((x, height)))
}

/// 命中红条后反应
async fn react_to_enemy(
    sim: &Simulator,
    height: i32,
    is_end_check: bool,
    state: &mut SeekState,
) -> SeekResult {
    if is_end_check {
        // KeyDown -> 100ms -> KeyUp
        let _ = sim.do_key(GIAction::MoveForward, KeyType::KeyDown);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let _ = sim.do_key(GIAction::MoveForward, KeyType::KeyUp);
    } else {
        let _ = sim.simulate(GIAction::MoveForward, KeyType::KeyPress).await;
        let _ = sim.simulate(GIAction::MoveForward, KeyType::KeyPress).await;
    }
    if (3..7).contains(&height) {
        // 这里应该加上网格追敌逻辑
        log::debug!("画面内疑似有敌人(height={height})，TODO: MoveForwardTask 追敌未移植");
        state.reset();
        return SeekResult::EnemyFound;
    }
    if (7..25).contains(&height) {
        state.reset();
        return SeekResult::EnemyFound;
    }
    // height<3或>25 -> 不确定
    SeekResult::NoEnemy
}

fn menu_white_yellow_hit(img: &Mat) -> bool {
    let h = img.rows();
    if h <= 0 {
        return false;
    }
    let scale = h as f64 / 1080.0;
    let py = (50.0 * scale) as i32;
    let px_bar = (790.0 * scale) as i32;
    let px_white = (768.0 * scale) as i32;
    let bar = sample_bgr(img, py, px_bar);
    let white = sample_bgr(img, py, px_white);
    is_white(white) && is_yellow(bar)
}
fn sample_bgr(img: &Mat, y: i32, x: i32) -> (u8, u8, u8) {
    use opencv::core::Vec3b;
    if y < 0 || x < 0 || y >= img.rows() || x >= img.cols() {
        return (0, 0, 0);
    }
    match img.at_2d::<Vec3b>(y, x) {
        Ok(p) => (p.0[0], p.0[1], p.0[2]),
        Err(_) => (0, 0, 0),
    }
}
fn is_yellow(bgr: (u8, u8, u8)) -> bool {
    let (b, g, r) = bgr;
    (200..=255).contains(&r) && (200..=255).contains(&g) && b <= 100
}
fn is_white(bgr: (u8, u8, u8)) -> bool {
    let (b, g, r) = bgr;
    (240..=255).contains(&r) && (240..=255).contains(&g) && (240..=255).contains(&b)
}

// seek
trait SimSyncExt {
    fn do_key(&self, action: GIAction, kt: KeyType) -> Result<(), crate::device::simulator::SimulatorError>;
}
impl SimSyncExt for Simulator {
    fn do_key(&self, action: GIAction, kt: KeyType) -> Result<(), crate::device::simulator::SimulatorError> {
        match kt {
            KeyType::KeyDown => self.key_down(action),
            KeyType::KeyUp => self.key_up(action),
            // KeyPress/Hold同步路径不应通过该helper调用
            _ => self.key_press(action),
        }
    }
}
