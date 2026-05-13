//! 战斗指令定义

/// 战斗指令
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    Skill,
    Burst,
    Attack,
    Charge,
    Wait,
    Ready,
    Check,
    Walk,
    W,
    A,
    S,
    D,
    Aim, // 不用这个
    Dash,
    Jump,
    MouseDown,
    MouseUp,
    Click,
    MoveBy,
    KeyDown,
    KeyUp,
    KeyPress,
    Scroll,
    /// 回合标记，本身不执行任何动作；解析阶段把后续指令的
    /// [`crate::fight::command::CombatCommand::activating_round`] 填上。
    Round,
}

impl Method {
    /// 指令的所有别名
    pub const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Skill => &["skill", "e"],
            Self::Burst => &["burst", "q"],
            Self::Attack => &["attack", "普攻", "普通攻击"],
            Self::Charge => &["charge", "重击"],
            Self::Wait => &["wait", "after", "等待"],
            Self::Ready => &["ready", "完成"],
            Self::Check => &["check", "检测"],
            Self::Walk => &["walk", "行走"],
            Self::W => &["w"],
            Self::A => &["a"],
            Self::S => &["s"],
            Self::D => &["d"],
            Self::Aim => &["aim", "r", "瞄准"],
            Self::Dash => &["dash", "冲刺"],
            Self::Jump => &["jump", "j", "跳跃"],
            Self::MouseDown => &["mousedown"],
            Self::MouseUp => &["mouseup"],
            Self::Click => &["click"],
            Self::MoveBy => &["moveby"],
            Self::KeyDown => &["keydown"],
            Self::KeyUp => &["keyup"],
            Self::KeyPress => &["keypress"],
            Self::Scroll => &["scroll", "verticalscroll"],
            Self::Round => &["round"],
        }
    }
    /// 指令主名
    pub const fn primary(self) -> &'static str {
        self.aliases()[0]
    }
    /// 按别名查找
    pub fn parse(token: &str) -> Option<Self> {
        let key = token.trim().to_ascii_lowercase();
        for m in Self::ALL {
            for &a in m.aliases() {
                if a == key || a == token.trim() {
                    return Some(*m);
                }
            }
        }
        None
    }
    /// 所有指令
    pub const ALL: &'static [Method] = &[
        Method::Skill,
        Method::Burst,
        Method::Attack,
        Method::Charge,
        Method::Wait,
        Method::Ready,
        Method::Check,
        Method::Walk,
        Method::W,
        Method::A,
        Method::S,
        Method::D,
        Method::Aim,
        Method::Dash,
        Method::Jump,
        Method::MouseDown,
        Method::MouseUp,
        Method::Click,
        Method::MoveBy,
        Method::KeyDown,
        Method::KeyUp,
        Method::KeyPress,
        Method::Scroll,
        Method::Round,
    ];
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.primary())
    }
}
