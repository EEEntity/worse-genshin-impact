//! 选择对话选项
//! 
//! 过场跳过/黑屏自动点击等功能还没实现

use std::sync::Arc;
use std::time::Duration;
use evdev_rs::enums::EV_KEY;
use opencv::core::{Mat, Rect, Scalar};
use opencv::imgproc::{COLOR_BGR2HSV, cvt_color};
use opencv::prelude::MatTraitConst;

use crate::device::simulator::Simulator;
use crate::navigate::bv::matcher::{find_template_all, matches};
use crate::navigate::error::NavigateError;
use crate::inference::ocr::{OcrEngine, OcrResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TalkOptionRes {
    /// 重试N次后仍未识别到目标文本
    NotFound,
    /// 文本命中但未通过橙色判定
    FoundButNotOrange,
    /// 命中并已发出点击
    FoundAndClick,
}

#[async_trait::async_trait]
pub trait ScreenCapturer: Send + Sync {
    async fn capture(&self) -> Result<Mat, NavigateError>;
}

/// 共享OCR
pub type OcrHandle = Arc<std::sync::Mutex<OcrEngine>>;

/// 是否在对话界面
pub fn is_in_talk_ui(screen: &Mat) -> Result<bool, NavigateError> {
    matches(screen, assets::disabled_ui()?)
}
/// 等待对话界面
pub async fn wait_for_talk_ui<C: ScreenCapturer + ?Sized>(
    cap: &C,
    retry_times: u32,
) -> Result<bool, NavigateError> {
    for _ in 0..retry_times {
        let s = cap.capture().await?;
        if is_in_talk_ui(&s)? {
            return Ok(true);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Ok(false)
}
/// 单个选项选择
pub async fn single_select_text<C: ScreenCapturer + ?Sized>(
    cap: &C,
    sim: &Simulator,
    ocr: &OcrHandle,
    option: &str,
    skip_times: u32,
    is_orange: bool,
) -> Result<TalkOptionRes, NavigateError> {
    if !wait_for_talk_ui(cap, 10).await? {
        log::error!("选项选择：当前界面不在对话选项界面");
        return Ok(TalkOptionRes::NotFound);
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut first_ocr_option = true;
    for _ in 0..skip_times {
        let region = cap.capture().await?;
        let option_regions = recognize_option(&region, ocr)?;
        let regions = match option_regions {
            None => {
                // 没识别到气泡 -> Space推进, 等500ms
                press_space(sim);
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
            Some(rs) => {
                if first_ocr_option {
                    // 首次识别延迟1s, 重新识别一次保证完整
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    first_ocr_option = false;
                }
                rs
            }
        };
        for opt in &regions {
            if !opt.text.contains(option) {
                continue;
            }
            if is_orange {
                let cropped = crop_owned(&region, opt.bbox)?;
                if !is_orange_option(&cropped)? {
                    return Ok(TalkOptionRes::FoundButNotOrange);
                }
            }
            click_ocr_region(sim, opt);
            tokio::time::sleep(Duration::from_millis(300)).await;
            return Ok(TalkOptionRes::FoundAndClick);
        }
    }
    Ok(TalkOptionRes::NotFound)
}
/// 在对话界面持续点击最后选项
pub async fn select_last_option_until_end<C, F>(
    cap: &C,
    sim: &Simulator,
    end_action: Option<F>,
    retry: u32,
) -> Result<(), NavigateError>
where
    C: ScreenCapturer + ?Sized,
    F: Fn(&Mat) -> bool + Send,
{
    for _ in 0..retry {
        let region = cap.capture().await?;
        if is_in_talk_ui(&region)? {
            let hits = find_template_all(&region, assets::icon_option()?, 16)?;
            if !hits.is_empty() {
                // 取最大Y
                let lowest = hits.iter().max_by_key(|h| h.top_left.y).unwrap();
                let cx = lowest.top_left.x + lowest.width / 2;
                let cy = lowest.top_left.y + lowest.height / 2;
                let _ = sim.device().teleport_mouse(cx, cy);
                let _ = sim.left_button_click();
            } else {
                press_space(sim);
            }
        } else if crate::navigate::bv::is_in_main_ui(&region).unwrap_or(false) {
            break;
        } else if let Some(f) = end_action.as_ref() {
            if f(&region) {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Ok(())
}
/// 识别对话选项，按Y升序
pub fn recognize_option(
    screen: &Mat,
    ocr: &OcrHandle,
) -> Result<Option<Vec<OcrResult>>, NavigateError> {
    let icon_hits = find_template_all(screen, assets::icon_option()?, 16)?;
    if icon_hits.is_empty() {
        return Ok(None);
    }
    // 第一个(最下方)
    let lowest = icon_hits.iter().max_by_key(|h| h.top_left.y).unwrap();
    let w = screen.cols();
    let h = screen.rows();
    let scale = h as f64 / 1080.0;
    let off_x = (8.0 * scale) as i32;
    let off_w = (535.0 * scale) as i32;
    let off_h = (30.0 * scale) as i32;
    let ocr_x = lowest.top_left.x + lowest.width + off_x;
    let ocr_y = h / 8;
    let ocr_w = off_w;
    let ocr_h = lowest.top_left.y + lowest.height + off_h - h / 12;
    let roi = Rect { x: ocr_x, y: ocr_y, width: ocr_w, height: ocr_h };
    let roi = clip_rect(roi, w, h);
    if roi.width <= 0 || roi.height <= 0 {
        return Ok(None);
    }
    let cropped = Mat::roi(screen, roi).map_err(|e| NavigateError::Cv(e.to_string()))?;
    let owned = cropped.clone_pointee();
    let rgb = crate::navigate::tp::bgr_mat_to_rgb_bytes(&owned)?;
    let mut ocr_lock = ocr.lock().expect("ocr mutex poisoned");
    let mut results = ocr_lock
        .run(&rgb, roi.width as u32, roi.height as u32)
        .map_err(|e| NavigateError::Other(format!("recognize_option: {e}")))?;
    drop(ocr_lock);
    // bbox ROI坐标 -> 屏幕坐标
    for r in &mut results {
        r.bbox[0] = r.bbox[0].saturating_add(roi.x as u32);
        r.bbox[1] = r.bbox[1].saturating_add(roi.y as u32);
    }
    // 按Y升序
    results.sort_by_key(|r| r.bbox[1]);
    // 过滤None/短纯英数字/Y间距>150
    let mut kept: Vec<OcrResult> = Vec::with_capacity(results.len());
    for i in 0..results.len() {
        let r = &results[i];
        if r.text.is_empty() {
            continue;
        }
        if r.text.chars().count() < 5 && is_en_or_num(&r.text) {
            continue;
        }
        if i + 1 < results.len() {
            let dy = results[i + 1].bbox[1] as i32 - r.bbox[1] as i32;
            if dy > 150 {
                log::debug!("recognize_option: 忽略 Y 间距过大项: {}", r.text);
                continue;
            }
        }
        kept.push(r.clone());
    }
    Ok(Some(kept))
}

/// 点击OCR结果区域
fn click_ocr_region(sim: &Simulator, r: &OcrResult) {
    let cx = r.bbox[0] as i32 + r.bbox[2] as i32 / 2;
    let cy = r.bbox[1] as i32 + r.bbox[3] as i32 / 2;
    let _ = sim.device().teleport_mouse(cx, cy);
    let _ = sim.left_button_click();
    if !r.text.is_empty() {
        log::info!("对话选项：{}", r.text);
    }
}
/// 继续对话(space)
fn press_space(sim: &Simulator) {
    let dev = sim.device();
    let _ = dev.key_down(EV_KEY::KEY_SPACE);
    std::thread::sleep(Duration::from_millis(30));
    let _ = dev.key_up(EV_KEY::KEY_SPACE);
}
/// 判断是否橙色选项
pub fn is_orange_option(text_bgr: &Mat) -> Result<bool, NavigateError> {
    if text_bgr.empty() {
        return Ok(false);
    }
    let mut hsv = Mat::default();
    cvt_color(text_bgr, &mut hsv, COLOR_BGR2HSV, 0, opencv::core::AlgorithmHint::ALGO_HINT_DEFAULT)
        .map_err(|e| NavigateError::Cv(e.to_string()))?;
    let lower = Scalar::new(10.0, 150.0, 150.0, 0.0);
    let upper = Scalar::new(25.0, 255.0, 255.0, 0.0);
    let mut mask = Mat::default();
    opencv::core::in_range(&hsv, &lower, &upper, &mut mask)
        .map_err(|e| NavigateError::Cv(e.to_string()))?;
    let nz = opencv::core::count_non_zero(&mask).map_err(|e| NavigateError::Cv(e.to_string()))?;
    let area = (mask.cols() * mask.rows()).max(1);
    let rate = nz as f64 / area as f64;
    log::debug!("识别到橙色文字区域占比:{rate}");
    Ok(rate > 0.1)
}

fn crop_owned(src: &Mat, bbox: [u32; 4]) -> Result<Mat, NavigateError> {
    let r = Rect {
        x: bbox[0] as i32,
        y: bbox[1] as i32,
        width: bbox[2] as i32,
        height: bbox[3] as i32,
    };
    let r = clip_rect(r, src.cols(), src.rows());
    if r.width <= 0 || r.height <= 0 {
        return Ok(Mat::default());
    }
    let v = Mat::roi(src, r).map_err(|e|NavigateError::Cv(e.to_string()))?;
    Ok(v.clone_pointee())
}
fn clip_rect(mut r: Rect, w: i32, h: i32) -> Rect {
    if r.x < 0 {
        r.width += r.x;
        r.x = 0;
    }
    if r.y < 0 {
        r.height += r.y;
        r.y = 0;
    }
    if r.x + r.width > w {
        r.width = w - r.x;
    }
    if r.y + r.height > h {
        r.height = h - r.y;
    }
    r
}
fn is_en_or_num(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric())
}

mod assets {
    use std::path::PathBuf;
    use std::sync::OnceLock;
    use opencv::core::{Mat, Rect};
    use opencv::imgcodecs::{IMREAD_COLOR, imread};
    use opencv::prelude::MatTraitConst;
    use crate::navigate::bv::assets::Template;
    use crate::navigate::error::NavigateError;

    struct SyncT(Template);
    unsafe impl Sync for SyncT {}
    unsafe impl Send for SyncT {}

    fn load(rel: &str) -> Result<Mat, NavigateError> {
        let path: PathBuf = PathBuf::from("assets/templates").join(rel);
        let s = path.to_string_lossy().to_string();
        let m = imread(&s, IMREAD_COLOR).map_err(|e| NavigateError::Cv(e.to_string()))?;
        if m.empty() {
            return Err(NavigateError::Cv(format!("无法加载模板 PNG：{s}")));
        }
        Ok(m)
    }
    pub fn disabled_ui() -> Result<&'static Template, NavigateError> {
        static CELL: OnceLock<Result<SyncT, String>> = OnceLock::new();
        let entry = CELL.get_or_init(|| {
            load("AutoSkip/1920x1080/disabled_ui.png")
                .map(|mat| {
                    SyncT(Template {
                        name: "DisabledUiButton",
                        mat,
                        // (0, 0, w/3, h/8)
                        // 需要适配不同分辨率
                        roi: Rect { x: 0, y: 0, width: 1920 / 3, height: 1080 / 8 },
                        threshold: 0.8,
                    })
                })
                .map_err(|e|e.to_string())
        });
        match entry {
            Ok(t) => Ok(&t.0),
            Err(e) => Err(NavigateError::Cv(e.clone())),
        }
    }
    pub fn icon_option() -> Result<&'static Template, NavigateError> {
        static CELL: OnceLock<Result<SyncT, String>> = OnceLock::new();
        let entry = CELL.get_or_init(|| {
            load("AutoSkip/1920x1080/icon_option.png")
                .map(|mat| {
                    SyncT(Template {
                        name: "OptionIcon",
                        mat,
                        // OptionRoi(w/2,h/12,w-w/2-w/6,h-h/12-10)
                        // 需要适配不同分辨率
                        roi: Rect {
                            x: 1920 / 2,
                            y: 1080 / 12,
                            width: 1920 - 1920 / 2 - 1920 / 6,
                            height: 1080 - 1080 / 12 - 10,
                        },
                        threshold: 0.8,
                    })
                })
                .map_err(|e| e.to_string())
        });
        match entry {
            Ok(t) => Ok(&t.0),
            Err(e) => Err(NavigateError::Cv(e.clone())),
        }
    }
}
