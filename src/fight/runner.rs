//! 对外公开接口

use std::path::PathBuf;
use std::sync::Arc;

use crate::fight::command::CommandError;
use crate::avatar::avatar::CancelFlag;
use crate::device::simulator::Simulator;

/// 启动一次战斗
pub trait FightRunner: Send + Sync {
    /// 运行`script_path`指向的战斗脚本路径
    ///
    /// `team_names_csv`：命名(中/英/别名，逗号或分号分隔)
    fn run<'a>(
        &'a self,
        script_path: PathBuf,
        team_names_csv: String,
        timeout_seconds: u64,
        sim: Arc<Simulator>,
        cancel: CancelFlag,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), CommandError>> + Send + 'a>>;
}
