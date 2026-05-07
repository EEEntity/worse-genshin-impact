//! 路径点类型/动作枚举

use std::fmt;

/// 路径点类型`type`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WaypointType {
    Orientation,
    Path,
    Target,
    Teleport,
    /// 未知类型，保留兼容新增值
    Unknown,
}

impl WaypointType {
    pub fn from_code(code: &str) -> Self {
        match code {
            "orientation" => Self::Orientation,
            "path" => Self::Path,
            "target" => Self::Target,
            "teleport" => Self::Teleport,
            _ => Self::Unknown,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::Orientation => "orientation",
            Self::Path => "path",
            Self::Target => "target",
            Self::Teleport => "teleport",
            Self::Unknown => "",
        }
    }
}

impl fmt::Display for WaypointType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// 移动方式`move_mode`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveMode {
    Climb,
    Dash,
    Fly,
    Jump,
    Run,
    Swim,
    Walk,
    /// 未知类型，保留兼容新增值
    Unknown,
}

impl MoveMode {
    pub fn from_code(code: &str) -> Self {
        match code {
            "climb" => Self::Climb,
            "dash" => Self::Dash,
            "fly" => Self::Fly,
            "jump" => Self::Jump,
            "run" => Self::Run,
            "swim" => Self::Swim,
            "walk" => Self::Walk,
            _ => Self::Unknown,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::Climb => "climb",
            Self::Dash => "dash",
            Self::Fly => "fly",
            Self::Jump => "jump",
            Self::Run => "run",
            Self::Swim => "swim",
            Self::Walk => "walk",
            Self::Unknown => "",
        }
    }
}

impl fmt::Display for MoveMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// 路径点执行动作`action`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionCode {
    None,
    /// 下落攻击停止飞行
    StopFlying,
    /// 强制传送
    ForceTp,
    /// 纳西妲长E采集
    NahidaCollect,
    /// 尝试在周围拾取
    PickAround,
    /// 战斗
    Fight,
    /// 普通攻击
    NormalAttack,
    /// 元素战技
    ElementalSkill,
    /// 四叶印
    UpDownGrabLeaf,
    /// 水元素采集
    HydroCollect,
    /// 雷元素采集
    ElectroCollect,
    /// 风元素采集
    AnemoCollect,
    /// 火元素采集
    PyroCollect,
    /// 战斗策略脚本
    CombatScript,
    /// 采矿
    Mining,
    /// 莉奈娅采矿
    LinneaMining,
    /// 输出日志
    LogOutput,
    /// 钓鱼
    Fishing,
    /// 退出并重新登录
    ExitAndRelogin,
    /// 进出千星奇域
    EnterAndExitWonderland,
    /// 设置时间
    SetTime,
    /// 使用小道具
    UseGadget,
    /// 聚集材料
    PickUpCollect,
    /// 未识别的字符串原样保留
    Other(String),
}

impl ActionCode {
    pub fn from_optional(s: Option<&str>) -> Self {
        match s {
            None | Some("") => Self::None,
            Some(c) => Self::from_code(c),
        }
    }
    pub fn from_code(code: &str) -> Self {
        match code {
            "" => Self::None,
            "stop_flying" => Self::StopFlying,
            "force_tp" => Self::ForceTp,
            "nahida_collect" => Self::NahidaCollect,
            "pick_around" => Self::PickAround,
            "fight" => Self::Fight,
            "normal_attack" => Self::NormalAttack,
            "elemental_skill" => Self::ElementalSkill,
            "up_down_grab_leaf" => Self::UpDownGrabLeaf,
            "hydro_collect" => Self::HydroCollect,
            "electro_collect" => Self::ElectroCollect,
            "anemo_collect" => Self::AnemoCollect,
            "pyro_collect" => Self::PyroCollect,
            "combat_script" => Self::CombatScript,
            "mining" => Self::Mining,
            "linnea_mining" => Self::LinneaMining,
            "log_output" => Self::LogOutput,
            "fishing" => Self::Fishing,
            "exit_and_relogin" => Self::ExitAndRelogin,
            "wonderland_cycle" => Self::EnterAndExitWonderland,
            "set_time" => Self::SetTime,
            "use_gadget" => Self::UseGadget,
            "pick_up_collect" => Self::PickUpCollect,
            other => Self::Other(other.to_string()),
        }
    }
    /// 强制将当前路径点视为某种类型
    /// - `Some(Target)`/`Some(Path)`: 覆盖路径点自身类型
    /// - `None`/`Some(Unknown)`: 保留原路径点类型
    pub fn enforces_waypoint_type(&self) -> Option<WaypointType> {
        match self {
            Self::Fight => Some(WaypointType::Path),
            Self::HydroCollect
            | Self::ElectroCollect
            | Self::AnemoCollect
            | Self::PyroCollect => Some(WaypointType::Target),
            _ => None,
        }
    }
}
