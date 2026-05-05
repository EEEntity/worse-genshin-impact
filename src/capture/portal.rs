//! XDG Desktop Portal
//! 返回对应node ID供GStreamer pipewiresrc使用
//! 内部单线程Tokio Runtime完成异步D-Bus交互
//! 对外暴露为普通阻塞函数

use std::os::fd::OwnedFd;
use ashpd::desktop::screencast::{
    OpenPipeWireRemoteOptions, Screencast, SelectSourcesOptions, SourceType, StartCastOptions,
};
use enumflags2::BitFlags;
use ashpd::desktop::CreateSessionOptions;
use tokio::runtime::Builder;
use super::CaptureError;

/// 弹出窗口选择器，等待用户选择
/// 返回node ID和PipeWire remote fd
pub fn select_window() -> Result<(u32, OwnedFd), CaptureError> {
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e|CaptureError::Portal(format!("tokio runtime init: {e}")))?;
    rt.block_on(select_window_async())
}

async fn select_window_async() -> Result<(u32, OwnedFd), CaptureError> {
    let proxy = Screencast::new()
        .await
        .map_err(|e|CaptureError::Portal(format!("Screencast::new: {e}")))?;
    let session = proxy
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(|e|CaptureError::Portal(format!("create_session: {e}")))?;
    proxy
        .select_sources(
            &session,
            SelectSourcesOptions::default().set_sources(BitFlags::from(SourceType::Window)),
        )
        .await
        .map_err(|e|CaptureError::Portal(format!("select_sources: {e}")))?
        .response()
        .map_err(|e|CaptureError::Portal(format!("select_sources response: {e}")))?;
    let streams = proxy
        .start(&session, None, StartCastOptions::default())
        .await
        .map_err(|e| CaptureError::Portal(format!("start: {e}")))?
        .response()
        .map_err(|e| CaptureError::Portal(format!("start response: {e}")))?;
    let node_id = streams
        .streams()
        .first()
        .map(|s| s.pipe_wire_node_id())
        .ok_or_else(|| CaptureError::Portal("portal returned no streams".to_string()))?;
    let pw_fd = proxy
        .open_pipe_wire_remote(&session, OpenPipeWireRemoteOptions::default())
        .await
        .map_err(|e| CaptureError::Portal(format!("open_pipe_wire_remote: {e}")))?;
    Ok((node_id, pw_fd))
}
