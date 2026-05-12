//! 传送功能入口

use std::time::Duration;
use evdev_rs::enums::EV_KEY;
use opencv::core::Mat;
use tokio::time::sleep;

use crate::device::GIDevice;
use crate::navigate::coord::Point2f;
use crate::navigate::error::NavigateError;
use crate::navigate::map::BigMapMatcher;
use crate::navigate::tp::data::{TpDatabase, TpPosition};
use crate::navigate::tp::teyvat_coord::game_to_screen_click;
use crate::inference::ocr::OcrEngine;

/// 获取整个游戏窗口的图像闭包
pub type ScreenProvider<'a> = &'a mut dyn FnMut() -> Result<Option<Mat>, NavigateError>;

/// 大地图相关坐标常量
pub mod tp_consts {
    pub const ZOOM_BUTTON_X: i32 = 47;
    pub const ZOOM_START_Y: i32 = 468;
    pub const ZOOM_END_Y: i32 = 612;
    pub const MAX_ZOOM_LEVEL: f64 = 5.0;
    pub const MIN_ZOOM_LEVEL: f64 = 2.0;
    pub const DISPLAY_TP_POINT_ZOOM_LEVEL: f64 = 4.4;
    pub const PRECISION_THRESHOLD: f64 = 0.1;
    pub const REF_W: i32 = 1920;
    pub const REF_H: i32 = 1080;
    pub const SWITCH_AREA_BTN_X: i32 = REF_W - 160;
    pub const SWITCH_AREA_BTN_Y: i32 = REF_H - 60;
    pub const MOVE_MAP_MAX_ITER: u32 = 30;
    pub const MOVE_MAP_TOLERANCE_PX: i32 = 200;
}

/// 传送时依赖注入
pub struct TpDeps<'a> {
    pub device: &'a GIDevice,
    pub screen: ScreenProvider<'a>,
    pub ocr: Option<&'a mut OcrEngine>,
    pub big_map: Option<&'a mut BigMapMatcher>,
}

/// 传送子系统
pub struct TpTask {
    pub db: TpDatabase,
    pub timeout: Duration,
}

