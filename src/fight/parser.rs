//! 脚本解析
//! 
//! # 格式
//! ```text
//! //或#开头注释
//! 角色名 方法1, 方法2(arg), 方法3(a,b); 方法4
//! // | 分隔：round标记激活回合
//! 角色名 round(1) | skill, attack
//! // 没有角色名时为当前角色
//! attack(0.5)
//! ```

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::fight::command::{CombatCommand, CommandError, CURRENT_AVATAR_NAME};
use crate::fight::method::Method;
use crate::avatar::assets::registry;
use crate::utils::sorted_read_dir_paths;

/// 战斗脚本
pub struct CombatScript {
    pub name: String,
    pub path: String,
    pub avatar_names: HashSet<String>,
    pub commands: Vec<CombatCommand>,
    /// 与当前队伍的"匹配人数"
    /// 用于[`CombatScriptBag::find`]排序
    pub match_count: usize,
}

/// 战斗脚本集合
#[derive(Debug, Default)]
pub struct CombatScriptBag {
    pub scripts: Vec<CombatScript>,
}

impl CombatScriptBag {
    pub fn new(scripts: Vec<CombatScript>) -> Self {
        Self { scripts }
    }
    pub fn single(script: CombatScript) -> Self {
        Self { scripts: vec![script] }
    }
    /// 找到最匹配脚本
    pub fn find(&mut self, avatar_names: &[String]) -> Result<&[CombatCommand], CommandError> {
        // 计算每个脚本的`match_count`
        // 记录是否有完全匹配
        let mut full_match: Option<usize> = None;
        for (i, s) in self.scripts.iter_mut().enumerate() {
            let mc = avatar_names
                .iter()
                .filter(|n| s.avatar_names.contains(*n))
                .count();
            s.match_count = mc;
            if mc == avatar_names.len() && full_match.is_none() {
                full_match = Some(i);
            }
        }
        if let Some(i) = full_match {
            log::info!("匹配到战斗脚本：{}", self.scripts[i].name);
            return Ok(&self.scripts[i].commands);
        }
        // 取匹配度最高
        self.scripts.sort_by(|a, b| b.match_count.cmp(&a.match_count));
        let best = self
            .scripts
            .first()
            .ok_or_else(|| CommandError::Parse("未匹配到任何战斗脚本".into()))?;
        if best.match_count == 0 {
            return Err(CommandError::Parse("未匹配到任何战斗脚本".into()));
        }
        log::warn!("未完整匹配到四人队伍，使用匹配度最高的队伍：{}", best.name);
        Ok(&self.scripts[0].commands)
    }
}

/// 从文件解析
pub fn read_and_parse<P: AsRef<Path>>(path: P) -> Result<CombatScriptBag, CommandError> {
    let p = path.as_ref();
    if p.is_file() {
        Ok(CombatScriptBag::single(parse_file(p)?))
    } else if p.is_dir() {
        let mut scripts = Vec::new();
        collect_txt_files(p, &mut |f| match parse_file(f) {
            Ok(s) => scripts.push(s),
            Err(e) => log::warn!("解析战斗脚本文件失败：{} , {}", f.display(), e),
        });
        if scripts.is_empty() {
            return Err(CommandError::Parse(format!(
                "战斗脚本文件不存在：{}",
                p.display()
            )));
        }
        Ok(CombatScriptBag::new(scripts))
    } else {
        Err(CommandError::Parse(format!(
            "战斗脚本文件不存在：{}",
            p.display()
        )))
    }
}

fn collect_txt_files(dir: &Path, push: &mut dyn FnMut(&Path)) {
    let Ok(entries) = sorted_read_dir_paths(dir) else { return };
    for p in entries {
        if p.is_dir() {
            collect_txt_files(&p, push);
        } else if p.extension().and_then(|s| s.to_str()) == Some("txt") {
            push(&p);
        }
    }
}

fn parse_file(p: &Path) -> Result<CombatScript, CommandError> {
    let text = fs::read_to_string(p)
        .map_err(|e| CommandError::Parse(format!("读取脚本失败 {}: {e}", p.display())))?;
    let mut s = parse_text(&text, true)?;
    s.path = p.display().to_string();
    s.name = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    Ok(s)
}

