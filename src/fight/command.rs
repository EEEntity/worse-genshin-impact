//! 解析/执行单条战斗指令

use std::time::Duration;

use crate::fight::method::Method;
use crate::avatar::avatar::{Avatar, AvatarError, WalkDir};

/// 解析/执行错误
#[derive(Debug)]
pub enum CommandError {
    Parse(String),
    Avatar(AvatarError),
    NotImplemented(&'static str),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(s) => write!(f, "解析战斗脚本失败：{s}"),
            Self::Avatar(e) => write!(f, "执行战斗指令失败：{e}"),
            Self::NotImplemented(s) => write!(f, "命令 `{s}` 未实现"),
        }
    }
}

impl std::error::Error for CommandError {}

impl From<AvatarError> for CommandError {
    fn from(e: AvatarError) -> Self {
        Self::Avatar(e)
    }
}

/// 单条指令
#[derive(Debug, Clone)]
pub struct CombatCommand {
    pub name: String,
    pub method: Method,
    pub args: Vec<String>,
    pub activating_round: Vec<u32>,
}

/// "当前角色"占位
pub const CURRENT_AVATAR_NAME: &str = "当前角色";

impl CombatCommand {
    /// 解析文本格式单条指令
    pub fn parse(name: &str, command: &str) -> Result<Self, CommandError> {
        let name = name.trim().to_string();
        let command = command.trim();
        let (method_str, args): (String, Vec<String>) = match command.find('(') {
            Some(start) => {
                let end = command
                    .find(')')
                    .ok_or_else(|| CommandError::Parse(format!("缺少右括号：{command}")))?;
                let m = command[..start].trim().to_string();
                let inner = &command[start + 1..end];
                let args = inner
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                (m, args)
            }
            None => (command.to_string(), vec![]),
        };
        let method = Method::parse(&method_str).ok_or_else(|| {
            CommandError::Parse(format!("战斗策略脚本中出现未知的方法：{method_str}"))
        })?;
        // 校验参数
        match method {
            Method::Walk => {
                if args.len() != 2 {
                    return Err(CommandError::Parse(
                        "walk方法必须有两个入参，第一个参数是方向，第二个参数是行走时间。例：walk(s, 0.2)".into(),
                    ));
                }
                let s: f64 = args[1]
                    .parse()
                    .map_err(|_| CommandError::Parse(format!("walk 时间不是数字：{}", args[1])))?;
                if !(s > 0.0) {
                    return Err(CommandError::Parse("行走时间必须大于0".into()));
                }
            }
            Method::W | Method::A | Method::S | Method::D => {
                if args.len() != 1 {
                    return Err(CommandError::Parse(format!(
                        "{}方法必须有一个入参，代表行走时间。例：d(0.5)",
                        method.primary()
                    )));
                }
            }
            Method::MoveBy => {
                if args.len() != 2 {
                    return Err(CommandError::Parse(
                        "moveby方法必须有两个入参，分别是x和y。例：moveby(100, 100)".into(),
                    ));
                }
            }
            Method::KeyDown | Method::KeyUp | Method::KeyPress => {
                if args.len() != 1 {
                    return Err(CommandError::Parse(format!(
                        "{}方法必须有一个入参，代表按键",
                        method.primary()
                    )));
                }
            }
            Method::Scroll => {
                if args.len() != 1 {
                    return Err(CommandError::Parse(
                        "scroll方法必须有一个入参，代表滚动格数。例：scroll(1) 或 scroll(-1)".into(),
                    ));
                }
                if args[0].parse::<i32>().is_err() {
                    return Err(CommandError::Parse("滚动格数必须是整数".into()));
                }
            }
            _ => {}
        }
        Ok(Self {
            name,
            method,
            args,
            activating_round: vec![],
        })
    }
    /// 在`avatar`上执行指令
    pub async fn execute_on(&self, avatar: &Avatar) -> Result<(), CommandError> {
        match self.method {
            Method::Skill => {
                let hold = self.args.iter().any(|a| a == "hold");
                let _wait = self.args.iter().any(|a| a == "wait");
                let fast = self.args.iter().any(|a| a == "fast");
                if fast && !avatar.is_skill_ready() {
                    return Ok(());
                }
                // 这里不应该用sleep等待，应该依赖OCR检测技能就绪
                if _wait {
                    let cd = avatar.get_skill_cd_seconds();
                    if cd > 0.0 {
                        avatar.wait((cd * 1000.0) as i32).await;
                    }
                }
                avatar.use_skill(hold).await?;
            }
            Method::Burst => avatar.use_burst().await?,
            Method::Attack => {
                let ms = parse_seconds_arg(&self.args, 0).unwrap_or(0);
                avatar.attack(ms).await?;
            }
            Method::Charge => {
                let ms = parse_seconds_arg(&self.args, 0).unwrap_or(0);
                avatar.charge(ms).await?;
            }
            Method::Walk => {
                let dir = WalkDir::from_key(&self.args[0])
                    .ok_or_else(|| CommandError::Parse(format!("walk 未知方向：{}", self.args[0])))?;
                let ms = parse_seconds_arg(&self.args, 1).unwrap_or(0);
                avatar.walk(dir, ms).await?;
            }
            Method::W => walk_one(avatar, WalkDir::W, &self.args).await?,
            Method::A => walk_one(avatar, WalkDir::A, &self.args).await?,
            Method::S => walk_one(avatar, WalkDir::S, &self.args).await?,
            Method::D => walk_one(avatar, WalkDir::D, &self.args).await?,
            Method::Wait => {
                let ms = parse_seconds_arg(&self.args, 0).unwrap_or(0);
                avatar.wait(ms).await;
            }
            Method::Ready => {
                // Avatar.Ready需要has_index_rect闭包；调用方未提供截图源时退化为 sleep 10ms
                // 之后修一下
                avatar
                    .ready(|| false)
                    .await?;
            }
            Method::Check => {
                // 在主循环中处理(结束检测)
                // 这里直接跳过
            }
            Method::Aim => return Err(CommandError::NotImplemented("aim")),
            Method::Dash => {
                let ms = parse_seconds_arg(&self.args, 0).unwrap_or(0);
                avatar.dash(ms).await?;
            }
            Method::Jump => {
                avatar.jump()?;
            }
            Method::MouseDown => {
                let key = self.args.first().map(String::as_str).unwrap_or("Left");
                avatar.mouse_down(key)?;
            }
            Method::MouseUp => {
                let key = self.args.first().map(String::as_str).unwrap_or("Left");
                avatar.mouse_up(key)?;
            }
            Method::Click => {
                let key = self.args.first().map(String::as_str).unwrap_or("Left");
                avatar.mouse_click(key).await?;
            }
            Method::MoveBy => {
                let x: i32 = self.args[0]
                    .parse()
                    .map_err(|_| CommandError::Parse(format!("moveby x 不是整数：{}", self.args[0])))?;
                let y: i32 = self.args[1]
                    .parse()
                    .map_err(|_| CommandError::Parse(format!("moveby y 不是整数：{}", self.args[1])))?;
                avatar.move_by(x, y)?;
            }
            Method::KeyDown | Method::KeyUp | Method::KeyPress => {
                // 之后再做
                return Err(CommandError::NotImplemented("key_down/up/press"));
            }
            Method::Scroll => {
                let clicks: i32 = self.args[0]
                    .parse()
                    .map_err(|_| CommandError::Parse(format!("scroll 不是整数：{}", self.args[0])))?;
                avatar.scroll(clicks)?;
            }
            Method::Round => {
                // 解析时已经处理(标记`activating_round`)，这里直接跳过
            }
        }
        Ok(())
    }
}

fn parse_seconds_arg(args: &[String], idx: usize) -> Option<i32> {
    let s: f64 = args.get(idx)?.parse().ok()?;
    Some(Duration::from_secs_f64(s).as_millis() as i32)
}

async fn walk_one(avatar: &Avatar, dir: WalkDir, args: &[String]) -> Result<(), CommandError> {
    let ms = parse_seconds_arg(args, 0).unwrap_or(0);
    avatar.walk(dir, ms).await?;
    Ok(())
}

impl std::fmt::Display for CombatCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<CombatCommand {} {}({:?}) (rounds {:?})>",
            self.name, self.method, self.args, self.activating_round
        )
    }
}
