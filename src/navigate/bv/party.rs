//! 队伍头像侧栏/编号栏识别

use opencv::core::{Mat, MatTraitConst, Rect};

use crate::avatar::assets as fa;
use crate::navigate::bv::assets;
use crate::navigate::bv::matcher::{find_template, find_template_all, matches};
use crate::navigate::error::NavigateError;

/// 联机/队伍人数信息
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiGameStatus {
    /// 是否正在联机
    pub is_in_multi_game: bool,
    /// 是否房主
    pub is_host: bool,
    /// 玩家数量
    pub player_count: u8,
}

impl MultiGameStatus {
    /// 单人离线/未识别默认值
    pub const fn single_player() -> Self {
        Self {
            is_in_multi_game: false,
            is_host: false,
            player_count: 1,
        }
    }
    /// 自己能控制的最大角色数
    pub fn max_control_avatar_count(&self) -> u8 {
        if !self.is_in_multi_game {
            return 4;
        }
        if self.is_host {
            match self.player_count {
                1 => 4,
                2 | 3 => 2,
                4 => 1,
                _ => 1,
            }
        } else {
            match self.player_count {
                2 => 2,
                3 | 4 => 1,
                _ => 1,
            }
        }
    }
    /// 转换
    pub fn multi_key(&self) -> Option<fa::MultiKey> {
        if !self.is_in_multi_game {
            return None;
        }
        Some(fa::MultiKey {
            is_host: self.is_host,
            player_count: self.player_count,
        })
    }
}

/// 联机/人数判定
pub fn detect_multi_game_status(screen: &Mat) -> Result<MultiGameStatus, NavigateError> {
    let p_hits = find_template_all(screen, assets::p_icon()?, 3)?;
    let one_p_hit = find_template(screen, assets::one_p_icon()?)?;
    if !p_hits.is_empty() {
        let player_count = (p_hits.len() as u8 + 1).min(4);
        let is_host = one_p_hit.is_some();
        return Ok(MultiGameStatus {
            is_in_multi_game: true,
            is_host,
            player_count,
        });
    }
    if one_p_hit.is_some() {
        return Ok(MultiGameStatus {
            is_in_multi_game: true,
            is_host: true,
            player_count: 1,
        });
    }
    Ok(MultiGameStatus::single_player())
}

/// 有无编号块
pub fn has_any_index_rect(screen: &Mat) -> Result<bool, NavigateError> {
    for t in [assets::index_1()?, assets::index_2()?, assets::index_3()?, assets::index_4()?] {
        if matches(screen, t)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// 命中的编号块个数
pub fn count_index_rect(screen: &Mat) -> Result<u8, NavigateError> {
    let mut n = 0u8;
    for t in [assets::index_1()?, assets::index_2()?, assets::index_3()?, assets::index_4()?] {
        if matches(screen, t)? {
            n += 1;
        }
    }
    Ok(n)
}

/// 有无出战角色箭头
pub fn has_active_avatar_arrow(screen: &Mat) -> Result<bool, NavigateError> {
    matches(screen, assets::current_avatar_threshold()?)
}

/// ROI列表
pub fn get_all_index_rects(
    screen: &Mat,
    status: MultiGameStatus,
) -> (Vec<Rect>, Vec<Rect>) {
    let w = screen.cols();
    let h = screen.rows();
    if let Some(key) = status.multi_key() {
        let side = fa::avatar_side_icon_rects_multi(w, h, key);
        let idx = fa::avatar_index_rects_multi(w, h, key);
        (idx, side)
    } else {
        let idx = fa::avatar_index_rects(w, h).to_vec();
        let side = fa::avatar_side_icon_rects(w, h).to_vec();
        (idx, side)
    }
}
