//! 自动钓鱼各阶段状态机

use std::time::{Duration, Instant};
use evdev_rs::enums::EV_KEY;
use opencv::core::{Mat, Rect};
use opencv::prelude::MatTraitConst;

use crate::device::action::GIAction;
use crate::device::keytype::KeyType;
use crate::device::simulator::Simulator;
use crate::navigate::bv::matcher::{find_template, matches};

use super::fish_types::{BaitType, Fishpond, OneFish};
use super::recognition::{get_fish_bar_rects, match_fish_bite_words};
use super::rod_net::{RodInput, RodState, get_rod_state};
use super::{AutoFishingConfig, AutoFishingError, AutoFishingTask, ScreenFn, assets};

/// 判断该鱼是否应该钓
fn is_target_fish(cfg: &AutoFishingConfig, f: &OneFish) -> bool {
    cfg.fish_allowed(f.fish_type.name) && cfg.bait_allowed(f.fish_type.bait)
}

/// 共享状态
#[derive(Debug, Default)]
pub struct Blackboard {
    /// 当前选中的鱼饵
    pub selected_bait: Option<BaitType>,
    /// 当前识别到的鱼塘
    pub fishpond: Option<Fishpond>,
    /// 抛竿失败次数(无落点)
    pub throw_rod_no_target_times: u32,
    /// 抛竿目标鱼失败次数(按饵分组)
    pub throw_rod_no_bait_fish_failures: Vec<BaitType>,
    /// 钓鱼条ROI(原图坐标系)
    pub fish_box_rect: Option<Rect>,
    /// 是否需要重置视角(俯视)
    pub pitch_reset: bool,
    /// 是否要中止本轮
    pub abort: bool,
    /// 同一饵累计>=MAX_FAILED_TIMES后续直接忽略
    pub choose_bait_failures: Vec<BaitType>,
}

impl Blackboard {
    pub fn reset(&mut self) {
        *self = Self::default();
        self.pitch_reset = true;
    }
}

fn capture(screen: &mut ScreenFn) -> Result<Mat, AutoFishingError> {
    screen().ok_or(AutoFishingError::Capture)
}

fn dev_err(e: impl std::fmt::Display) -> AutoFishingError {
    AutoFishingError::Device(e.to_string())
}

fn click_at(sim: &Simulator, x: i32, y: i32) -> Result<(), AutoFishingError> {
    sim.device().teleport_mouse(x, y).map_err(dev_err)?;
    std::thread::sleep(Duration::from_millis(20));
    sim.left_button_click().map_err(dev_err)
}

fn press_esc(sim: &Simulator) -> Result<(), AutoFishingError> {
    sim.device()
        .press_keys(&[EV_KEY::KEY_ESC], Duration::from_millis(50))
        .map_err(dev_err)
}

