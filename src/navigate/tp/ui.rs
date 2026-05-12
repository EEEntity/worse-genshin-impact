//! 传送UI
//! 
//! 打开地图/切换国家/调整缩放等级/拖动地图/点击传送按钮/等待

use std::time::Duration;
use evdev_rs::enums::EV_KEY;
use opencv::core::{Mat, MatTraitConst, MatTraitConstManual, Rect as CvRect};
use tokio::time::sleep;

use crate::device::GIDevice;
use crate::navigate::bv;
use crate::navigate::coord::Point2f;
use crate::navigate::error::NavigateError;
use crate::navigate::map::BigMapMatcher;
use crate::navigate::tp::country::nearest_country;
use crate::navigate::tp::data::TpPosition;
use crate::navigate::tp::teyvat_coord::{BIG_MAP_256_TO_2048, Rect2048, game_to_screen_click};

use super::service::{ScreenProvider, TpDeps, TpTask, tp_consts};

impl TpTask {
    /// 打开大地图
    pub async fn open_big_map_ui(
        &self,
        deps: &mut TpDeps<'_>,
        retry: u32,
    ) -> Result<(), NavigateError> {
        for k in [EV_KEY::KEY_W, EV_KEY::KEY_A, EV_KEY::KEY_S, EV_KEY::KEY_D] {
            let _ = deps.device.key_up(k);
        }
        for _i in 0..retry.max(1) {
            deps.device
                .press_keys(&[EV_KEY::KEY_M], Duration::from_millis(50))
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(800)).await;
            let Some(s) = (deps.screen)()? else {
                return Err(NavigateError::Other("open_big_map_ui: 截图源不可用".into()));
            };
            if bv::is_in_big_map_ui(&s)? {
                sleep(Duration::from_millis(400)).await;
                return Ok(());
            }
            deps.device
                .press_keys(&[EV_KEY::KEY_ESC], Duration::from_millis(50))
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(500)).await;
        }
        Err(NavigateError::Timeout(format!(
            "open_big_map_ui: {retry} 次重试后仍未进入大地图"
        )))
    }
    /// 切换国家
    pub async fn switch_country_if_needed(
        &self,
        deps: &mut TpDeps<'_>,
        tp: &TpPosition,
    ) -> Result<(), NavigateError> {
        // 如果在地下，先切换到地上
        if let Some(screen) = (deps.screen)()? {
            if bv::big_map_is_underground(&screen)? {
                if let Some(btn) = crate::navigate::bv::matcher::find_template(
                    &screen,
                    crate::navigate::bv::assets::map_underground_switch()?,
                )? {
                    let c = btn.center();
                    deps.device
                        .teleport_mouse(c.x, c.y)
                        .map_err(|e|NavigateError::Device(e.to_string()))?;
                    sleep(Duration::from_millis(50)).await;
                    deps.device
                        .mouse_click(
                            EV_KEY::BTN_LEFT,
                            Duration::ZERO,
                            Duration::from_millis(40),
                            Duration::from_millis(120),
                        )
                        .map_err(|e|NavigateError::Device(e.to_string()))?;
                    sleep(Duration::from_millis(200)).await;
                }
            }
        }
        let country = match nearest_country(tp.x(), tp.y(), f64::MAX) {
            Some(c) => c,
            None => return Ok(()),
        };
        if deps.ocr.is_none() {
            log::warn!("未注入 OcrEngine，跳过切换国家（假定已在正确地图）");
            return Ok(());
        }
        self.switch_area(deps, country).await
    }
    /// 切换区域
    pub async fn switch_area(
        &self,
        deps: &mut TpDeps<'_>,
        area_name: &str,
    ) -> Result<(), NavigateError> {
        deps.device
            .teleport_mouse(tp_consts::SWITCH_AREA_BTN_X, tp_consts::SWITCH_AREA_BTN_Y)
            .map_err(|e|NavigateError::Device(e.to_string()))?;
        sleep(Duration::from_millis(50)).await;
        deps.device
            .mouse_click(
                EV_KEY::BTN_LEFT,
                Duration::ZERO,
                Duration::from_millis(40),
                Duration::from_millis(120),
            )
            .map_err(|e|NavigateError::Device(e.to_string()))?;
        sleep(Duration::from_millis(300)).await;

        let Some(screen) = (deps.screen)()? else {
            return Err(NavigateError::Other("switch_area: 截图源不可用".into()));
        };
        let w = screen.cols();
        let h = screen.rows();
        let roi_x = w * 2 / 3;
        let roi_w = w - roi_x;
        let roi = CvRect::new(roi_x, 0, roi_w, h);
        let sub = Mat::roi(&screen, roi).map_err(|e| NavigateError::Sift(format!("ocr roi: {e}")))?;
        let sub_mat = sub.clone_pointee();
        let rgb = bgr_mat_to_rgb_bytes(&sub_mat)?;
        let ocr = deps.ocr.as_deref_mut().expect("switch_area requires OCR");
        let results = ocr
            .run(&rgb, roi_w as u32, h as u32)
            .map_err(|e| NavigateError::Other(format!("ocr run: {e}")))?;
        let target = normalize_area_name(area_name);
        let mut hits: Vec<_> = results
            .iter()
            .filter(|r| normalize_area_name(&r.text).contains(&target))
            .collect();
        hits.sort_by_key(|r| std::cmp::Reverse(r.bbox[1]));
        let Some(hit) = hits.first() else {
            if is_isolated_map(area_name) {
                return Err(NavigateError::Other(format!(
                    "switch_area: 切换独立地图区域[{area_name}]失败"
                )));
            }
            log::warn!("switch_area: OCR 未识别到 \"{area_name}\"，跳过切换");
            sleep(Duration::from_millis(500)).await;
            return Ok(());
        };
        let cx = roi_x + hit.bbox[0] as i32 + (hit.bbox[2] / 2) as i32;
        let cy = hit.bbox[1] as i32 + (hit.bbox[3] / 2) as i32;
        deps.device
            .teleport_mouse(cx, cy)
            .map_err(|e|NavigateError::Device(e.to_string()))?;
        sleep(Duration::from_millis(50)).await;
        deps.device
            .mouse_click(
                EV_KEY::BTN_LEFT,
                Duration::ZERO,
                Duration::from_millis(40),
                Duration::from_millis(120),
            )
            .map_err(|e|NavigateError::Device(e.to_string()))?;
        sleep(Duration::from_millis(500)).await;
        Ok(())
    }
    /// 获取缩放等级
    pub async fn read_big_map_zoom(&self, deps: &mut TpDeps<'_>) -> Option<f64> {
        let s = (deps.screen)().ok().flatten()?;
        match bv::get_big_map_scale(&s) {
            Ok(z) => Some(z),
            Err(e) => {
                log::debug!("read_big_map_zoom 失败：{e}");
                None
            }
        }
    }
    /// 调整缩放等级
    pub async fn adjust_zoom_to(
        &self,
        deps: &mut TpDeps<'_>,
        target_zoom: f64,
    ) -> Result<(), NavigateError> {
        let Some(s) = (deps.screen)()? else {
            return Err(NavigateError::Other("adjust_zoom_to: 截图源不可用".into()));
        };
        let current = bv::get_big_map_scale(&s)?;
        if (current - target_zoom).abs() < tp_consts::PRECISION_THRESHOLD {
            return Ok(());
        }
        let ratio = (target_zoom / tp_consts::MAX_ZOOM_LEVEL).clamp(0.0, 1.0);
        let target_y = (tp_consts::ZOOM_END_Y as f64
            - ratio * (tp_consts::ZOOM_END_Y - tp_consts::ZOOM_START_Y) as f64) as i32;
        let m = crate::navigate::bv::matcher::find_template(
            &s,
            crate::navigate::bv::assets::map_scale_button()?,
        )?
        .ok_or_else(||NavigateError::Other("adjust_zoom_to: 找不到缩放按钮".into()))?;
        let cur_y = m.center().y;
        deps.device
            .mouse_drag(
                tp_consts::ZOOM_BUTTON_X,
                cur_y,
                tp_consts::ZOOM_BUTTON_X,
                target_y,
                Duration::from_millis(400),
                12,
            )
            .map_err(|e|NavigateError::Device(e.to_string()))?;
        sleep(Duration::from_millis(300)).await;
        Ok(())
    }
    /// 在大地图上拖动到目标位置
    pub async fn move_map_to_target(
        &self,
        device: &GIDevice,
        screen: ScreenProvider<'_>,
        big_map: &mut BigMapMatcher,
        target_game_x: f64,
        target_game_y: f64,
    ) -> Result<Rect2048, NavigateError> {
        let target = Point2f { x: target_game_x as f32, y: target_game_y as f32 };
        let initial_rect = {
            let Some(s) = screen()? else {
                return Err(NavigateError::Other("move_map_to_target: 截图源不可用".into()));
            };
            match big_map.match_rect_256(&s)? {
                Some(r) => rect_256_to_2048(r),
                None => return Err(NavigateError::Sift("move_map_to_target: 初始 SIFT 失败".into())),
            }
        };
        let mut current_rect = initial_rect;
        let mut consecutive_sift_fail = 0u32;
        for _attempt in 0..tp_consts::MOVE_MAP_MAX_ITER {
            let (target_screen_x, target_screen_y) = game_to_screen_click(current_rect, target, tp_consts::REF_W, tp_consts::REF_H);
            let center_x = tp_consts::REF_W / 2;
            let center_y = tp_consts::REF_H / 2;
            let off_x = target_screen_x - center_x;
            let off_y = target_screen_y - center_y;
            let dist_to_center = ((off_x * off_x + off_y * off_y) as f64).sqrt() as i32;
            if dist_to_center < tp_consts::MOVE_MAP_TOLERANCE_PX {
                return Ok(current_rect);
            }
            let dx = off_x.clamp(-700, 700);
            let dy = off_y.clamp(-500, 500);
            let mut rng = rand::thread_rng();
            let jitter_x = rand::Rng::gen_range(&mut rng, -(tp_consts::REF_W / 6)..=(tp_consts::REF_W / 6));
            let jitter_y = rand::Rng::gen_range(&mut rng, -(tp_consts::REF_H / 6)..=(tp_consts::REF_H / 6));
            let drag_from_x = center_x + jitter_x;
            let drag_from_y = center_y + jitter_y;
            let drag_to_x = drag_from_x - dx;
            let drag_to_y = drag_from_y - dy;
            device
                .mouse_drag(
                    drag_from_x,
                    drag_from_y,
                    drag_to_x,
                    drag_to_y,
                    Duration::from_millis(500),
                    20,
                )
                .map_err(|e|NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(400)).await;
            let dx_img = dx as f64 * current_rect.w / tp_consts::REF_W as f64;
            let dy_img = dy as f64 * current_rect.h / tp_consts::REF_H as f64;
            let predicted_rect = Rect2048 {
                x: current_rect.x + dx_img,
                y: current_rect.y + dy_img,
                w: current_rect.w,
                h: current_rect.h,
            };
            let expected_move_len = ((dx_img * dx_img + dy_img * dy_img) as f64).sqrt();
            let Some(s) = screen()? else {
                return Err(NavigateError::Other("move_map_to_target: 截图源不可用".into()));
            };
            match big_map.match_rect_256(&s)? {
                Some(r) => {
                    let measured = rect_256_to_2048(r);
                    let cdx = measured.center_x() - predicted_rect.center_x();
                    let cdy = measured.center_y() - predicted_rect.center_y();
                    let jump = (cdx * cdx + cdy * cdy).sqrt();
                    let limit = (200.0_f64).max(expected_move_len * 2.0);
                    if jump > limit {
                        consecutive_sift_fail += 1;
                        current_rect = predicted_rect;
                    } else {
                        consecutive_sift_fail = 0;
                        current_rect = measured;
                    }
                }
                None => {
                    consecutive_sift_fail += 1;
                    current_rect = predicted_rect;
                }
            }
            if consecutive_sift_fail > 5 {
                return Err(NavigateError::Sift(format!(
                    "move_map_to_target: 连续 {consecutive_sift_fail} 次 SIFT 失败/异常跳跃"
                )));
            }
        }
        Err(NavigateError::Timeout(format!(
            "move_map_to_target: {} 次拖动后目标 ({target_game_x:.1},{target_game_y:.1}) 仍未收敛",
            tp_consts::MOVE_MAP_MAX_ITER
        )))
    }
    /// 强制切换目标区域
    pub async fn force_jump_to_target_area(
        &self,
        deps: &mut TpDeps<'_>,
        target_x: f64,
        target_y: f64,
        map_name: &str,
    ) -> Result<(), NavigateError> {
        if map_name == "Teyvat" {
            if let Some(country) = nearest_country(target_x, target_y, f64::MAX) {
                if deps.ocr.is_some() {
                    self.switch_area(deps, country).await?;
                }
            }
        } else if let Some(desc) = scene_description(map_name) {
            if deps.ocr.is_some() {
                self.switch_area(deps, desc).await?;
            }
        }
        Ok(())
    }
    /// 点传送
    pub async fn click_tp_point(&self, deps: &mut TpDeps<'_>) -> Result<(), NavigateError> {
        let Some(s0) = (deps.screen)()? else {
            return Err(NavigateError::Other("click_tp_point: 截图源不可用".into()));
        };
        if !bv::is_in_big_map_ui(&s0)? {
            return Err(NavigateError::Retry("click_tp_point: 不在地图界面".into()));
        }
        sleep(Duration::from_millis(50)).await;
        if Self::try_click_teleport_button(deps).await? {
            return Ok(());
        }
        let Some(s1) = (deps.screen)()? else {
            return Err(NavigateError::Other("click_tp_point: 截图源不可用".into()));
        };
        if bv::find_map_close_button(&s1)?.is_some() {
            return Err(NavigateError::TpPointNotActivate(
                "传送点未激活或不存在（地图关闭按钮可见）".into(),
            ));
        }
        let icons = bv::find_map_choose_icons(&s1)?;
        if icons.is_empty() {
            return Err(NavigateError::TpPointNotActivate("选项列表不存在传送点".into()));
        }
        let mut clicked = false;
        for icon in &icons {
            if Self::try_click_choose_option(deps, &s1, icon).await? {
                clicked = true;
                break;
            }
        }
        if !clicked {
            return Err(NavigateError::TpPointNotActivate(
                "选项列表无任何可识别的传送点名称".into(),
            ));
        }
        let mut appeared = false;
        for _ in 0..6 {
            sleep(Duration::from_millis(300)).await;
            let Some(s) = (deps.screen)()? else {
                return Err(NavigateError::Other("click_tp_point: 截图源不可用".into()));
            };
            if bv::find_teleport_button(&s)?.is_some() {
                appeared = true;
                break;
            }
        }
        if !appeared {
            return Err(NavigateError::TpPointNotActivate(
                "选项列表的传送点未激活（按钮 1.8s 内未出现）".into(),
            ));
        }
        for _ in 0..6 {
            let Some(s) = (deps.screen)()? else { break; };
            let Some(m) = bv::find_teleport_button(&s)? else {
                return Ok(());
            };
            let c = m.center();
            deps.device
                .teleport_mouse(c.x, c.y)
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(50)).await;
            deps.device
                .mouse_click(
                    EV_KEY::BTN_LEFT,
                    Duration::ZERO,
                    Duration::from_millis(40),
                    Duration::from_millis(100),
                )
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(300)).await;
        }
        Ok(())
    }
    /// 点击传送按钮
    async fn try_click_teleport_button(deps: &mut TpDeps<'_>) -> Result<bool, NavigateError> {
        let Some(s) = (deps.screen)()? else {
            return Err(NavigateError::Other("click_tp_point: 截图源不可用".into()));
        };
        let Some(m) = bv::find_teleport_button(&s)? else {
            return Ok(false);
        };
        let c = m.center();
        deps.device
            .teleport_mouse(c.x, c.y)
            .map_err(|e|NavigateError::Device(e.to_string()))?;
        sleep(Duration::from_millis(50)).await;
        deps.device
            .mouse_click(
                EV_KEY::BTN_LEFT,
                Duration::ZERO,
                Duration::from_millis(40),
                Duration::from_millis(100),
            )
            .map_err(|e|NavigateError::Device(e.to_string()))?;
        Ok(true)
    }
    /// 选择点击传送点
    async fn try_click_choose_option(
        deps: &mut TpDeps<'_>,
        screen: &Mat,
        icon: &crate::navigate::bv::matcher::MatchResult,
    ) -> Result<bool, NavigateError> {
        let strip_x = icon.top_left.x + icon.width;
        let strip_y = icon.top_left.y - 8;
        let strip_w = 200;
        let strip_h = icon.height + 16;
        let strip_x = strip_x.max(0);
        let strip_y = strip_y.max(0);
        let max_w = (screen.cols() - strip_x).max(0);
        let max_h = (screen.rows() - strip_y).max(0);
        let strip_w = strip_w.min(max_w);
        let strip_h = strip_h.min(max_h);
        if strip_w <= 0 || strip_h <= 0 {
            return Ok(false);
        }
        let ocr_ok = if let Some(ocr) = deps.ocr.as_deref_mut() {
            let roi = CvRect::new(strip_x, strip_y, strip_w, strip_h);
            let sub = Mat::roi(screen, roi)
                .map_err(|e| NavigateError::Cv(e.to_string()))?
                .clone_pointee();
            let rgb = bgr_mat_to_rgb_bytes(&sub)?;
            let results = ocr
                .run(&rgb, strip_w as u32, strip_h as u32)
                .map_err(|e| NavigateError::Other(format!("ocr run: {e}")))?;
            let mut text = String::new();
            for r in &results {
                if !r.text.is_empty() && r.text.chars().count() > 1 {
                    text = r.text.replace('>', "");
                    break;
                }
            }
            !text.is_empty()
        } else {
            true
        };
        if !ocr_ok {
            return Ok(false);
        }
        sleep(Duration::from_millis(500)).await;
        let click_x = strip_x + strip_w / 2;
        let click_y = strip_y + strip_h / 2;
        deps.device
            .teleport_mouse(click_x, click_y)
            .map_err(|e| NavigateError::Device(e.to_string()))?;
        sleep(Duration::from_millis(50)).await;
        deps.device
            .mouse_click(
                EV_KEY::BTN_LEFT,
                Duration::ZERO,
                Duration::from_millis(40),
                Duration::from_millis(120),
            )
            .map_err(|e| NavigateError::Device(e.to_string()))?;
        Ok(true)
    }
    /// 等待传送完成
    pub async fn wait_for_teleport_completion(
        &self,
        screen: ScreenProvider<'_>,
        max_attempts: u32,
        delay: Duration,
    ) -> Result<(), NavigateError> {
        sleep(delay).await;
        for i in 0..max_attempts {
            let Some(s) = screen()? else {
                return Err(NavigateError::Other(
                    "wait_for_teleport_completion: 截图源不可用".into(),
                ));
            };
            if bv::is_in_main_ui(&s)? {
                log::info!("传送完成（第 {i} 次检测）");
                return Ok(());
            }
            sleep(delay).await;
        }
        Err(NavigateError::Timeout(format!(
            "wait_for_teleport_completion: {max_attempts} x {delay:?} 后仍未回到主界面"
        )))
    }
}

