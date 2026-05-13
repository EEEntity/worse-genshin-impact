//! 队伍/战斗UI相关资源

use opencv::core::Rect;

/// 1080p
fn s(h: i32) -> f64 {
    h as f64 / 1080.0
}

#[inline]
fn px(v: f64, scale: f64) -> i32 {
    (v * scale) as i32
}

/// 队伍区(含编号)
pub fn team_rect(w: i32, h: i32) -> Rect {
    let s = s(h);
    Rect::new(w - px(355.0, s), px(220.0, s), px(355.0, s), px(465.0, s))
}

/// 队伍区(不含编号)
pub fn team_rect_no_index(w: i32, h: i32) -> Rect {
    let s = s(h);
    Rect::new(w - px(355.0, s), px(220.0, s), px(355.0 - 85.0, s), px(465.0, s))
}

/// E图标
pub fn e_rect(w: i32, h: i32) -> Rect {
    let s = s(h);
    Rect::new(w - px(267.0, s), h - px(132.0, s), px(77.0, s), px(77.0, s))
}

/// E技能CD数字
pub fn e_cooldown_rect(w: i32, h: i32) -> Rect {
    let s = s(h);
    Rect::new(w - px(241.0, s), h - px(97.0, s), px(41.0, s), px(18.0, s))
}

/// Q图标
pub fn q_rect(w: i32, h: i32) -> Rect {
    let s = s(h);
    Rect::new(w - px(157.0, s), h - px(165.0, s), px(110.0, s), px(110.0, s))
}

/// Z道具图标
pub fn gadget_rect(w: i32, h: i32) -> Rect {
    let s = s(h);
    Rect::new(w - px(133.0, s), px(800.0, s), px(60.0, s), px(50.0, s))
}

/// Z道具CD数字ROI
pub fn z_cooldown_rect(w: i32, h: i32) -> Rect {
    let s = s(h);
    Rect::new(w - px(130.0, s), px(814.0, s), px(60.0, s), px(24.0, s))
}

/// 上方"挑战达成"提示
pub fn end_tips_upper_rect(w: i32, h: i32) -> Rect {
    let s = s(h);
    Rect::new(w / 2 - px(100.0, s), px(243.0, s), px(200.0, s), px(50.0, s))
}

/// 底部战斗结束提示
pub fn end_tips_rect(w: i32, h: i32) -> Rect {
    let s = s(h);
    Rect::new(w / 2 - px(200.0, s), h - px(160.0, s), px(400.0, s), px(80.0, s))
}

/// 角色编号块
pub fn avatar_index_rects(w: i32, h: i32) -> [Rect; 4] {
    let s = s(h);
    let make = |y: f64| Rect::new(w - px(61.0, s), px(y, s), px(28.0, s), px(24.0, s));
    [make(256.0), make(352.0), make(448.0), make(544.0)]
}

/// 角色侧边头像
pub fn avatar_side_icon_rects(w: i32, h: i32) -> [Rect; 4] {
    let s = s(h);
    let make = |y: f64| Rect::new(w - px(155.0, s), px(y, s), px(76.0, s), px(76.0, s));
    [make(225.0), make(315.0), make(410.0), make(500.0)]
}

/// 角色侧边Q图标
pub fn avatar_q_rects(w: i32, h: i32) -> [Rect; 4] {
    let s = s(h);
    let make = |y: f64| Rect::new(w - px(336.0, s), px(y, s), px(64.0, s), px(84.0, s));
    [make(216.0), make(316.0), make(416.0), make(516.0)]
}

/// 联机模式键
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiKey {
    /// 是否房主
    pub is_host: bool,
    /// 联机总人数
    pub player_count: u8,
}

/// 联机模式侧边头像列表
pub fn avatar_side_icon_rects_multi(w: i32, h: i32, key: MultiKey) -> Vec<Rect> {
    let s = s(h);
    let mk = |y: f64| Rect::new(w - px(155.0, s), px(y, s), px(76.0, s), px(76.0, s));
    match (key.is_host, key.player_count) {
        // 1p_2 / 1p_3 同位置
        (true, 2) | (true, 3) | (false, 2) => vec![mk(375.0), mk(470.0)],
        (true, 4) | (false, 4) => vec![mk(515.0)],
        (false, 3) => vec![mk(475.0)],
        _ => Vec::new(),
    }
}

/// 联机模式角色编号列表
pub fn avatar_index_rects_multi(w: i32, h: i32, key: MultiKey) -> Vec<Rect> {
    let s = s(h);
    let mk = |y: f64| Rect::new(w - px(61.0, s), px(y, s), px(28.0, s), px(24.0, s));
    match (key.is_host, key.player_count) {
        (true, 2) | (false, 2) => vec![mk(412.0), mk(508.0)],
        (true, 3) => vec![mk(459.0), mk(555.0)],
        (true, 4) => vec![mk(552.0)],
        (false, 3) => vec![mk(412.0)],
        (false, 4) => vec![mk(507.0)],
        _ => Vec::new(),
    }
}

// 辅助功能

/// 已知一个编号块位置，推算另一个编号位置
/// X相同，Y相差96(1080p下)
pub fn index_rect_from_known(known_index: i32, known_rect: Rect, target_index: i32, h: i32) -> Rect {
    let s = s(h);
    let dy = ((target_index - known_index) as f64 * 96.0 * s) as i32;
    Rect::new(known_rect.x, known_rect.y + dy, known_rect.width, known_rect.height)
}

/// 已知出战角色箭头，推算编号块位置
pub fn index_rect_from_current_arrow(curr_rect: Rect, h: i32) -> Rect {
    let s = s(h);
    Rect::new(
        curr_rect.x + px(126.0, s),
        curr_rect.y - px(194.0, s),
        px(16.0, s),
        px(17.0, s),
    )
}

/// 由编号块推算侧边头像位置
pub fn side_icon_from_index(index_rect: Rect, h: i32) -> Rect {
    let s = s(h);
    Rect::new(
        index_rect.x - px(91.0, s),
        index_rect.y - px(47.0, s),
        px(82.0, s),
        px(82.0, s),
    )
}
