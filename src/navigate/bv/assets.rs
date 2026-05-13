//! 加载模板资源
//! 有大量常数需要修改

use std::path::PathBuf;
use std::sync::OnceLock;
use opencv::core::{Mat, Rect};
use opencv::imgcodecs::{IMREAD_COLOR, imread};
use opencv::prelude::MatTraitConst;

use crate::navigate::error::NavigateError;

#[deprecated(note = "迁移常量至配置")]
// TEMPLATES_ROOT
const ASSETS_ROOT: &str = "assets/templates";
const SCREEN_W: i32 = 1920;
const SCREEN_H: i32 = 1080;

/// 模板
pub struct Template {
    pub name: &'static str,
    pub mat: Mat,
    /// 搜索区域
    pub roi: Rect,
    /// 默认匹配阈值
    pub threshold: f64,
}

struct SyncTemplate(Template);
unsafe impl Sync for SyncTemplate {}
unsafe impl Send for SyncTemplate {}

fn load(rel_path: &str) -> Result<Mat, NavigateError> {
    let path: PathBuf = PathBuf::from(ASSETS_ROOT).join(rel_path);
    let s = path.to_string_lossy().to_string();
    let mat = imread(&s, IMREAD_COLOR).map_err(|e| NavigateError::Cv(e.to_string()))?;
    if mat.empty() {
        return Err(NavigateError::Cv(format!(
            "无法加载模板 PNG：{s}"
        )));
    }
    Ok(mat)
}

macro_rules! lazy_template {
    ($fn_name:ident, $rel:expr, $name:expr, $roi:expr, $threshold:expr) => {
        pub fn $fn_name() -> Result<&'static Template, NavigateError> {
            static CELL: OnceLock<Result<SyncTemplate, String>> = OnceLock::new();
            let entry = CELL.get_or_init(|| {
                load($rel)
                    .map(|mat| {
                        SyncTemplate(Template {
                            name: $name,
                            mat,
                            roi: $roi,
                            threshold: $threshold,
                        })
                    })
                    .map_err(|e| e.to_string())
            });
            match entry {
                Ok(t) => Ok(&t.0),
                Err(e) => Err(NavigateError::Cv(e.clone())),
            }
        }
    };
}

// 派蒙菜单，判定游戏主界面
lazy_template!(
    paimon_menu,
    "Common/1920x1080/paimon_menu.png",
    "PaimonMenu",
    Rect { x: 0, y: 0, width: SCREEN_W / 4, height: SCREEN_H / 4 },
    0.85
);

// 右下角Space按键提示，判定飞行/攀爬状态
lazy_template!(
    key_space,
    "Common/1920x1080/key_space.png",
    "KeySpace",
    Rect {
        x: SCREEN_W - 350,
        y: SCREEN_H - 70,
        width: 200,
        height: 70,
    },
    0.85
);

// 右下角X按键提示，判定攀爬状态
lazy_template!(
    key_x,
    "Common/1920x1080/key_x.png",
    "KeyX",
    Rect {
        x: SCREEN_W - 350,
        y: SCREEN_H - 70,
        width: 200,
        height: 70,
    },
    0.85
);

// 大地图左侧缩放条按钮
lazy_template!(
    map_scale_button,
    "QuickTeleport/1920x1080/MapScaleButton.png",
    "MapScaleButton",
    Rect { x: 30, y: 440, width: 40, height: 200 },
    0.85
);

// 大地图左下角设置按钮
lazy_template!(
    map_settings_button,
    "QuickTeleport/1920x1080/MapSettingsButton.png",
    "MapSettingsButton",
    Rect { x: 25, y: 990, width: 58, height: 62 },
    0.85
);

// 大地图右上角关闭按钮
lazy_template!(
    map_close_button,
    "QuickTeleport/1920x1080/MapCloseButton.png",
    "MapCloseButton",
    Rect { x: SCREEN_W - 107, y: 19, width: 58, height: 58 },
    0.85
);

// 地下切换按钮，出现时在地下
lazy_template!(
    map_underground_switch,
    "QuickTeleport/1920x1080/MapUndergroundSwitchButton.png",
    "MapUndergroundSwitchButton",
    // BGI 没限定 ROI（用 3 通道全屏匹配），我们给一个右下角的合理范围
    Rect { x: SCREEN_W - 200, y: SCREEN_H - 200, width: 200, height: 200 },
    0.85
);

// 国家选择面板按钮
lazy_template!(
    map_choose,
    "QuickTeleport/1920x1080/MapChoose.png",
    "MapChoose",
    Rect { x: SCREEN_W - 480, y: 0, width: 100, height: 70 },
    0.85
);

// 选中传送点后的传送按钮
lazy_template!(
    go_teleport,
    "QuickTeleport/1920x1080/GoTeleport.png",
    "GoTeleport",
    Rect { x: 1440, y: SCREEN_H - 120, width: 100, height: 120 },
    0.85
);