impl TpTask {
    pub fn new(db: TpDatabase) -> Self {
        Self { db, timeout: Duration::from_secs(120) }
    }
    pub fn load_default() -> Result<Self, NavigateError> {
        Ok(Self::new(TpDatabase::load_default()?))
    }
    /// 在`map_name`下传送到目标坐标，返回落地坐标
    pub async fn tp_to(
        &self,
        deps: &mut TpDeps<'_>,
        target: Point2f,
        map_name: &str,
    ) -> Result<Point2f, NavigateError> {
        let mut last_err: Option<NavigateError> = None;
        for attempt in 0..3 { // 配置
            match self.tp_once(deps, target, map_name).await {
                Ok(p) => return Ok(p),
                Err(NavigateError::TpPointNotActivate(msg)) => {
                    log::warn!(
                        "tp_to: TpPointNotActivate（{msg}），按ESC后重试 #{}",
                        attempt + 1
                    );
                    let _ = deps.device.press_keys(&[EV_KEY::KEY_ESC], Duration::from_millis(50));
                    sleep(Duration::from_millis(300)).await;
                    last_err = Some(NavigateError::TpPointNotActivate(msg));
                }
                Err(e) => {
                    log::error!("tp_to: 第 {} 次失败：{e}", attempt + 1);
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| NavigateError::Other("tp_to: 3 次重试后仍失败".into())))
    }
    async fn tp_once(
        &self,
        deps: &mut TpDeps<'_>,
        target: Point2f,
        map_name: &str,
    ) -> Result<Point2f, NavigateError> {
        let nearest = self.db.nearest_n(target.x as f64, target.y as f64, map_name, 2);
        let tp: TpPosition = nearest
            .first()
            .copied()
            .ok_or_else(|| NavigateError::Other(format!("tp.json 中场景 {map_name} 没有任何传送点")))?
            .clone();
        let min_zoom_level = if nearest.len() >= 2 {
            let dx = nearest[0].x() - nearest[1].x();
            let dy = nearest[0].y() - nearest[1].y();
            let dist = (dx * dx + dy * dy).sqrt();
            (dist / 20.0).max(1.0)
        } else {
            1.0
        };
        log::info!(
            "目标 ({:.1}, {:.1}) 最近 TP = id={} {} ({:.1}, {:.1})、min_zoom_level={:.2}",
            target.x,
            target.y,
            tp.id,
            tp.name.as_deref().unwrap_or(""),
            tp.x(),
            tp.y(),
            min_zoom_level,
        );
        self.tp_via_ui(deps, &tp, map_name, min_zoom_level).await
    }
    /// 用大地图UI完成传送
    pub async fn tp_via_ui(
        &self,
        deps: &mut TpDeps<'_>,
        tp: &TpPosition,
        map_name: &str,
        min_zoom_level: f64,
    ) -> Result<Point2f, NavigateError> {
        self.open_big_map_ui(deps, 3).await?;
        if map_name == "Teyvat" {
            self.switch_country_if_needed(deps, tp).await?;
        } else if let Some(desc) = super::ui::scene_description(map_name) {
            if deps.ocr.is_some() {
                self.switch_area(deps, desc).await?;
            } else {
                log::warn!("场景 {map_name}: 未注入 OCR，无法自动切换到 \"{desc}\" 地图");
            }
        } else {
            log::warn!("未知地图 {map_name}，跳过切换区域");
        }
        sleep(Duration::from_millis(50)).await;
        let mut current_zoom = self
            .read_big_map_zoom(deps)
            .await
            .unwrap_or(tp_consts::DISPLAY_TP_POINT_ZOOM_LEVEL);
        if current_zoom > tp_consts::DISPLAY_TP_POINT_ZOOM_LEVEL + tp_consts::PRECISION_THRESHOLD {
            self.adjust_zoom_to(deps, tp_consts::DISPLAY_TP_POINT_ZOOM_LEVEL)
                .await?;
            current_zoom = tp_consts::DISPLAY_TP_POINT_ZOOM_LEVEL;
        } else if current_zoom < tp_consts::MIN_ZOOM_LEVEL - tp_consts::PRECISION_THRESHOLD {
            self.adjust_zoom_to(deps, tp_consts::MIN_ZOOM_LEVEL).await?;
            current_zoom = tp_consts::MIN_ZOOM_LEVEL;
        }
        if current_zoom > min_zoom_level {
            self.adjust_zoom_to(deps, min_zoom_level).await?;
            sleep(Duration::from_millis(300)).await;
        }
        let visible_rect = {
            let bg = deps.big_map.as_deref_mut().ok_or_else(|| {
                NavigateError::Unsupported("tp_via_ui: 未注入 BigMapMatcher，无法定位大地图".into())
            })?;
            self.move_map_to_target(deps.device, deps.screen, bg, tp.x(), tp.y())
                .await
        };
        let visible_rect = match visible_rect {
            Ok(r) => r,
            Err(e) => {
                log::warn!("move_map_to_target 失败：{e}；尝试 ForceJumpToTargetArea 后再试一次");
                self.force_jump_to_target_area(deps, tp.x(), tp.y(), map_name).await?;
                sleep(Duration::from_millis(300)).await;
                let bg = deps.big_map.as_deref_mut().ok_or_else(|| {
                    NavigateError::Unsupported("tp_via_ui: 未注入 BigMapMatcher，无法定位大地图".into())
                })?;
                self.move_map_to_target(deps.device, deps.screen, bg, tp.x(), tp.y())
                    .await?
            }
        };
        let target = Point2f { x: tp.x() as f32, y: tp.y() as f32 };
        let (sx, sy) = game_to_screen_click(visible_rect, target, tp_consts::REF_W, tp_consts::REF_H);
        deps.device
            .teleport_mouse(sx, sy)
            .map_err(|e| NavigateError::Device(e.to_string()))?;
        sleep(Duration::from_millis(80)).await;
        deps.device
            .mouse_click(
                EV_KEY::BTN_LEFT,
                Duration::ZERO,
                Duration::from_millis(40),
                Duration::from_millis(120),
            )
            .map_err(|e| NavigateError::Device(e.to_string()))?;
        sleep(Duration::from_millis(300)).await;
        self.click_tp_point(deps).await?;
        self.wait_for_teleport_completion(deps.screen, 50, Duration::from_millis(1200))
            .await?;
        sleep(Duration::from_millis(400)).await;
        let (w, h) = deps.device.window_size();
        if w > 0 && h > 0 {
            let _ = deps.device.teleport_mouse(w / 2, h / 2);
        }
        let _ = deps.device.mouse_click(
            EV_KEY::BTN_LEFT,
            Duration::ZERO,
            Duration::from_millis(40),
            Duration::from_millis(120),
        );
        sleep(Duration::from_millis(300)).await;
        let landed = if tp.tran_position != [0.0; 3] {
            Point2f { x: tp.tran_x() as f32, y: tp.tran_y() as f32 }
        } else {
            target
        };
        Ok(landed)
    }
    /// 传送到距当前位置最近的七天神像
    pub async fn tp_to_statue_of_the_seven(
        &self,
        deps: &mut TpDeps<'_>,
        current_pos: Point2f,
    ) -> Result<Point2f, NavigateError> {
        let goddess = self
            .db
            .nearest_goddess(current_pos.x as f64, current_pos.y as f64)
            .ok_or_else(|| NavigateError::Other("tp.json 中未找到任何七天神像点位".into()))?
            .clone();
        log::info!(
            "TP 七天神像：从 ({:.1}, {:.1}) -> id={} {} ({:.1}, {:.1})",
            current_pos.x,
            current_pos.y,
            goddess.id,
            goddess.name.as_deref().unwrap_or(""),
            goddess.x(),
            goddess.y(),
        );
        self.tp_via_ui(deps, &goddess, "Teyvat", 1.0).await
    }
}
