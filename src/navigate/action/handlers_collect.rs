//! 拾取/收集动作实现

use std::time::Duration;
use evdev_rs::enums::EV_KEY;
use tokio::time::sleep;

use crate::device::action::GIAction;
use crate::device::keytype::KeyType;
use crate::navigate::error::NavigateError;

use super::context::{ActionContext, ActionHandler};

pub struct ElementalCollectHandler {
    pub element: ElementalType,
}

/// 元素类型
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementalType {
    /// 水
    Hydro,
    /// 雷
    Electro,
    /// 风
    Anemo,
    /// 火
    Pyro,
}

impl ElementalType {
    pub fn to_chinese(self) -> &'static str {
        match self {
            Self::Hydro => "水",
            Self::Electro => "雷",
            Self::Anemo => "风",
            Self::Pyro => "火",
        }
    }
}

static ELEM_COLLECT_AVATARS: &[(&str, ElementalType, bool, bool)] = &[
    ("芭芭拉", ElementalType::Hydro, true, true),
    ("莫娜", ElementalType::Hydro, true, false),
    ("珊瑚宫心海", ElementalType::Hydro, true, true),
    ("玛拉妮", ElementalType::Hydro, true, false),
    ("那维莱特", ElementalType::Hydro, true, true),
    ("芙宁娜", ElementalType::Hydro, true, false),
    ("妮露", ElementalType::Hydro, false, true),
    ("坎蒂斯", ElementalType::Hydro, false, true),
    ("行秋", ElementalType::Hydro, false, true),
    ("神里绫人", ElementalType::Hydro, false, true),
    ("丽莎", ElementalType::Electro, true, true),
    ("八重神子", ElementalType::Electro, true, false),
    ("瓦雷莎", ElementalType::Electro, true, false),
    ("雷电将军", ElementalType::Electro, false, true),
    ("久岐忍", ElementalType::Electro, false, true),
    ("北斗", ElementalType::Electro, false, true),
    ("菲谢尔", ElementalType::Electro, false, true),
    ("雷泽", ElementalType::Electro, false, true),
    ("砂糖", ElementalType::Anemo, true, true),
    ("鹿野院平藏", ElementalType::Anemo, true, true),
    ("流浪者", ElementalType::Anemo, true, false),
    ("闲云", ElementalType::Anemo, true, false),
    ("蓝砚", ElementalType::Anemo, true, false),
    ("枫原万叶", ElementalType::Anemo, false, true),
    ("珐露珊", ElementalType::Anemo, false, true),
    ("琳妮特", ElementalType::Anemo, false, true),
    ("温迪", ElementalType::Anemo, false, true),
    ("琴", ElementalType::Anemo, false, true),
    ("早柚", ElementalType::Anemo, false, true),
    ("烟绯", ElementalType::Pyro, true, true),
    ("迪卢克", ElementalType::Pyro, false, true),
    ("可莉", ElementalType::Pyro, true, true),
    ("班尼特", ElementalType::Pyro, false, true),
    ("香菱", ElementalType::Pyro, false, true),
    ("托马", ElementalType::Pyro, false, true),
    ("胡桃", ElementalType::Pyro, false, true),
    ("迪希雅", ElementalType::Pyro, false, true),
    ("夏沃蕾", ElementalType::Pyro, false, true),
    ("辛焱", ElementalType::Pyro, false, true),
    ("林尼", ElementalType::Pyro, false, true),
    ("宵宫", ElementalType::Pyro, false, true),
];

impl ActionHandler for ElementalCollectHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let avatars = ctx.avatars.ok_or_else(|| {
                NavigateError::Unsupported(format!(
                    "{}_collect: 缺少 CombatScenes 上下文",
                    self.element.to_chinese()
                ))
            })?;
            for av in avatars.avatars() {
                let Some(&(_, _, normal, skill)) = ELEM_COLLECT_AVATARS
                    .iter()
                    .find(|(n, e, _, _)| *n == av.name() && *e == self.element)
                else {
                    continue;
                };
                let scenes_ptr = avatars as *const crate::avatar::CombatScenes;
                let av_index = av.index;
                let switched = av
                    .try_switch(4, false, move || {
                        let s = unsafe { &*scenes_ptr };
                        s.select_by_index(av_index).map(|a| a.index)
                    })
                    .await
                    .map_err(|e| NavigateError::Other(format!("try_switch: {e}")))?;
                if !switched {
                    log::error!("切人失败,无法进行{}元素采集", self.element.to_chinese());
                    return Ok(());
                }
                if normal {
                    av.attack(100)
                        .await
                        .map_err(|e| NavigateError::Other(format!("attack: {e}")))?;
                } else if skill {
                    av.use_skill(false)
                        .await
                        .map_err(|e| NavigateError::Other(format!("use_skill: {e}")))?;
                }
                break;
            }
            Ok(())
        })
    }
}

