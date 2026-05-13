use std::path::PathBuf;
use std::sync::OnceLock;

use opencv::core::{Mat, Rect};
use opencv::imgcodecs::{IMREAD_COLOR, imread};
use opencv::prelude::MatTraitConst;

use crate::navigate::bv::assets::Template;
use crate::navigate::error::NavigateError;

#[deprecated(note = "适配不同分辨率")]
const SCREEN_W: i32 = 1920;
#[deprecated(note = "适配不同分辨率")]
const SCREEN_H: i32 = 1080;

struct SyncT(Template);
unsafe impl Sync for SyncT {}
unsafe impl Send for SyncT {}

/// 加载模板
fn load(rel: &str) -> Result<Mat, NavigateError> {
    let path: PathBuf = PathBuf::from("assets/templates").join(rel);
    let s = path.to_string_lossy().to_string();
    let m = imread(&s, IMREAD_COLOR).map_err(|e| NavigateError::Cv(e.to_string()))?;
    if m.empty() {
        return Err(NavigateError::Cv(format!("无法加载模板 PNG：{s}")));
    }
    Ok(m)
}

// 或许该把所有assets lazy load都放一块
macro_rules! lazy_t {
    ($fn_name:ident, $rel:expr, $name:expr, $roi:expr, $th:expr) => {
        pub fn $fn_name() -> Result<&'static Template, NavigateError> {
            static CELL: OnceLock<Result<SyncT, String>> = OnceLock::new();
            let entry = CELL.get_or_init(|| {
                load($rel)
                    .map(|mat| SyncT(Template { name: $name, mat, roi: $roi, threshold: $th }))
                    .map_err(|e| e.to_string())
            });
            entry.as_ref().map(|s| &s.0).map_err(|e| NavigateError::Cv(e.clone()))
        }
    };
}

// 选择鱼饵
lazy_t!(
    bait_button,
    "AutoFishing/1920x1080/switch_bait.png",
    "BaitButton",
    Rect {
        x: SCREEN_W - SCREEN_W / 2,
        y: SCREEN_H - SCREEN_H / 4,
        width: SCREEN_W / 2,
        height: SCREEN_H / 4,
    },
    0.7
);

// 等待咬钩
lazy_t!(
    wait_bite_button,
    "AutoFishing/1920x1080/wait_bite.png",
    "WaitBiteButton",
    Rect {
        x: SCREEN_W - SCREEN_W / 2,
        y: SCREEN_H - SCREEN_H / 4,
        width: SCREEN_W / 2,
        height: SCREEN_H / 4,
    },
    0.8
);

// 提竿按钮
lazy_t!(
    lift_rod_button,
    "AutoFishing/1920x1080/lift_rod.png",
    "LiftRodButton",
    Rect {
        x: SCREEN_W - SCREEN_W / 2,
        y: SCREEN_H - SCREEN_H / 4,
        width: SCREEN_W / 2,
        height: SCREEN_H / 4,
    },
    0.8
);

// 退出钓鱼按钮
lazy_t!(
    exit_fishing_button,
    "AutoFishing/1920x1080/exit_fishing.png",
    "ExitFishingButton",
    Rect {
        x: SCREEN_W - SCREEN_W / 4,
        y: SCREEN_H - SCREEN_H / 4,
        width: SCREEN_W / 4,
        height: SCREEN_H / 4,
    },
    0.8
);

// 空格键提示
lazy_t!(
    space_button,
    "AutoFishing/1920x1080/space.png",
    "SpaceButton",
    Rect {
        x: SCREEN_W - SCREEN_W / 3,
        y: SCREEN_H - SCREEN_H / 5,
        width: SCREEN_W / 3,
        height: SCREEN_H / 5,
    },
    0.8
);

// 白色确认按钮
lazy_t!(
    btn_white_confirm,
    "Common/1920x1080/btn_white_confirm.png",
    "BtnWhiteConfirm",
    Rect { x: 0, y: 0, width: SCREEN_W, height: SCREEN_H },
    0.8
);

// 黑色确认按钮
lazy_t!(
    btn_black_confirm,
    "Common/1920x1080/btn_black_confirm.png",
    "BtnBlackConfirm",
    Rect { x: 0, y: 0, width: SCREEN_W, height: SCREEN_H },
    0.8
);

// F键提示
lazy_t!(
    pick_f,
    "AutoPick/1920x1080/F.png",
    "PickF",
    Rect {
        x: 1090,
        y: 330,
        width: 60,
        height: 420,
    },
    0.8
);
