//! `GIAction`: 语义化动作枚举
//! 修改动作条目时，请同步检查:
//! - [`crate::device::keybindings::KeyBindingsConfig::default`] 是否给出默认绑定
//! - [`crate::device::constants::GI_KEYS`] 是否包含所需 `EV_KEY`

use serde::{Deserialize, Serialize};

/// 所有语义化操作
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "PascalCase")]
pub enum GIAction {
    MoveForward, // 向前移动
    MoveBackward, // 向后移动
    MoveLeft, // 向左移动
    MoveRight, // 向右移动
    NormalAttack, // 普通攻击
    ElementalSkill, // 元素战技
    ElementalBurst, // 元素爆发
    SprintKeyboard, // Shift冲刺
    SprintMouse, // 右键冲刺
    SwitchToWalkOrRun, // 切换走/跑
    SwitchAimingMode, // 切换瞄准模式
    Jump, // 跳跃
    Drop, // 落下
    PickUpOrInteract, // 拾取/交互
    QuickUseGadget, // 快速道具(Z)
    InteractionInSomeMode, // 特定模式下的交互(Alt切换光标等)
    QuestNavigation, // 任务导航
    AbandonChallenge, // 放弃挑战
    SwitchMember1, // 切换角色1
    SwitchMember2, // 切换角色2
    SwitchMember3, // 切换角色3
    SwitchMember4, // 切换角色4
    SwitchMember5, // 切换角色5
    ShortcutWheel, // 快捷轮盘
    OpenInventory, // 打开背包
    OpenCharacterScreen, // 打开角色界面
    OpenMap, // 打开地图
    OpenPaimonMenu, // 打开菜单
    OpenTheSettingsMenu, // 打开设置
    OpenAdventurerHandbook, // 打开冒险手册
    OpenCoOpScreen, // 打开联机界面
    OpenSpecialEnvironmentInformation, // 打开特殊环境信息
    OpenWishScreen, // 打开祈愿界面
    OpenBattlePassScreen, // 打开通行证界面
    OpenTheEventsMenu, // 打开活动界面
    OpenQuestMenu, // 打开任务界面
    OpenTheFurnishingScreen, // 打开尘歌壶?界面
    OpenStellarReunion, // 打开回归界面
    OpenNotificationDetails, // 打开通知详情
    HideUI, // 隐藏界面
    OpenChatScreen, // 打开聊天界面
    OpenPartySetupScreen, // 打开队伍配置界面
    CheckTutorialDetails, // 查看教程详情
    OpenFriendsScreen, // 打开好友界面
    ElementalSight, // 切换元素视野
    ShowCursor, // 显示光标
}

impl GIAction {
    pub const fn switch_member_for(index: u8) -> Option<Self> {
        match index {
            1 => Some(Self::SwitchMember1),
            2 => Some(Self::SwitchMember2),
            3 => Some(Self::SwitchMember3),
            4 => Some(Self::SwitchMember4),
            5 => Some(Self::SwitchMember5),
            _ => None,
        }
    }
}
