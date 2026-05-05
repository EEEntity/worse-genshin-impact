mod capturer;
mod portal;

pub use capturer::Capturer;

#[derive(Debug)]
pub enum CaptureError {
    /// XDG Desktop Portal交互失败
    Portal(String),
    /// GStreamer管道创建/运行失败
    Gst(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureError::Portal(msg) => write!(f, "[CaptureError::Portal] {msg}"),
            CaptureError::Gst(msg) => write!(f, "[CaptureError::Gst] {msg}"),
        }
    }
}

impl std::error::Error for CaptureError {}

/// 顺序Portal窗口选择-管道创建-启动采集，返回就绪的Capturer
/// 调用时候会弹出DE的窗口选择器，阻塞知道用户完成选择
pub fn init() -> Result<Capturer, CaptureError> {
    let (node_id, pw_fd) = portal::select_window()?;
    let capturer = Capturer::new(node_id, pw_fd)?;
    capturer.start()?;
    Ok(capturer)
}