/// 纳西妲E采集
pub struct NahidaCollectHandler;

impl ActionHandler for NahidaCollectHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let avatars = ctx
                .avatars
                .ok_or_else(|| NavigateError::Unsupported("nahida_collect: 缺少 CombatScenes".into()))?;
            let sim = ctx
                .sim
                .clone()
                .ok_or_else(|| NavigateError::Unsupported("nahida_collect: 缺少 Simulator".into()))?;
            let nahida = avatars
                .select_by_name("纳西妲")
                .ok_or_else(|| NavigateError::Other("队伍中未找到纳西妲角色".into()))?;
            let scenes_ptr = avatars as *const crate::avatar::CombatScenes;
            let nahida_index = nahida.index;
            let _ = nahida
                .try_switch(4, false, move || {
                    let s = unsafe { &*scenes_ptr };
                    s.select_by_index(nahida_index).map(|a| a.index)
                })
                .await
                .map_err(|e| NavigateError::Other(format!("try_switch: {e}")))?;
            let (x, mut y) = (400i32, -30i32);
            let mut i = 60;
            sim.move_mouse_by(0, 10000)
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(200)).await;
            sim.simulate(GIAction::ElementalSkill, KeyType::KeyDown)
                .await
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            let result: Result<(), NavigateError> = async {
                sleep(Duration::from_millis(200)).await;
                for _ in 0..15 {
                    sim.move_mouse_by(x, 500)
                        .map_err(|e| NavigateError::Device(e.to_string()))?;
                    sleep(Duration::from_millis(30)).await;
                }
                while i > 0 {
                    if let Some(c) = ctx.cancel.as_ref()
                        && c.load(std::sync::atomic::Ordering::SeqCst)
                    {
                        break;
                    }
                    i -= 1;
                    if i == 40 {
                        y -= 20;
                    }
                    sim.move_mouse_by(x, y)
                        .map_err(|e| NavigateError::Device(e.to_string()))?;
                    sleep(Duration::from_millis(30)).await;
                }
                Ok(())
            }
            .await;
            sim.simulate(GIAction::ElementalSkill, KeyType::KeyUp)
                .await
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(800)).await;
            ctx.device
                .mouse_click(
                    EV_KEY::BTN_MIDDLE,
                    Duration::ZERO,
                    Duration::from_millis(40),
                    Duration::from_millis(150),
                )
                .map_err(|e| NavigateError::Device(e.to_string()))?;
            sleep(Duration::from_millis(1000)).await;
            result
        })
    }
}

/// 莉奈娅采集
pub struct LinneaMiningHandler;

impl ActionHandler for LinneaMiningHandler {
    fn run<'a>(
        &'a self,
        ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            let avatars = ctx
                .avatars
                .ok_or_else(|| NavigateError::Unsupported("linnea_mining: 缺少 CombatScenes".into()))?;
            let linnea = avatars
                .select_by_name("莉奈娅")
                .ok_or_else(|| NavigateError::Other("队伍中未找到莉奈娅".into()))?;
            let scenes_ptr = avatars as *const crate::avatar::CombatScenes;
            let linnea_index = linnea.index;
            let _ = linnea
                .try_switch(4, false, move || {
                    let s = unsafe { &*scenes_ptr };
                    s.select_by_index(linnea_index).map(|a| a.index)
                })
                .await
                .map_err(|e| NavigateError::Other(format!("try_switch: {e}")))?;
            sleep(Duration::from_millis(500)).await;
            Err(NavigateError::Unsupported(
                "linnea_mining: YOLO 子任务未移植".into(),
            ))
        })
    }
}

pub struct UnsupportedHandler {
    pub name: &'static str,
    pub reason: &'static str,
}

impl ActionHandler for UnsupportedHandler {
    fn run<'a>(
        &'a self,
        _ctx: ActionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), NavigateError>> + 'a>> {
        Box::pin(async move {
            Err(NavigateError::Unsupported(format!(
                "action `{}` 未实现：{}",
                self.name, self.reason
            )))
        })
    }
}
