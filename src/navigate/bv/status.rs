//! 状态相关
//! 和导航无关的行为不在这里实现

use opencv::core::{Mat, MatTraitConst, Point, Vec3b};

use crate::navigate::bv::assets;
use crate::navigate::bv::matcher::{MatchResult, find_template, matches};
use crate::navigate::error::NavigateError;

/// 运动状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionStatus {
    Normal,
    Fly,
    Climb,
    Swim,
}

/// UI状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameUiCategory {
    Unknown,
    Main,
    Talk,
    BigMap,
}

/// 识别当前UI场景
pub fn which_game_ui(screen: &Mat) -> GameUiCategory {
    if matches_or_false(screen, assets::disabled_ui_button) {
        return GameUiCategory::Talk;
    }
    if matches_or_false(screen, assets::map_scale_button) 
        || matches_or_false(screen, assets::map_settings_button) {
        return GameUiCategory::BigMap;
    }
    if matches_or_false(screen, assets::paimon_menu) {
        return GameUiCategory::Main;
    }
    GameUiCategory::Unknown
}

fn matches_or_false<F>(screen: &Mat, asset_fn: F) -> bool
where 
    F: Fn() -> Result<&'static assets::Template, NavigateError>,
{
    matches!(asset_fn().and_then(|t|matches(screen, t)), Ok(true))
}

/// 判断是否在主界面
pub fn is_in_main_ui(screen: &Mat) -> Result<bool, NavigateError> {
    if !matches(screen, assets::paimon_menu()?)? {
        return Ok(true);
    }
    if crate::navigate::bv::revive::is_in_revive_prompt(screen)? {
        return Ok(false);
    }
    Ok(true)
}

/// 是否在大地图界面
pub fn is_in_big_map_ui(screen: &Mat) -> Result<bool, NavigateError> {
    if matches(screen, assets::map_scale_button()?)? {
        return Ok(true);
    }
    matches(screen, assets::map_settings_button()?)
}

/// 是否在对话界面
pub fn is_in_talk_ui(screen: &Mat) -> Result<bool, NavigateError> {
    matches(screen, assets::disabled_ui_button()?)
}

/// 是否在秘境内
pub fn is_in_domain(screen: &Mat) -> Result<bool, NavigateError> {
    let Some(m) = find_template(screen, assets::in_domain()?)? else {
        return Ok(false);
    };
    Ok(!all_sample_points_white(screen, &m))
    // 还应再排除复苏弹窗
}

fn all_sample_points_white(screen: &Mat, m: &MatchResult) -> bool {
    let cx = m.top_left.x + m.width / 2;
    let cy = m.top_left.y + m.height / 2;
    let qx = m.top_left.x + m.width / 4;
    let qy = m.top_left.y + m.height / 4;
    let qx3 = m.top_left.x + m.width * 3 / 4;
    let qy3 = m.top_left.y + m.height * 3 / 4;
    let pts = [
        Point { x: cx, y: cy },
        Point { x: qx, y: qy },
        Point { x: qx3, y: qy },
        Point { x: qx, y: qy3 },
        Point { x: qx3, y: qy3 },
    ];
    pts.iter().all(|p| pixel_is_white(screen, *p))
}

fn pixel_is_white(screen: &Mat, p: Point) -> bool {
    if p.x < 0 || p.y < 0 || p.x >= screen.cols() || p.y >= screen.rows() {
        return false;
    }
    let v: Vec3b = match screen.at_2d::<Vec3b>(p.y, p.x) {
        Ok(v) => *v,
        Err(_) => return false,
    };
    let in_white = |c: u8| (240..=255).contains(&c);
    in_white(v[0]) && in_white(v[1]) && in_white(v[2])
}

/// 大地图当前是否在地下
pub fn big_map_is_underground(screen: &Mat) -> Result<bool, NavigateError> {
    matches(screen, assets::map_underground_switch()?)
}

