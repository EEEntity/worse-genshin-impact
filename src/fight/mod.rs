//! 自动战斗DSL

pub mod command;
pub mod method;
pub mod parser;
pub mod runner;
pub mod seek;
pub mod task;

pub use command::{CombatCommand, CommandError, CURRENT_AVATAR_NAME};
pub use method::Method;
pub use parser::{CombatScript, CombatScriptBag, parse_text, read_and_parse};
pub use runner::FightRunner;
pub use task::{AutoFightParam, AutoFightTask, FinishDetectConfig, ScreenFn};
