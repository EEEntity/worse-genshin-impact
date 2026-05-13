//! 动作执行
pub mod context;
pub mod handlers_basic;
pub mod handlers_common_jobs;
pub mod handlers_collect;
pub mod handlers_combat;
pub mod handlers_fishing;
pub mod registry;

pub use context::{ActionContext, ActionHandler, ActionPhase, ScreenProvider};
pub use handlers_fishing::FishingHandler;
pub use registry::ActionRegistry;
