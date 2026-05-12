//! 视觉辅助层
//! 用小模板图和固定ROI做匹配以判断状态
//! 
//! 公开函数接收1920x1080 BGR图[`opencv::core::Mat`]
//! 截图/裁剪由[`PathExecutor`](super::executor::PathExecutor)负责

pub mod assets;
pub mod hp;
pub mod matcher;
pub mod motion;
pub mod party;
pub mod revive;
pub mod skill;
pub mod status;

pub use hp::current_avatar_low_hp;
pub use motion::{is_climbing, is_flying, is_swimming, is_using_parachute};
pub use party::{MultiGameStatus, detect_multi_game_status, get_all_index_rects};
pub use revive::is_in_revive_prompt;
pub use skill::{read_e_cooldown_ready, read_q_cooldown_ready};
pub use status::{
    GameUiCategory, MotionStatus, big_map_is_underground, current_avatar_is_low_hp, find_revive_modal,
    find_map_choose_icons, find_map_close_button, find_teleport_button, get_big_map_scale,
    get_motion_status, is_in_big_map_ui, is_in_domain, is_in_main_ui, is_in_talk_ui,
    which_game_ui,
};