fn rect_256_to_2048(r256: CvRect) -> Rect2048 {
    Rect2048 {
        x: r256.x as f64 * BIG_MAP_256_TO_2048,
        y: r256.y as f64 * BIG_MAP_256_TO_2048,
        w: r256.width as f64 * BIG_MAP_256_TO_2048,
        h: r256.height as f64 * BIG_MAP_256_TO_2048,
    }
}

/// BGR Mat -> 行主序RGB Vec
pub fn bgr_mat_to_rgb_bytes(m: &Mat) -> Result<Vec<u8>, NavigateError> {
    use opencv::core::CV_8UC3;
    use opencv::imgproc::{COLOR_BGR2RGB, cvt_color_def};
    if m.typ() != CV_8UC3 {
        return Err(NavigateError::Other(format!(
            "bgr_mat_to_rgb_bytes: 期望 CV_8UC3, 实际 type={}",
            m.typ()
        )));
    }
    let mut rgb = Mat::default();
    cvt_color_def(m, &mut rgb, COLOR_BGR2RGB)
        .map_err(|e| NavigateError::Sift(format!("BGR→RGB: {e}")))?;
    let mut out = Vec::with_capacity((rgb.rows() * rgb.cols() * 3) as usize);
    for r in 0..rgb.rows() {
        let row = rgb
            .row(r)
            .map_err(|e| NavigateError::Sift(format!("row {r}: {e}")))?;
        let bytes = row
            .data_bytes()
            .map_err(|e| NavigateError::Sift(format!("row {r} bytes: {e}")))?;
        out.extend_from_slice(bytes);
    }
    Ok(out)
}

fn normalize_area_name(s: &str) -> String {
    s.replace('宮', "宫")
}

pub(super) fn scene_description(map_name: &str) -> Option<&'static str> {
    match map_name {
        "Teyvat" => Some("提瓦特大陆"),
        "TheChasm" => Some("层岩巨渊"),
        "Enkanomiya" => Some("渊下宫"),
        "SeaOfBygoneEras" => Some("旧日之海"),
        "AncientSacredMountain" => Some("远古圣山"),
        "TempleOfSpace" => Some("空之神殿"),
        _ => None,
    }
}

fn is_isolated_map(area_desc: &str) -> bool {
    matches!(
        area_desc,
        "层岩巨渊" | "渊下宫" | "旧日之海" | "远古圣山" | "空之神殿"
    )
}
