use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::fight::FightRunner;
use crate::config::AutoFishingGlobalConfig;
use crate::navigate::path::ActionCode;
use crate::task::choose_talk_option::OcrHandle;

use super::context::ActionHandler;
use super::context::ActionPhase;
use super::handlers_basic::{
    ElementalSkillHandler, LogOutputHandler, MiningHandler, NormalAttackHandler, PickAroundHandler,
    PickUpCollectHandler, StopFlyingHandler, UpDownGrabLeafHandler, UseGadgetHandler,
};
use super::handlers_common_jobs::{
    ExitAndReloginHandler, SetTimeHandler, WonderlandCycleHandler,
};
use super::handlers_collect::{
    ElementalCollectHandler, ElementalType, LinneaMiningHandler, NahidaCollectHandler,
};
use super::handlers_combat::{CombatScriptHandler, FightHandler};
use super::handlers_fishing::FishingHandler;

pub struct ActionRegistry {
    handlers: HashMap<String, Box<dyn ActionHandler>>,
}

impl ActionRegistry {
    pub fn empty() -> Self {
        Self { handlers: HashMap::new() }
    }

    pub fn with_defaults() -> Self {
        let mut r = Self::empty();
        r.register("normal_attack", Box::new(NormalAttackHandler));
        r.register("elemental_skill", Box::new(ElementalSkillHandler));
        r.register("use_gadget", Box::new(UseGadgetHandler));
        r.register("stop_flying", Box::new(StopFlyingHandler));
        r.register("log_output", Box::new(LogOutputHandler));
        r.register("mining", Box::new(MiningHandler));
        r.register("pick_up_collect", Box::new(PickUpCollectHandler));
        r.register("pick_around", Box::new(PickAroundHandler));
        r.register("up_down_grab_leaf", Box::new(UpDownGrabLeafHandler));
        r.register("combat_script", Box::new(CombatScriptHandler));
        r.register("nahida_collect", Box::new(NahidaCollectHandler));
        r.register("linnea_mining", Box::new(LinneaMiningHandler));
        r.register("set_time", Box::new(SetTimeHandler));
        r.register("exit_and_relogin", Box::new(ExitAndReloginHandler));
        r.register("wonderland_cycle", Box::new(WonderlandCycleHandler));
        r.register(
            "hydro_collect",
            Box::new(ElementalCollectHandler { element: ElementalType::Hydro }),
        );
        r.register(
            "electro_collect",
            Box::new(ElementalCollectHandler { element: ElementalType::Electro }),
        );
        r.register(
            "anemo_collect",
            Box::new(ElementalCollectHandler { element: ElementalType::Anemo }),
        );
        r.register(
            "pyro_collect",
            Box::new(ElementalCollectHandler { element: ElementalType::Pyro }),
        );
        r
    }

    pub fn register_fight(
        &mut self,
        runner: Arc<dyn FightRunner>,
        script_root: PathBuf,
        default_team_csv: String,
        default_timeout: u64,
    ) {
        self.register(
            "fight",
            Box::new(FightHandler {
                runner,
                script_root,
                default_team_csv,
                default_timeout,
            }),
        );
    }

    pub fn register_fishing(
        &mut self,
        ocr: OcrHandle,
        config: AutoFishingGlobalConfig,
        model_path: PathBuf,
        set_time_cap: Option<Arc<dyn crate::task::choose_talk_option::ScreenCapturer>>,
    ) {
        self.register(
            "fishing",
            Box::new(FishingHandler {
                ocr,
                config,
                model_path,
                set_time_cap,
            }),
        );
    }

    pub fn register(&mut self, code: &str, handler: Box<dyn ActionHandler>) {
        self.handlers.insert(code.to_string(), handler);
    }

    pub fn get(&self, code: &ActionCode) -> Option<&dyn ActionHandler> {
        let key: &str = match code {
            ActionCode::None => return None,
            ActionCode::StopFlying => "stop_flying",
            ActionCode::ForceTp => "force_tp",
            ActionCode::NahidaCollect => "nahida_collect",
            ActionCode::PickAround => "pick_around",
            ActionCode::Fight => "fight",
            ActionCode::NormalAttack => "normal_attack",
            ActionCode::ElementalSkill => "elemental_skill",
            ActionCode::UpDownGrabLeaf => "up_down_grab_leaf",
            ActionCode::HydroCollect => "hydro_collect",
            ActionCode::ElectroCollect => "electro_collect",
            ActionCode::AnemoCollect => "anemo_collect",
            ActionCode::PyroCollect => "pyro_collect",
            ActionCode::CombatScript => "combat_script",
            ActionCode::Mining => "mining",
            ActionCode::LinneaMining => "linnea_mining",
            ActionCode::LogOutput => "log_output",
            ActionCode::Fishing => "fishing",
            ActionCode::ExitAndRelogin => "exit_and_relogin",
            ActionCode::EnterAndExitWonderland => "wonderland_cycle",
            ActionCode::SetTime => "set_time",
            ActionCode::UseGadget => "use_gadget",
            ActionCode::PickUpCollect => "pick_up_collect",
            ActionCode::Other(s) => s.as_str(),
        };
        self.handlers.get(key).map(|b| b.as_ref())
    }

    pub fn get_for_phase(&self, code: &ActionCode, phase: ActionPhase) -> Option<&dyn ActionHandler> {
        if !Self::matches_phase(code, phase) {
            return None;
        }
        self.get(code)
    }

    fn matches_phase(code: &ActionCode, phase: ActionPhase) -> bool {
        match code {
            ActionCode::None | ActionCode::ForceTp => false,
            ActionCode::UpDownGrabLeaf => phase == ActionPhase::BeforeMoveToTarget,
            ActionCode::StopFlying => phase == ActionPhase::BeforeMoveCloseToTarget,
            _ => phase == ActionPhase::AfterMoveToTarget,
        }
    }
}
