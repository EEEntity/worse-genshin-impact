//! 战斗动作实现

use std::path::PathBuf;
use std::sync::Arc;

use crate::fight::FightRunner;
use crate::navigate::error::NavigateError;

use super::context::{ActionContext, ActionHandler};

pub struct FightHandler {
    pub runner: Arc<dyn FightRunner>,
    pub script_root: PathBuf,
    pub default_team_csv: String,
    pub default_timeout: u64,
}

impl ActionHandler for FightHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let (sim, cancel) = match (ctx.sim.clone(), ctx.cancel.clone()) {
                (Some(s), Some(c)) => (s, c),
                _ => {
                    return Err(NavigateError::Unsupported(
                        "fight: 缺少 Simulator/CancelFlag 上下文".into(),
                    ));
                }
            };
            let path = match ctx.action_params {
                Some(name) if !name.trim().is_empty() && name.trim() != "根据队伍自动选择" => {
                    self.script_root.join(format!("{}.txt", name.trim()))
                }
                _ => self.script_root.clone(),
            };
            self.runner
                .run(
                    path,
                    self.default_team_csv.clone(),
                    self.default_timeout,
                    sim,
                    cancel,
                )
                .await
                .map_err(|e| NavigateError::Other(format!("fight: {e}")))
        })
    }
}

pub struct CombatScriptHandler;

impl ActionHandler for CombatScriptHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let avatars = ctx.avatars.ok_or_else(|| {
                NavigateError::Unsupported("combat_script: 缺少 CombatScenes 上下文".into())
            })?;
            let text = ctx.action_params.unwrap_or("").trim();
            if text.is_empty() {
                log::error!("策略脚本 action_params 内容为空");
                return Ok(());
            }
            let script = crate::fight::parse_text(text, true)
                .map_err(|e| NavigateError::Other(format!("combat_script: 解析失败 {e}")))?;
            let need_current = script
                .avatar_names
                .contains(crate::fight::CURRENT_AVATAR_NAME);
            if !need_current {
                let has = avatars
                    .avatars()
                    .iter()
                    .any(|a| script.avatar_names.contains(a.name()));
                if !has {
                    log::error!("简易策略脚本要求的角色不存在！需要：{:?}", script.avatar_names);
                    return Ok(());
                }
            }
            for cmd in &script.commands {
                if let Some(c) = ctx.cancel.as_ref()
                    && c.load(std::sync::atomic::Ordering::SeqCst)
                {
                    return Err(NavigateError::Other("combat_script: 取消".into()));
                }
                let avatar_opt = if cmd.name == crate::fight::CURRENT_AVATAR_NAME {
                    avatars.avatars().first()
                } else {
                    avatars.select_by_name(&cmd.name)
                };
                let Some(avatar) = avatar_opt else { continue };
                if let Err(e) = cmd.execute_on(avatar).await {
                    log::warn!("combat_script 命令执行失败：{e}");
                }
            }
            Ok(())
        })
    }
}