/// 从文本解析
pub fn parse_text(context: &str, validate: bool) -> Result<CombatScript, CommandError> {
    let mut lines: Vec<String> = Vec::new();
    for raw in context.split(['\n', '\r']) {
        let l = raw
            .trim()
            .replace('（', "(")
            .replace('）', ")")
            .replace('，', ",");
        if l.starts_with("//") || l.starts_with('#') || l.is_empty() {
            continue;
        }
        if l.contains(';') {
            for piece in l.split(';') {
                let t = piece.trim();
                if !t.is_empty() {
                    lines.push(t.to_string());
                }
            }
        } else {
            lines.push(l);
        }
    }
    parse_lines(&lines, validate)
}

fn parse_lines(lines: &[String], validate: bool) -> Result<CombatScript, CommandError> {
    let mut commands = Vec::new();
    let mut names: HashSet<String> = HashSet::new();
    for line in lines {
        let one = parse_line(line, &mut names, validate)?;
        commands.extend(one);
    }
    Ok(CombatScript {
        avatar_names: names,
        commands,
        ..Default::default()
    })
}

fn parse_line(
    line: &str,
    names: &mut HashSet<String>,
    validate: bool,
) -> Result<Vec<CombatCommand>, CommandError> {
    let line = line.trim();
    let mut character = CURRENT_AVATAR_NAME.to_string();
    let commands_part: &str;
    if let Some(idx) = line.find(' ') {
        let raw_char = &line[..idx];
        character = match registry().lookup(raw_char) {
            Some(c) => c.name.clone(),
            None => raw_char.to_string(),
        };
        commands_part = &line[idx + 1..];
    } else if validate {
        return Err(CommandError::Parse(
            "战斗脚本格式错误，必须以空格分隔角色和指令".into(),
        ));
    } else {
        commands_part = line;
    }
    let cmds = parse_line_commands(commands_part, &character)?;
    names.insert(character);
    Ok(cmds)
}

/// 解析指令
fn parse_line_commands(line_no_avatar: &str, name: &str) -> Result<Vec<CombatCommand>, CommandError> {
    let mut full = Vec::new();
    for part in line_no_avatar.split('|') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let mut cmds = parse_line_part(p, name)?;
        if !cmds.is_empty() && cmds[0].method == Method::Round {
            let round_cmd = cmds.remove(0);
            let rounds = parse_round_command(&round_cmd)?;
            for c in cmds.iter_mut() {
                c.activating_round = rounds.clone();
            }
        }
        full.extend(cmds);
    }
    Ok(full)
}

fn parse_line_part(line_no_avatar: &str, name: &str) -> Result<Vec<CombatCommand>, CommandError> {
    let mut out = Vec::new();
    let parts: Vec<&str> = line_no_avatar
        .split(',')
        .filter(|s| !s.is_empty())
        .collect();
    let mut i = 0;
    while i < parts.len() {
        let mut command = parts[i].to_string();
        if command.contains('(') && !command.contains(')') {
            let mut j = i + 1;
            while j < parts.len() {
                command.push(',');
                command.push_str(parts[j]);
                if command.matches('(').count() > 1 {
                    return Err(CommandError::Parse(format!(
                        "战斗脚本格式错误，指令 {command} 括号无法配对"
                    )));
                }
                if command.contains(')') {
                    i = j;
                    break;
                }
                j += 1;
            }
            if !(command.contains('(') && command.contains(')')) {
                return Err(CommandError::Parse(format!(
                    "战斗脚本格式错误，指令 {command} 括号不完整"
                )));
            }
        }
        out.push(CombatCommand::parse(name, &command)?);
        i += 1;
    }
    Ok(out)
}

fn parse_round_command(cmd: &CombatCommand) -> Result<Vec<u32>, CommandError> {
    if cmd.args.is_empty() {
        return Err(CommandError::Parse(
            "round方法必须有入参，代表在哪些回合执行后续指令".into(),
        ));
    }
    let mut rounds = Vec::new();
    for arg in &cmd.args {
        if let Some((s, e)) = arg.split_once('-') {
            let start: u32 = s
                .trim()
                .parse()
                .map_err(|_| CommandError::Parse("round 入参格式错误".into()))?;
            let end: u32 = e
                .trim()
                .parse()
                .map_err(|_| CommandError::Parse("round 入参格式错误".into()))?;
            if start > end || start == 0 {
                return Err(CommandError::Parse(
                    "round 入参格式错误，起始回合必须 ≤ 结束回合且大于0".into(),
                ));
            }
            rounds.extend(start..=end);
        } else {
            let r: u32 = arg
                .parse()
                .map_err(|_| CommandError::Parse(format!("round 入参格式错误：{arg}")))?;
            if r == 0 {
                return Err(CommandError::Parse("round 回合数必须大于0".into()));
            }
            rounds.push(r);
        }
    }
    Ok(rounds)
}