/// 调整视角为俯视
pub async fn adjust_view_down(task: &mut AutoFishingTask) -> Result<(), AutoFishingError> {
    if task.bb.pitch_reset {
        log::info!("调整视角至俯视");
        task.bb.pitch_reset = false;
        task.sim.move_mouse_by(0, 500).map_err(dev_err)?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok(())
}

/// 找鱼
pub async fn find_fish(
    task: &mut AutoFishingTask,
    screen: &mut ScreenFn,
    deadline: Instant,
    inner_timeout: Duration,
) -> Result<bool, AutoFishingError> {
    let inner_deadline = Instant::now() + inner_timeout;
    log::info!("开始寻找鱼塘(最长 {}s)", inner_timeout.as_secs());
    let mut tick: u32 = 0;
    let mut last_diag = Instant::now();
    loop {
        if task.cancelled() {
            return Err(AutoFishingError::Cancelled);
        }
        if Instant::now() >= inner_deadline.min(deadline) {
            log::warn!("找鱼超时，退出钓鱼界面");
            return Ok(false);
        }
        let img = capture(screen)?;
        let dets = task
            .predictor
            .detect(&img)
            .map_err(|e| AutoFishingError::ModelLoad(e.to_string()))?;
        // 每~2s打一行，无检测时帮助判断是YOLO没识别到
        // 或是有检测但Fishpond把它过滤掉了
        if last_diag.elapsed() >= Duration::from_millis(2000) {
            if dets.is_empty() {
                log::info!("[find_fish#{tick}] YOLO 无任何检测");
            } else {
                let mut counts: std::collections::BTreeMap<&str, (u32, f32)> =
                    std::collections::BTreeMap::new();
                for d in &dets {
                    let e = counts.entry(d.label.as_str()).or_insert((0, 0.0));
                    e.0 += 1;
                    if d.score > e.1 {
                        e.1 = d.score;
                    }
                }
                let summary = counts
                    .iter()
                    .map(|(k, (n, s))| format!("{k}×{n}({s:.2})"))
                    .collect::<Vec<_>>()
                    .join(", ");
                log::info!("[find_fish#{tick}] YOLO 共 {} 个检测：{summary}", dets.len());
            }
            last_diag = Instant::now();
        }
        tick += 1;
        if !dets.is_empty() {
            let fp = Fishpond::from_detections(&dets, img.cols(), img.rows(), false, false);
            if let Some(fr) = fp.fishpond_rect {
                log::info!(
                    "定位到鱼塘：{}",
                    summarize_fishes(&fp.fishes)
                );
                let one_fourth = img.cols() / 4;
                let three_fourth = img.cols() * 3 / 4;

                if fr.x > three_fourth {
                    task.sim.move_mouse_by(100, 0).map_err(dev_err)?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                } else if fr.x + fr.width < one_fourth {
                    task.sim.move_mouse_by(-100, 0).map_err(dev_err)?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                // 让人物朝向与镜头一致
                // 打断待机动作
                task.sim
                    .device()
                    .press_keys(&[EV_KEY::KEY_S], Duration::from_millis(100))
                    .map_err(dev_err)?;
                tokio::time::sleep(Duration::from_millis(400)).await;
                task.sim
                    .device()
                    .press_keys(&[EV_KEY::KEY_W], Duration::from_millis(100))
                    .map_err(dev_err)?;
                tokio::time::sleep(Duration::from_millis(700)).await;
                log::info!("视角调整完毕");
                task.bb.fishpond = Some(fp);
                return Ok(true);
            }
        }
        // 转动视角
        task.sim.move_mouse_by(100, 0).map_err(dev_err)?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn summarize_fishes(fishes: &[OneFish]) -> String {
    use std::collections::BTreeMap;
    let mut group: BTreeMap<&'static str, u32> = BTreeMap::new();
    for f in fishes {
        *group.entry(f.fish_type.chinese_name).or_insert(0) += 1;
    }
    group
        .into_iter()
        .map(|(k, v)| format!("{k}{v}条"))
        .collect::<Vec<_>>()
        .join("、")
}

/// 进入钓鱼模式
/// 
/// F -> 确认 -> 看到ExitFishingButton
pub async fn enter_fishing_mode(
    task: &mut AutoFishingTask,
    screen: &mut ScreenFn,
    overall: Duration,
) -> Result<bool, AutoFishingError> {
    let overall_deadline = Instant::now() + overall;
    let mut press_f_until: Option<Instant> = None;
    let mut click_confirm_until: Option<Instant> = None;
    log::info!("进入钓鱼模式");
    loop {
        if task.cancelled() {
            return Err(AutoFishingError::Cancelled);
        }
        if Instant::now() >= overall_deadline {
            log::warn!("进入钓鱼模式失败（10s 超时）");
            return Ok(false);
        }
        let img = capture(screen)?;
        // 找F提示 -> OCR校验文字 == "钓鱼" -> 按 F
        if press_f_until.map_or(true, |t| Instant::now() >= t) {
            if let Some(f) = find_template(&img, assets::pick_f()?)? {
                // 文字ROI
                const ITEM_TEXT_LEFT_OFFSET: i32 = 115;
                const ITEM_TEXT_RIGHT_OFFSET: i32 = 400;
                let scr_w = img.cols();
                let scr_h = img.rows();
                let tx = (f.top_left.x + ITEM_TEXT_LEFT_OFFSET).clamp(0, scr_w - 1);
                let ty = f.top_left.y.clamp(0, scr_h - 1);
                let tw = (ITEM_TEXT_RIGHT_OFFSET - ITEM_TEXT_LEFT_OFFSET).min(scr_w - tx);
                let th = f.height.min(scr_h - ty);
                if tw > 0 && th > 0 {
                    let text_rect = Rect { x: tx, y: ty, width: tw, height: th };
                    let sub = Mat::roi(&img, text_rect).map_err(|e| {
                        AutoFishingError::Navigate(crate::navigate::error::NavigateError::Cv(e.to_string()))
                    })?;
                    let sub_owned = sub.clone_pointee();
                    let text = ocr_region(&task.ocr, &sub_owned, tw, th)?;
                    if text.contains("钓鱼") {
                        task.sim
                            .simulate(GIAction::PickUpOrInteract, KeyType::KeyPress)
                            .await
                            .map_err(dev_err)?;
                        log::info!("按下钓鱼键F(OCR校验通过：\"{text}\")");
                        press_f_until = Some(Instant::now() + Duration::from_secs(3));
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        continue;
                    } else {
                        log::debug!("F模板命中但文字校验失败：\"{text}\"，跳过本帧");
                    }
                }
            }
        }
        // 钓鱼准备界面 "开始钓鱼"
        if click_confirm_until.map_or(true, |t| Instant::now() >= t) {
            if let Some(m) = find_template(&img, assets::btn_white_confirm()?)? {
                let cx = m.center().x;
                let cy = m.center().y;
                log::info!(
                    "命中开始钓鱼按钮：score={:.3} center=({},{}) size={}×{}",
                    m.score, cx, cy, m.width, m.height
                );
                // ROI -> GridIcons.Infer
                // 选择鱼饵
                if task.bb.selected_bait.is_none() {
                    if let Some(predictor) = task.grid_icon.as_mut() {
                        let w = img.cols();
                        let h = img.rows();
                        let side = (0.065 * w as f64) as i32;
                        let bait_roi = Rect {
                            x: (0.824 * w as f64) as i32,
                            y: (0.669 * h as f64) as i32,
                            width: side,
                            height: side,
                        };
                        if bait_roi.x >= 0
                            && bait_roi.y >= 0
                            && bait_roi.x + bait_roi.width <= w
                            && bait_roi.y + bait_roi.height <= h
                        {
                            match Mat::roi(&img, bait_roi) {
                                Ok(roi) => {
                                    let icon = roi.clone_pointee();
                                    match predictor.infer(&icon) {
                                        Ok(Some(name)) => {
                                            if let Some(b) = BaitType::from_chinese_name(&name) {
                                                task.bb.selected_bait = Some(b);
                                                log::info!(
                                                    "GridIcons识别到当前装备饵：{} ({})",
                                                    b.chinese_name(),
                                                    name
                                                );
                                            } else {
                                                log::info!(
                                                    "GridIcons识别结果非饵类：{}（视为未识别）",
                                                    name
                                                );
                                            }
                                        }
                                        Ok(None) => log::info!("GridIcons未识别出饵（距离超阈值）"),
                                        Err(e) => log::warn!("GridIcons推断失败：{e}"),
                                    }
                                }
                                Err(e) => log::warn!("饵图标ROI切片失败：{e}"),
                            }
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
                click_at(&task.sim, cx, cy)?;
                log::info!(
                    "点击开始钓鱼，当前装备的鱼饵：{}",
                    task.bb
                        .selected_bait
                        .map(|b| b.chinese_name())
                        .unwrap_or("未识别")
                );
                task.bb.pitch_reset = true;
                click_confirm_until = Some(Instant::now() + Duration::from_secs(3));
                tokio::time::sleep(Duration::from_millis(300)).await;
                continue;
            }
        }
        // 看到右下ExitFishingButton -> 已进入钓鱼
        if matches(&img, assets::exit_fishing_button()?)? {
            log::info!("→ 已进入钓鱼模式");
            return Ok(true);
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

/// 内层找鱼塘
pub async fn find_fish_with_initial_check(
    task: &mut AutoFishingTask,
    screen: &mut ScreenFn,
    timeout: Duration,
) -> Result<bool, AutoFishingError> {
    let deadline = Instant::now() + timeout;
    // CheckInitalState螺旋找BaitButton
    log::info!("寻找换饵图标 (确认在钓鱼界面)");
    let mut theta: f64 = 0.0;
    loop {
        if task.cancelled() {
            return Err(AutoFishingError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        let img = capture(screen)?;
        if matches(&img, assets::bait_button()?)? {
            break;
        }
        theta += std::f64::consts::PI / 10.0;
        let rho = 10.0 + 2.0 * theta;
        let dx = (rho * theta.cos()) as i32;
        let dy = (rho * theta.sin()) as i32;
        task.sim.move_mouse_by(dx, dy).map_err(dev_err)?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // GetFishpond
    // `ignore_obtained=true`的YOLO识别
    log::info!("开始寻找鱼塘 (内层)");
    loop {
        if task.cancelled() {
            return Err(AutoFishingError::Cancelled);
        }
        if Instant::now() >= deadline {
            log::warn!("内层 GetFishpond 超时");
            return Ok(false);
        }
        let img = capture(screen)?;
        let dets = task
            .predictor
            .detect(&img)
            .map_err(|e|AutoFishingError::ModelLoad(e.to_string()))?;
        let fp = Fishpond::from_detections(&dets, img.cols(), img.rows(), false, true);
        if fp.fishpond_rect.is_some() {
            log::info!("定位到鱼塘：{}", summarize_fishes(&fp.fishes));
            // 白名单过滤后是否有可钓目标鱼
            // 只要有目标鱼就进入后续choose_bait
            // 不因"当前饵不匹配"提前结束本轮
            let any_target = fp
                .fishes
                .iter()
                .any(|f|is_target_fish(&task.config, f));
            task.bb.fishpond = Some(fp);
            tokio::time::sleep(Duration::from_millis(700)).await;
            if any_target {
                return Ok(true);
            } else {
                log::warn!("鱼塘内没有白名单允许的鱼，继续等待");
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// 选择鱼饵
pub async fn choose_bait(
    task: &mut AutoFishingTask,
    screen: &mut ScreenFn,
) -> Result<bool, AutoFishingError> {
    use opencv::core::Vector;
    use opencv::imgproc::{
        self, COLOR_BGR2GRAY, CHAIN_APPROX_SIMPLE, RETR_EXTERNAL,
    };
    // 重试次数应该扔出去
    const MAX_FAILED_TIMES: usize = 2;
    let Some(pond) = task.bb.fishpond.as_ref() else {
        log::warn!("choose_bait: fishpond 为空，跳过");
        return Ok(true);
    };
    if pond.fishes.is_empty() {
        return Ok(true);
    }
    // 仅看白名单允许(且鱼饵也允许)的鱼
    let target_fishes: Vec<&OneFish> = pond
        .fishes
        .iter()
        .filter(|f| is_target_fish(&task.config, f))
        .collect();
    if target_fishes.is_empty() {
        log::warn!("choose_bait: 鱼塘内无白名单允许的鱼，跳过");
        return Ok(false);
    }
    // 早返回：当前装备的饵还能钓到目标鱼
    if let Some(cur) = task.bb.selected_bait {
        if task.config.bait_allowed(cur)
            && target_fishes.iter().any(|f| f.fish_type.bait == cur)
        {
            return Ok(true);
        }
    }
    // 选最佳饵：按"该饵能钓到的目标鱼数"分组
    // 剔除失败>=MAX的
    use std::collections::HashMap;
    let mut counts: HashMap<BaitType, usize> = HashMap::new();
    for f in &target_fishes {
        *counts.entry(f.fish_type.bait).or_insert(0) += 1;
    }
    let failed_counts: HashMap<BaitType, usize> = {
        let mut m = HashMap::new();
        for b in &task.bb.choose_bait_failures {
            *m.entry(*b).or_insert(0) += 1;
        }
        m
    };
    let mut candidates: Vec<(BaitType, usize)> = counts
        .into_iter()
        .filter(|(b, _)|task.config.bait_allowed(*b))
        .filter(|(b, _)|failed_counts.get(b).copied().unwrap_or(0) < MAX_FAILED_TIMES)
        .collect();
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    let Some(&(target, _n)) = candidates.first() else {
        log::warn!("choose_bait: 候选饵全部失败次数超限或不在白名单，跳过本轮");
        return Ok(false);
    };
    task.bb.selected_bait = Some(target);
    log::info!("选择鱼饵 {}", target.chinese_name());
    // GridIcons未加载 -> 直接放弃 UI 操作
    if task.grid_icon.is_none() {
        log::warn!(
            "GridIcons 未加载，无法自动切换为 {}，沿用当前装备饵继续抛竿",
            target.chinese_name()
        );
        return Ok(false);
    }
    // 打开换饵UI
    log::info!("打开换饵界面");
    task.sim.right_button_down().map_err(dev_err)?;
    tokio::time::sleep(Duration::from_millis(50)).await;
    task.sim.right_button_up().map_err(dev_err)?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    task.sim.move_mouse_by(0, 200).map_err(dev_err)?; // 移开鼠标避免遮挡
    tokio::time::sleep(Duration::from_millis(500)).await;
    // ~3s内反复在UI中找目标饵
    // 也许以后还得拖动
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut clicked_target = false;
    while Instant::now() < deadline {
        if task.cancelled() {
            press_esc(&task.sim)?;
            return Err(AutoFishingError::Cancelled);
        }
        let img = capture(screen)?;
        let w = img.cols();
        let h = img.rows();
        // ROI = (0.28w, 0.37h, 0.45w, 0.22h)
        let row_rect = Rect {
            x: (0.28 * w as f64) as i32,
            y: (0.37 * h as f64) as i32,
            width: (0.45 * w as f64) as i32,
            height: (0.22 * h as f64) as i32,
        };
        let row = Mat::roi(&img, row_rect)
            .map_err(|e|AutoFishingError::Navigate(crate::navigate::error::NavigateError::Cv(e.to_string())))?
            .clone_pointee();
        let mut grey = Mat::default();
        imgproc::cvt_color(
            &row,
            &mut grey,
            COLOR_BGR2GRAY,
            0,
            opencv::core::AlgorithmHint::ALGO_HINT_DEFAULT,
        )
        .map_err(|e|AutoFishingError::Navigate(crate::navigate::error::NavigateError::Cv(e.to_string())))?;
        let mut canny = Mat::default();
        imgproc::canny(&grey, &mut canny, 20.0, 40.0, 3, false)
            .map_err(|e|AutoFishingError::Navigate(crate::navigate::error::NavigateError::Cv(e.to_string())))?;
        let mut contours: Vector<Vector<opencv::core::Point>> = Vector::new();
        imgproc::find_contours(
            &canny,
            &mut contours,
            RETR_EXTERNAL,
            CHAIN_APPROX_SIMPLE,
            opencv::core::Point::new(0, 0),
        )
        .map_err(|e|AutoFishingError::Navigate(crate::navigate::error::NavigateError::Cv(e.to_string())))?;
        let min_w = 0.065 * w as f64 * 0.80;
        for c in contours.iter() {
            let r = imgproc::bounding_rect(&c)
                .map_err(|e|AutoFishingError::Navigate(crate::navigate::error::NavigateError::Cv(e.to_string())))?;
            if (r.width as f64) < min_w {
                continue;
            }
            if r.height == 0 {
                continue;
            }
            let ratio = r.width as f64 / r.height as f64;
            if (ratio - 0.81).abs() >= 0.05 {
                continue;
            }
            // 裁出该格子
            let cell = Mat::roi(&row, r)
                .map_err(|e| AutoFishingError::Navigate(crate::navigate::error::NavigateError::Cv(e.to_string())))?
                .clone_pointee();
            let icon125 = grid_icon_crop(&cell)?;
            let predictor = task.grid_icon.as_mut().unwrap();
            let pred = predictor
                .infer(&icon125)
                .map_err(|e| AutoFishingError::Navigate(crate::navigate::error::NavigateError::Other(format!("gridIcon infer: {e}"))))?;
            let Some(pred_name) = pred else { continue };
            let Some(matched) = BaitType::from_chinese_name(&pred_name) else {
                continue;
            };
            if matched != target {
                continue;
            }
            // 命中：在原图坐标系中计算中心点并点击
            let cx = row_rect.x + r.x + r.width / 2;
            let cy = row_rect.y + r.y + r.height / 2;
            click_at(&task.sim, cx, cy)?;
            tokio::time::sleep(Duration::from_millis(700)).await;
            // 防止重复点击产生的菜单：固定点击(0.675w, h/3)
            click_at(
                &task.sim,
                (0.675 * w as f64) as i32,
                h / 3,
            )?;
            tokio::time::sleep(Duration::from_millis(200)).await;
            // 点击右下白色确认
            let img2 = capture(screen)?;
            if let Some(m) = find_template(&img2, assets::btn_white_confirm()?)? {
                click_at(&task.sim, m.center().x, m.center().y)?;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
            log::info!("退出换饵界面（已切换为 {}）", target.chinese_name());
            clicked_target = true;
            break;
        }
        if clicked_target {
            return Ok(true);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    // 3s内没找到 -> ESC + 记录失败
    log::warn!("没有找到目标鱼饵 {}", target.chinese_name());
    press_esc(&task.sim)?;
    task.bb.choose_bait_failures.push(target);
    let cnt = task
        .bb
        .choose_bait_failures
        .iter()
        .filter(|b| **b == target)
        .count();
    if cnt >= MAX_FAILED_TIMES {
        log::warn!("本次将忽略 {}", target.chinese_name());
    }
    task.bb.selected_bait = None;
    Ok(false)
}

fn grid_icon_crop(mat: &Mat) -> Result<Mat, AutoFishingError> {
    use opencv::imgproc::{INTER_LINEAR, resize};
    let mut resized = Mat::default();
    resize(
        mat,
        &mut resized,
        opencv::core::Size::new(125, 153),
        0.0,
        0.0,
        INTER_LINEAR,
    )
    .map_err(|e| AutoFishingError::Navigate(crate::navigate::error::NavigateError::Cv(e.to_string())))?;
    let icon = Mat::roi(
        &resized,
        Rect { x: 0, y: 0, width: 125, height: 125 },
    )
    .map_err(|e| AutoFishingError::Navigate(crate::navigate::error::NavigateError::Cv(e.to_string())))?
    .clone_pointee();
    Ok(icon)
}

/// 抛竿
pub async fn throw_rod_until_success(
    task: &mut AutoFishingTask,
    screen: &mut ScreenFn,
) -> Result<bool, AutoFishingError> {
    log::info!("长按举起鱼竿");
    task.sim.left_button_down().map_err(dev_err)?;
    // 每次抛竿失败后若都强制再次俯视下压
    // 可能在多次重试后把镜头逐步压偏(看向角色而非鱼塘)
    // 因此不在每次ThrowRod初始化时重置pitch
    // 避免累积下压
    let started = Instant::now();
    let ignore_obtained_end = started + Duration::from_secs(6);
    let find_target_end = started + Duration::from_secs(5);
    let mut found_target = false;
    let mut no_placement_times = 0u32;
    let mut no_target_fish_times = 0u32;
    let mut mouse_move_i: i32 = 1;
    let mut mouse_move_r: f64 = 0.0;
    mouse_move_i *= -1;
    let mut rng_state: u64 = 0x12345;
    loop {
        if task.cancelled() {
            task.sim.left_button_up().map_err(dev_err)?;
            return Err(AutoFishingError::Cancelled);
        }
        let img = capture(screen)?;
        let dets = task
            .predictor
            .detect(&img)
            .map_err(|e| AutoFishingError::ModelLoad(e.to_string()))?;
        let include_target = Instant::now() <= ignore_obtained_end;
        let fp = Fishpond::from_detections(&dets, img.cols(), img.rows(), include_target, false);
        task.bb.fishpond = Some(fp.clone());
        let target_rect = match fp.target_rect {
            Some(r) => r,
            None => {
            // 没有落点
            if !found_target {
                if Instant::now() <= find_target_end {
                    // 上下移动视角
                    mouse_move_r += std::f64::consts::PI / 16.0;
                    let sign = mouse_move_r.cos().signum() as i32;
                    task.sim.move_mouse_by(0, mouse_move_i * 80 * sign).map_err(dev_err)?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                } else {
                    log::warn!("举起鱼竿失败，始终没有找到落点");
                    task.sim.left_button_up().map_err(dev_err)?;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    task.sim.left_button_click().map_err(dev_err)?;
                    tokio::time::sleep(Duration::from_millis(800)).await;
                    task.bb.throw_rod_no_target_times += 1;
                    if task.bb.throw_rod_no_target_times > 2 {
                        log::warn!("没有找到落点次数过多，可能视野不佳，退出");
                        task.bb.abort = true;
                    }
                    return Ok(false);
                }
            }
                no_placement_times += 1;
                tokio::time::sleep(Duration::from_millis(50)).await;
                // 随机移动鼠标
                let (dx, dy) = random_mouse_offset(&mut rng_state, img.cols(), img.rows());
                task.sim.move_mouse_by(dx, dy).map_err(dev_err)?;
                if no_placement_times > 25 {
                    log::info!("中途丢失鱼饵落点，重试");
                    task.sim.left_button_up().map_err(dev_err)?;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    task.sim.left_button_click().map_err(dev_err)?;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    return Ok(false);
                }
                continue;
            }
        };
        found_target = true;
        // 找最近按饵筛选的鱼
        let bait = task.bb.selected_bait;
        let mut candidates: Vec<&OneFish> = fp
            .fishes
            .iter()
            .filter(|f| bait.is_none_or(|b| f.fish_type.bait == b))
            .collect();
        // 按距离落点中心排序
        let target_cx = target_rect.x as f32 + target_rect.width as f32 / 2.0;
        let target_cy = target_rect.y as f32 + target_rect.height as f32 / 2.0;
        candidates.sort_by(|a, b| {
            let da = dist_to_center(&a.rect, target_cx, target_cy);
            let db = dist_to_center(&b.rect, target_cx, target_cy);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        let current_fish = candidates.first().copied();
        if current_fish.is_none() {
            no_target_fish_times += 1;
            if no_target_fish_times > 10 {
                log::warn!("没有找到鱼饵适用鱼，松手重抛");
                if let Some(b) = bait {
                    task.bb.throw_rod_no_bait_fish_failures.push(b);
                }
                task.sim.left_button_up().map_err(dev_err)?;
                tokio::time::sleep(Duration::from_secs(2)).await;
                task.sim.left_button_click().map_err(dev_err)?;
                tokio::time::sleep(Duration::from_millis(800)).await;
                return Ok(true); // BGI 这里 Succeeded → 进 CheckThrowRod
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        }
        let fish = current_fish.unwrap();
        // RodNet计算
        let img_w = img.cols() as f64;
        let img_h = img.rows() as f64;
        let nx = |x: i32| x as f64 / img_w * 1024.0;
        let ny = |y: i32| y as f64 / img_h * 576.0;
        let rod = target_rect;
        let fr = fish.rect;
        let dx = (nx(fr.x) + nx(fr.x + fr.width) - nx(rod.x) - nx(rod.x + rod.width)) / 2.0;
        let dy = (ny(fr.y) + ny(fr.y + fr.height) - ny(rod.y) - ny(rod.y + rod.height)) / 2.0;
        let dl = (dx * dx + dy * dy).sqrt();
        let input = RodInput {
            rod_x1: nx(rod.x),
            rod_x2: nx(rod.x + rod.width),
            rod_y1: ny(rod.y),
            rod_y2: ny(rod.y + rod.height),
            fish_x1: nx(fr.x),
            fish_x2: nx(fr.x + fr.width),
            fish_y1: ny(fr.y),
            fish_y2: ny(fr.y + fr.height),
            fish_label: fish.fish_type.net_index as usize,
        };
        let state = get_rod_state(&input);
        match state {
            RodState::JustRight => {
                task.sim.left_button_up().map_err(dev_err)?;
                log::info!("尝试钓取 {}", fish.fish_type.chinese_name);
                return Ok(true);
            }
            RodState::TooClose => {
                let mdx = if dl > 1e-6 { dx / dl * 30.0 } else { 0.0 };
                let mdy = if dl > 1e-6 { dy / dl * 30.0 } else { 0.0 };
                task.sim
                    .move_mouse_by((-mdx / 1.5) as i32, (-mdy * 1.5) as i32)
                    .map_err(dev_err)?;
            }
            RodState::TooFar => {
                task.sim
                    .move_mouse_by((dx / 1.5) as i32, (dy * 1.5) as i32)
                    .map_err(dev_err)?;
            }
        }
        tokio::time::sleep(Duration::from_millis(dl.max(20.0).min(500.0) as u64)).await;
    }
}

fn dist_to_center(r: &Rect, cx: f32, cy: f32) -> f32 {
    let rcx = r.x as f32 + r.width as f32 / 2.0;
    let rcy = r.y as f32 + r.height as f32 / 2.0;
    ((rcx - cx).powi(2) + (rcy - cy).powi(2)).sqrt()
}

fn random_mouse_offset(state: &mut u64, w: i32, h: i32) -> (i32, i32) {
    // 简单 LCG（避免引 rand 依赖）
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let rx = (*state >> 33) as i32 & 0x7fff_ffff;
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let ry = (*state >> 33) as i32 & 0x7fff_ffff;
    let cx = w / 2;
    let cy = h / 2;
    let dx = 100 * (cx - rx % w.max(1)) / w.max(1);
    let dy = 100 * (cy - ry % h.max(1)) / h.max(1);
    (dx, dy)
}

/// 检查抛竿结果
pub async fn check_throw_rod(
    _task: &mut AutoFishingTask,
    screen: &mut ScreenFn,
    delay: Duration,
) -> Result<bool, AutoFishingError> {
    tokio::time::sleep(delay).await;
    let img = capture(screen)?;
    if matches(&img, assets::bait_button()?)? {
        log::warn!("抛竿失败（仍能看到换饵按钮）");
        Ok(false)
    } else {
        Ok(true)
    }
}

/// 等咬钩时自动提竿
pub async fn wait_for_bite(
    task: &mut AutoFishingTask,
    screen: &mut ScreenFn,
    bite_timeout: Duration,
) -> Result<(), AutoFishingError> {
    log::info!("提竿识别开始（超时 {}s）", bite_timeout.as_secs());
    let mut deadline = Instant::now() + bite_timeout;
    let mut left_clicked = false;
    loop {
        if task.cancelled() {
            return Err(AutoFishingError::Cancelled);
        }
        if Instant::now() >= deadline {
            if left_clicked {
                log::info!("收杆成功（已点击左键，无后续咬钩）");
                return Ok(());
            } else {
                log::info!("{}s 没有咬钩，本次收杆", bite_timeout.as_secs());
                left_clicked = true;
                task.sim.left_button_click().map_err(dev_err)?;
                deadline = Instant::now() + Duration::from_secs(2);
                continue;
            }
        }
        let img = capture(screen)?;
        let img_w = img.cols();
        let img_h = img.rows();
        let lifting = Rect {
            x: img_w / 3,
            y: 0,
            width: img_w / 3,
            height: img_h / 2,
        };
        let sub = Mat::roi(&img, lifting).map_err(|e| AutoFishingError::Navigate(crate::navigate::error::NavigateError::Cv(e.to_string())))?;
        let sub_owned = sub.clone_pointee();
        // HSV白字
        if let Some(_r) = match_fish_bite_words(&sub_owned, lifting)? {
            return raise_rod(&task.sim, "文字块").await;
        }
        // 模板：LiftRodButton
        if matches(&img, assets::lift_rod_button()?)? {
            return raise_rod(&task.sim, "图像识别").await;
        }
        // OCR含"上钩"
        if let Ok(text) = ocr_region(&task.ocr, &sub_owned, lifting.width, lifting.height) {
            let stripped: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            if stripped.contains("上钩") {
                return raise_rod(&task.sim, "OCR").await;
            }
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
}

async fn raise_rod(sim: &Simulator, method: &str) -> Result<(), AutoFishingError> {
    sim.left_button_click().map_err(dev_err)?;
    log::info!("---");
    log::info!("自动提竿({method})");
    Ok(())
}

fn ocr_region(
    ocr: &super::OcrHandle,
    sub_bgr: &Mat,
    w: i32,
    h: i32,
) -> Result<String, AutoFishingError> {
    let rgb = crate::navigate::tp::bgr_mat_to_rgb_bytes(sub_bgr)
        .map_err(|e|AutoFishingError::Navigate(e))?;
    let mut g = ocr.lock().expect("ocr mutex poisoned");
    let res = g
        .run(&rgb, w as u32, h as u32)
        .map_err(|e|AutoFishingError::Navigate(crate::navigate::error::NavigateError::Other(format!("ocr_region: {e}"))))?;
    drop(g);
    Ok(res
        .into_iter()
        .map(|r| r.text)
        .collect::<Vec<_>>()
        .join(""))
}

/// 找钓鱼条
pub async fn get_fish_box(
    task: &mut AutoFishingTask,
    screen: &mut ScreenFn,
    timeout: Duration,
) -> Result<bool, AutoFishingError> {
    log::info!("钓鱼框识别开始（超时 {}s）", timeout.as_secs());
    let deadline = Instant::now() + timeout;
    loop {
        if task.cancelled() {
            return Err(AutoFishingError::Cancelled);
        }
        if Instant::now() >= deadline {
            log::warn!("钓鱼框识别失败");
            return Ok(false);
        }
        let img = capture(screen)?;
        let img_w = img.cols();
        let img_h = img.rows();
        let top = Rect { x: 0, y: 0, width: img_w, height: img_h / 2 };
        let top_mat = Mat::roi(&img, top).map_err(|e| AutoFishingError::Navigate(crate::navigate::error::NavigateError::Cv(e.to_string())))?;
        let top_owned = top_mat.clone_pointee();
        let rects = get_fish_bar_rects(&top_owned)?;
        if rects.len() == 2 {
            let (cur, right) = if rects[0].width < rects[1].width {
                (rects[0], rects[1])
            } else {
                (rects[1], rects[0])
            };
            if (cur.height - right.height).abs() > 10 {
                log::error!("两个矩形高度差距过大，未识别到钓鱼框");
                continue;
            }
            // 几何校验
            if right.x < cur.x
                || cur.width > right.width
                || cur.x + cur.width > top_owned.cols() / 2
                || cur.x + cur.width > right.x - right.width / 2
                || cur.x + cur.width > top_owned.cols() / 2 - right.width
            {
                continue;
            }
            let h_extra = cur.height;
            let v_extra = cur.height / 4;
            let rx = cur.x - h_extra;
            let ry = cur.y - v_extra;
            let rw = (top_owned.cols() / 2 - cur.x) * 2 + h_extra * 2;
            let rh = cur.height + v_extra * 2;
            let mut r = Rect { x: rx, y: ry, width: rw, height: rh };
            // clamp 到全图
            if r.x < 0 { r.width += r.x; r.x = 0; }
            if r.y < 0 { r.height += r.y; r.y = 0; }
            if r.x + r.width > img_w { r.width = img_w - r.x; }
            if r.y + r.height > img_h { r.height = img_h - r.y; }
            if r.width <= 0 || r.height <= 0 {
                continue;
            }
            task.bb.fish_box_rect = Some(r);
            log::info!("识别到钓鱼框 {:?}", r);
            return Ok(true);
        }
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
}

/// 拉条动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeftBtn {
    Up,
    Down,
}

/// 拉条
pub async fn pulling(
    task: &mut AutoFishingTask,
    screen: &mut ScreenFn,
) -> Result<(), AutoFishingError> {
    let box_rect = match task.bb.fish_box_rect {
        Some(r) => r,
        None => return Ok(()),
    };
    log::info!("拉扯开始");
    let mut prev = LeftBtn::Up;
    let mut no_detect_until: Option<Instant> = None;
    loop {
        if task.cancelled() {
            task.sim.left_button_up().ok();
            return Err(AutoFishingError::Cancelled);
        }
        let img = capture(screen)?;
        let bar_mat = Mat::roi(&img, box_rect).map_err(|e| AutoFishingError::Navigate(crate::navigate::error::NavigateError::Cv(e.to_string())))?;
        let bar_owned = bar_mat.clone_pointee();
        let mut rects = get_fish_bar_rects(&bar_owned)?;
        if !rects.is_empty() {
            no_detect_until = None;
            if rects.len() > 3 {
                rects.sort_by(|a, b| b.height.cmp(&a.height));
                rects.truncate(3);
            }
            match rects.len() {
                2 => {
                    let (cursor, target) = if rects[0].width < rects[1].width {
                        (rects[0], rects[1])
                    } else {
                        (rects[1], rects[0])
                    };
                    if target.width < cursor.width * 10 {
                        // 异常
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        continue;
                    }
                    if cursor.x < target.x {
                        if prev != LeftBtn::Down {
                            task.sim.left_button_down().map_err(dev_err)?;
                            prev = LeftBtn::Down;
                        }
                    } else if prev == LeftBtn::Down {
                        task.sim.left_button_up().map_err(dev_err)?;
                        prev = LeftBtn::Up;
                    }
                }
                3 => {
                    rects.sort_by(|a, b| a.x.cmp(&b.x));
                    let left = rects[0];
                    let cursor = rects[1];
                    let right = rects[2];
                    let dist_right = (right.x + right.width) - (cursor.x + cursor.width);
                    let dist_left = cursor.x - left.x;
                    if dist_right <= dist_left {
                        if prev == LeftBtn::Down {
                            task.sim.left_button_up().map_err(dev_err)?;
                            prev = LeftBtn::Up;
                        }
                    } else if prev != LeftBtn::Down {
                        task.sim.left_button_down().map_err(dev_err)?;
                        prev = LeftBtn::Down;
                    }
                }
                _ => {} // 0/1个：不做反应
            }
        } else {
            // 1s都没矩形 -> 完成
            match no_detect_until {
                None => {
                    no_detect_until = Some(Instant::now() + Duration::from_secs(1));
                }
                Some(t) if Instant::now() >= t => {
                    log::info!("拉扯结束");
                    log::info!("---");
                    task.sim.left_button_up().ok();
                    // 拉条结束后等结果弹窗
                    // 等换饵按钮出现时可以抛竿
                    // 否则输入可能会被吞掉
                    wait_for_ready_to_cast(screen, Duration::from_secs(8)).await;
                    return Ok(());
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 钓上鱼后等待可以抛竿
async fn wait_for_ready_to_cast(screen: &mut ScreenFn, overall: Duration) {
    let deadline = Instant::now() + overall;
    let bait_btn = match assets::bait_button() {
        Ok(t) => t,
        Err(e) => {
            log::warn!("wait_for_ready_to_cast: 加载 BaitButton 模板失败 {e}，等待后继续");
            tokio::time::sleep(overall.min(Duration::from_secs(3))).await;
            return;
        }
    };
    while Instant::now() < deadline {
        if let Some(img) = screen() {
            match matches(&img, bait_btn) {
                Ok(true) => {
                    log::debug!("已回到待抛竿状态（BaitButton 重现）");
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    return;
                }
                Ok(false) => {}
                Err(e) => log::debug!("wait_for_ready_to_cast match error: {e}"),
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    log::warn!(
        "等 BaitButton 重现超时（{}s），继续下一轮（可能误触发 fallback）",
        overall.as_secs()
    );
    tokio::time::sleep(Duration::from_millis(1500)).await;
}

/// 退出钓鱼模式
pub async fn quit_fishing_mode(
    task: &mut AutoFishingTask,
    screen: &mut ScreenFn,
    overall: Duration,
) -> Result<(), AutoFishingError> {
    log::info!("退出钓鱼模式");
    let deadline = Instant::now() + overall;
    loop {
        if task.cancelled() {
            return Err(AutoFishingError::Cancelled);
        }
        if Instant::now() >= deadline {
            log::warn!("退出钓鱼模式超时");
            return Ok(());
        }
        let img = capture(screen)?;
        if matches(&img, assets::pick_f()?)? {
            log::info!("退出完成");
            return Ok(());
        }
        if let Some(m) = find_template(&img, assets::btn_black_confirm()?)? {
            click_at(&task.sim, m.center().x, m.center().y)?;
            log::info!("在 \"是否退出钓鱼？\" 界面点击确认");
            tokio::time::sleep(Duration::from_millis(1500)).await;
            return Ok(());
        }
        press_esc(&task.sim)?;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