/// 当前角色运动状态
pub fn get_motion_status(screen: &Mat) -> Result<MotionStatus, NavigateError> {
    let space_exist = matches(screen, assets::key_space()?)?;
    if !space_exist {
        return Ok(MotionStatus::Normal);
    }
    if matches(screen, assets::key_x()?)? {
        Ok(MotionStatus::Climb)
    } else {
        Ok(MotionStatus::Fly)
    }
}

/// 当前大地图缩放等级
pub fn get_big_map_scale(screen: &Mat) -> Result<f64, NavigateError> {
    get_big_map_scale_with_config(screen, 468, 612, 5.0)
}

/// 留一个配置入口
pub fn get_big_map_scale_with_config(
    screen: &Mat,
    zoom_start_y: i32,
    zoom_end_y: i32,
    max_zoom_level: f64,
) -> Result<f64, NavigateError> {
    let m = find_template(screen, assets::map_scale_button()?)?
        .ok_or_else(|| {
            NavigateError::Other("get_big_map_scale: 未在大地图界面".into())
        })?;
    let cur = (m.top_left.y + m.height / 2) as f64;
    let ratio = (zoom_end_y as f64 - cur) / (zoom_end_y as f64 - zoom_start_y as f64);
    Ok(ratio.clamp(0.0, 1.0) * max_zoom_level)
}

/// 选中传送点后，侧边栏的"传送"按钮位置
pub fn find_teleport_button(screen: &Mat) -> Result<Option<MatchResult>, NavigateError> {
    find_template(screen, assets::go_teleport()?)
}

/// 大地图右上角关闭按钮
pub fn find_map_close_button(screen: &Mat) -> Result<Option<MatchResult>, NavigateError> {
    find_template(screen, assets::map_close_button()?)
}

/// 歧义传送点列表
/// 用于"上一个图标-OCR-点击"
pub fn find_map_choose_icons(screen: &Mat) -> Result<Vec<MatchResult>, NavigateError> {
    use crate::navigate::bv::matcher::find_template_all;
    let mut hits = Vec::new();
    for tpl in assets::map_choose_icon_templates()? {
        // 每个模板最多5个命中
        let mut h = find_template_all(screen, tpl, 5)?;
        hits.append(&mut h);
    }
    hits.sort_by_key(|m| m.top_left.y);
    Ok(hits)
}

/// 当前角色是否低血量
pub fn current_avatar_is_low_hp(screen: &Mat) -> Result<bool, NavigateError> {
    use opencv::core::{MatTraitConst, Vec3b};
    if screen.cols() < 1920 || screen.rows() < 1080 {
        return Ok(false);
    }
    let p: Vec3b = *screen
        .at_2d::<Vec3b>(1010, 808)
        .map_err(|e| NavigateError::Cv(e.to_string()))?;
    Ok(p[0] == 90 && p[1] == 90 && p[2] == 255)
}

/// 复苏按钮
pub fn find_revive_modal(
    screen: &Mat,
    ocr: &mut crate::inference::ocr::OcrEngine,
) -> Result<Option<(i32, i32)>, NavigateError> {
    use opencv::core::{MatTraitConst, Rect as CvRect};
    if screen.empty() {
        return Ok(None);
    }
    let w = screen.cols();
    let h = screen.rows();
    let roi_y = h / 4 * 3;
    let roi_h = h - roi_y;
    let roi = CvRect::new(0, roi_y, w, roi_h);
    let sub = Mat::roi(screen, roi).map_err(|e| NavigateError::Cv(e.to_string()))?;
    let sub_mat = sub.clone_pointee();
    let rgb = crate::navigate::tp::bgr_mat_to_rgb_bytes(&sub_mat)?;
    let results = ocr
        .run(&rgb, w as u32, roi_h as u32)
        .map_err(|e| NavigateError::Other(format!("ocr revive: {e}")))?;
    for r in results {
        if r.text.contains("复苏") {
            let cx = r.bbox[0] as i32 + (r.bbox[2] / 2) as i32;
            let cy = roi_y + r.bbox[1] as i32 + (r.bbox[3] / 2) as i32;
            return Ok(Some((cx, cy)));
        }
    }
    Ok(None)
}
