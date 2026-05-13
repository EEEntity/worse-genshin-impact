//! 队伍状态

use std::sync::Arc;
use opencv::core::{Mat, MatTraitConst, Rect};

use crate::avatar::avatar::{Avatar, AvatarError, CancelFlag};
use crate::avatar::predictor::AvatarPredictor;
use crate::config::combat_avatar::registry;
use crate::device::simulator::Simulator;
use crate::navigate::bv::party::{
    MultiGameStatus, detect_multi_game_status, get_all_index_rects,
};
use crate::navigate::error::NavigateError;

/// 当前战斗场景: 队伍/联机状态
pub struct CombatScenes {
    avatars: Vec<Avatar>,
    multi_status: MultiGameStatus,
    /// 期望的(可控制)队伍人数
    expected_team_avatar_num: u8,
    /// 最近一次识别出的出战编号
    last_active_index: std::sync::Mutex<Option<u8>>,
}

impl CombatScenes {
    pub fn avatars(&self) -> &[Avatar] {
        &self.avatars
    }
    pub fn avatar_count(&self) -> usize {
        self.avatars.len()
    }
    pub fn multi_status(&self) -> MultiGameStatus {
        self.multi_status
    }
    pub fn expected_team_avatar_num(&self) -> u8 {
        self.expected_team_avatar_num
    }
    /// 按中文名/英文名/别名查找
    pub fn select_by_name(&self, name: &str) -> Option<&Avatar> {
        let combat = registry().lookup(name)?;
        self.avatars.iter().find(|a|std::ptr::eq(a.combat, combat))
    }
    /// 按编号查找
    pub fn select_by_index(&self, index: u8) -> Option<&Avatar> {
        self.avatars.iter().find(|a|a.index == index)
    }
    /// 从配置构造队伍
    /// 还没测试过
    pub fn initialize_from_config(
        team_names: &str,
        sim: Arc<Simulator>,
        cancel: CancelFlag,
    ) -> Self {
        let mut avatars = Vec::new();
        let mut idx = 1u8;
        for raw in team_names.split(|c: char|c == ','||c == ';'||c.is_whitespace()) {
            let s = raw.trim();
            if s.is_empty() {
                continue;
            }
            match registry().lookup(s) {
                Some(combat) => {
                    avatars.push(Avatar::new(
                        combat,
                        idx,
                        Rect::default(),
                        Rect::default(),
                        -1.0,
                        sim.clone(),
                        cancel.clone(),
                    ));
                    idx += 1;
                }
                None => log::warn!("队伍配置中找不到角色: {s:?}"),
            }
        }
        let expected_team_avatar_num = avatars.len() as u8;
        Self {
            avatars,
            multi_status: MultiGameStatus::single_player(),
            expected_team_avatar_num,
            last_active_index: std::sync::Mutex::new(None),
        }
    }
    /// 截图初始化队伍
    pub fn initialize_from_screen(
        screen: &Mat,
        predictor: &dyn AvatarPredictor,
        sim: Arc<Simulator>,
        cancel: CancelFlag,
    ) -> Result<Self, NavigateError> {
        let multi_status = detect_multi_game_status(screen)?;
        let (index_rects, side_icon_rects) = get_all_index_rects(screen, multi_status);
        let expected_team_avatar_num = index_rects.len() as u8;
        let mut avatars = Vec::new();
        for (i, side_rect) in side_icon_rects.iter().enumerate() {
            let slot = (i+1) as u8;
            let combat = match crop_safe(screen, *side_rect)
                .ok()
                .and_then(|crop|predictor.predict(&crop, slot))
            {
                Some(c) => c,
                None => {
                    log::warn!("第 {slot} 位角色识别失败，跳过");
                    continue;
                }
            };
            let idx_rect = index_rects.get(i).copied().unwrap_or_default();
            avatars.push(Avatar::new(
                combat,
                slot,
                *side_rect,
                idx_rect,
                -1.0,
                sim.clone(),
                cancel.clone(),
            ));
        }
        log::info!(
            "识别到的队伍角色: {}",
            avatars.iter().map(|a|a.name()).collect::<Vec<_>>().join(",")
        );
        Ok(Self {
            avatars,
            multi_status,
            expected_team_avatar_num,
            last_active_index: std::sync::Mutex::new(None),
        })
    }
}

/// 安全裁剪`screen`到`rect`
fn crop_safe(screen: &Mat, rect: Rect) -> Result<Mat, AvatarError> {
    let sr = Rect::new(0, 0, screen.cols(), screen.rows());
    let safe = intersect(rect, sr);
    if safe.width <= 0 || safe.height <= 0 {
        return Err(AvatarError::Cancelled); // 占位
    }
    Mat::roi(screen, safe)
        .map(|r| r.try_clone().unwrap_or_default())
        .map_err(|_| AvatarError::Cancelled)
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y2 = (a.y + a.height).min(b.y + b.height);
    if x2 <= x1 || y2 <= y1 {
        return Rect::new(0, 0, 0, 0);
    }
    Rect::new(x1, y1, x2 - x1, y2 - y1)
}

/// 编号块是否被白色块覆盖
/// 出战时为非白色
fn is_index_rect_white(screen: &Mat, rect: Rect) -> Result<bool, NavigateError> {
    use opencv::core::no_array;
    use opencv::imgproc::{COLOR_BGR2GRAY, cvt_color};
    let sr = Rect::new(0, 0, screen.cols(), screen.rows());
    let safe = intersect(rect, sr);
    if safe.width <= 0 || safe.height <= 0 {
        return Ok(false);
    }
    let crop = Mat::roi(screen, safe).map_err(|e|NavigateError::Cv(e.to_string()))?;
    let mut gray = Mat::default();
    cvt_color(&crop, &mut gray, COLOR_BGR2GRAY, 0, opencv::core::AlgorithmHint::ALGO_HINT_DEFAULT)
        .map_err(|e|NavigateError::Cv(e.to_string()))?;
    let mut mask = Mat::default();
    opencv::core::in_range(
        &gray,
        &opencv::core::Scalar::all(251.0),
        &opencv::core::Scalar::all(255.0),
        &mut mask,
    )
    .map_err(|e|NavigateError::Cv(e.to_string()))?;
    let count = opencv::core::count_non_zero(&mask).map_err(|e|NavigateError::Cv(e.to_string()))?;
    let _ = no_array;
    let area = safe.width * safe.height;
    if area <= 0 {
        return Ok(false);
    }
    Ok((count as f64) / (area as f64) > 0.5)
}

/// 用已知`index_recs`(无`Avatar`)列表查找出战编号
/// 
/// 用于`navigate::bv::part`无`CombatScenes`这类位置复用
pub fn get_active_avatar_index_from_indices(screen: &Mat, index_rects: &[Rect]) -> Option<u8> {
    for (i, r) in index_rects.iter().enumerate() {
        if r.width <= 0 || r.height <= 0 {
            continue;
        }
        if let Ok(false) = is_index_rect_white(screen, *r) {
            return Some((i + 1) as u8);
        }
    }
    None
}