// 选项列表图标搜索区域
pub const MAP_CHOOSE_ICON_ROI: Rect = Rect {
    x: 1270,
    y: 100,
    width: 50,
    height: SCREEN_H - 200, // = 880
};
// 选项图标多模板匹配阈值
pub const MAP_CHOOSE_ICON_THRESHOLD: f64 = 0.8;

macro_rules! map_choose_icon {
    ($fn_name:ident, $file:expr, $name:expr) => {
        lazy_template!(
            $fn_name,
            concat!("QuickTeleport/1920x1080/", $file),
            $name,
            MAP_CHOOSE_ICON_ROI,
            MAP_CHOOSE_ICON_THRESHOLD
        );
    };
}

map_choose_icon!(map_choose_teleport_waypoint, "TeleportWaypoint.png", "TeleportWaypoint");
map_choose_icon!(map_choose_statue, "StatueOfTheSeven.png", "StatueOfTheSeven");
map_choose_icon!(map_choose_domain, "Domain.png", "Domain");
map_choose_icon!(map_choose_domain2, "Domain2.png", "Domain2");
map_choose_icon!(map_choose_obsidian_totem, "ObsidianTotemPole.png", "ObsidianTotemPole");
map_choose_icon!(map_choose_portable, "PortableWaypoint.png", "PortableWaypoint");
map_choose_icon!(map_choose_mansion, "Mansion.png", "Mansion");
map_choose_icon!(map_choose_subspace, "SubSpaceWaypoint.png", "SubSpaceWaypoint");
map_choose_icon!(map_choose_nodkrai_meeting, "NodKraiMeetingPoint.png", "NodKraiMeetingPoint");
map_choose_icon!(map_choose_tablet_of_tona, "TabletOfTona.png", "TabletOfTona");

// 返回所有MapChooseIcon模板
pub fn map_choose_icon_templates() -> Result<[&'static Template; 10], NavigateError> {
    Ok([
        map_choose_teleport_waypoint()?,
        map_choose_statue()?,
        map_choose_domain()?,
        map_choose_domain2()?,
        map_choose_obsidian_totem()?,
        map_choose_portable()?,
        map_choose_mansion()?,
        map_choose_subspace()?,
        map_choose_nodkrai_meeting()?,
        map_choose_tablet_of_tona()?,
    ])
}

// 秘境退出图标，出现时在秘境内
lazy_template!(
    in_domain,
    "Common/1920x1080/in_domain.png",
    "InDomain",
    Rect { x: 0, y: 0, width: SCREEN_W / 4, height: SCREEN_H / 4 },
    0.85
);

// 对话左上角禁用UI按钮
lazy_template!(
    disabled_ui_button,
    "AutoSkip/1920x1080/disabled_ui.png",
    "DisabledUiButton",
    Rect { x: 0, y: 0, width: SCREEN_W / 3, height: SCREEN_H / 8 },
    0.85
);

// 复苏弹窗确定按钮
lazy_template!(
    confirm_button,
    "AutoFight/1920x1080/confirm.png",
    "Confirm",
    Rect { x: SCREEN_W / 2, y: SCREEN_H / 2, width: SCREEN_W / 2, height: SCREEN_H / 2 },
    0.85
);

// 队伍/联机ROI
const PARTY_RECT: Rect = Rect {
    x: SCREEN_W - 65,
    y: 155,
    width: 35,
    height: 600,
};

lazy_template!(
    index_1,
    "Common/1920x1080/index_1.png",
    "Index1",
    PARTY_RECT,
    0.85
);
lazy_template!(
    index_2,
    "Common/1920x1080/index_2.png",
    "Index2",
    PARTY_RECT,
    0.85
);
lazy_template!(
    index_3,
    "Common/1920x1080/index_3.png",
    "Index3",
    PARTY_RECT,
    0.85
);
lazy_template!(
    index_4,
    "Common/1920x1080/index_4.png",
    "Index4",
    PARTY_RECT,
    0.85
);

// 当前出战角色
lazy_template!(
    current_avatar_threshold,
    "Common/1920x1080/current_avatar_threshold.png",
    "CurrentAvatarThreshold",
    Rect { x: SCREEN_W - 240, y: 155, width: 210, height: 600 },
    0.7
);

// 联机: 左上角1P图标，自己是房主
lazy_template!(
    one_p_icon,
    "AutoFight/1920x1080/1p.png",
    "1P",
    Rect { x: 0, y: 0, width: SCREEN_W / 4, height: SCREEN_H / 7 },
    0.85
);

// 联机: 右侧P图标
lazy_template!(
    p_icon,
    "AutoFight/1920x1080/p.png",
    "P",
    Rect {
        // BGI: Rect(W - W/12.5, H/5, W/12.5, H/2 - W/7)
        x: SCREEN_W - SCREEN_W / 12,
        y: SCREEN_H / 5,
        width: SCREEN_W / 12,
        height: SCREEN_H / 2 - SCREEN_W / 7,
    },
    0.85
);
