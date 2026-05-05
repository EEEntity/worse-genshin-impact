//! GStreamer管道+懒加载帧缓冲
//! `new-sample`只有gst::Sample的引用计数指针
//! 只有在调用get_region/with_frame时读像素数据

use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::{Arc, RwLock};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use super::CaptureError;

pub struct Capturer {
    pipeline: gst::Pipeline,
    /// 最新一帧的引用，None为未收到第一帧
    latest: Arc<RwLock<Option<gst::Sample>>>,
    /// 持有Pipewire remote fd
    _pw_fd: OwnedFd,
}

impl Capturer {
    /// 用Portal返回的node_id和pw_fd创建管道
    /// 但不直接启动采集
    pub fn new(node_id: u32, pw_fd: OwnedFd) -> Result<Self, CaptureError> {
        gst::init().map_err(|e|CaptureError::Gst(format!("gst::init: {e}")))?;
        let fd_num = pw_fd.as_raw_fd();
        let pipeline_desc = format!(
            "pipewiresrc fd={fd_num} path={node_id} ! \
             videoconvert ! \
             video/x-raw,format=RGB ! \
             appsink name=sink emit-signals=false max-buffers=1 drop=true sync=false");
        let element = gst::parse::launch(&pipeline_desc)
            .map_err(|e|CaptureError::Gst(format!("parse pipeline: {e}")))?;
        let pipeline = element
            .downcast::<gst::Pipeline>()
            .map_err(|_|CaptureError::Gst("pipeline downcast failed".to_string()))?;
        let appsink = pipeline
            .by_name("sink")
            .ok_or_else(||CaptureError::Gst("appsink 'sink' not found".to_string()))?
            .downcast::<gst_app::AppSink>()
            .map_err(|_|CaptureError::Gst("AppSink downcast failed".to_string()))?;
        let latest = Arc::new(RwLock::new(None::<gst::Sample>));
        let latest_cb = Arc::clone(&latest);
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move|sink|{
                    let sample = sink.pull_sample().map_err(|_|gst::FlowError::Error)?;
                    if let Ok(mut guard) = latest_cb.write() {
                        *guard = Some(sample);
                    }
                    Ok(gst::FlowSuccess::Ok)
                })
                .build()
        );
        Ok(Self { pipeline, latest, _pw_fd: pw_fd })
    }
    /// 启动管道，开始采集
    pub fn start(&self) -> Result<(), CaptureError> {
        self.pipeline
            .set_state(gst::State::Playing)
            .map_err(|e|CaptureError::Gst(format!("set_state Playing: {e}")))?;
        Ok(())
    }
    /// 停止采集
    /// Drop时也应停止
    pub fn stop(&self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
    /// 返回当前帧的(width, height)
    pub fn frame_size(&self) -> Option<(u32, u32)> {
        let guard = self.latest.read().ok()?;
        let sample = guard.as_ref()?;
        let caps = sample.caps()?;
        let s = caps.structure(0)?;
        let w = s.get::<i32>("width").ok()? as u32;
        let h = s.get::<i32>("height").ok()? as u32;
        Some((w, h))
    }
    /// 从最新帧中复制指定矩形区域，返回行主序RGB数据
    /// 
    /// `x`,`y`为区域左上角坐标，`w`,`h`为宽高，坐标系与窗口原始分辨率一致
    /// 未收到帧/坐标越界返回None
    pub fn get_region(&self, x: u32, y: u32, w: u32, h: u32) -> Option<Vec<u8>> {
        let guard = self.latest.read().ok()?;
        let sample = guard.as_ref()?;
        // 读实际分辨率
        let caps = sample.caps()?;
        let s = caps.structure(0)?;
        let frame_w = s.get::<i32>("width").ok()? as u32;
        let frame_h = s.get::<i32>("height").ok()? as u32;
        if x.checked_add(w)? > frame_w || y.checked_add(h)? > frame_h {
            return None;
        }
        let buf = sample.buffer()?;
        let map = buf.map_readable().ok()?;
        let data = map.as_slice();
        // 反推每行字节数(4 bytes stride)
        // 用buf_size/height得到真实stride
        let stride = data.len() / frame_h as usize;
        let x = x as usize;
        let y = y as usize;
        let w = w as usize;
        let h = h as usize;
        let mut region = Vec::with_capacity(w*h*3);
        for row in y..y + h {
            let start = row * stride + x * 3;
            region.extend_from_slice(&data[start..start+w*3])
        }
        Some(region)
    }
    /// zero-copy 完整帧
    /// 可以直接将&[u8]给OpenCV Mat
    /// 未收到帧返回None
    pub fn with_frame<F, R>(&self, f:F) -> Option<R>
    where 
        F: FnOnce(&[u8]) -> R,
    {
        let guard = self.latest.read().ok()?;
        let sample = guard.as_ref()?;
        let buf = sample.buffer()?;
        let map = buf.map_readable().ok()?;
        Some(f(map.as_slice()))
    }
}

impl Drop for Capturer {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}
