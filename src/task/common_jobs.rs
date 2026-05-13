//! 通用任务
//! 
//! - [`set_time`] 调整时间
//! - [`exit_and_relogin`] 退出重进
//! - [`wonderland_cycle`] 千星奇域进入/退出

use std::future::Future;
use std::time::Duration;
use evdev_rs::enums::EV_KEY;
use opencv::core::Mat;

use crate::device::simulator::Simulator;
use crate::navigate::bv::assets::Template;
use crate::navigate::bv::matcher::matches;
use crate::navigate::error::NavigateError;

use super::choose_talk_option::ScreenCapturer;

// 表盘常量
#[deprecated(note = "移植到配置，适配不同分辨率")]
const CENTER_X: f64 = 1441.0;
#[deprecated(note = "移植到配置，适配不同分辨率")]
const CENTER_Y: f64 = 501.6;

/// 错误类型
#[derive(Debug)]
#[allow(dead_code)]
pub enum CommonJobError {
    /// 等待元素出现超时
    ElementAppearTimeout(&'static str),
    /// 等待元素消失超时
    ElementDisappearTimeout(&'static str),
    Device(String),
    Navigate(NavigateError),
}

impl std::fmt::Display for CommonJobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ElementAppearTimeout(n) => write!(f, "等待元素 [{n}] 出现超时"),
            Self::ElementDisappearTimeout(n) => write!(f, "等待元素 [{n}] 消失超时"),
            Self::Device(s) => write!(f, "device error: {s}"),
            Self::Navigate(e) => write!(f, "{e}"),
        }
    }
}
impl std::error::Error for CommonJobError {}
impl From<NavigateError> for CommonJobError {
    fn from(e: NavigateError) -> Self {
        Self::Navigate(e)
    }
}

/// 等待元素出现
pub async fn wait_for_element_appear<C, F, Fut>(
    cap: &C,
    template: &Template,
    mut action: F,
    times: u32,
    interval: Duration,
) -> Result<bool, CommonJobError>
where
    C: ScreenCapturer + ?Sized,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(), CommonJobError>>,
{
    for _ in 0..times {
        action().await?;
        tokio::time::sleep(interval).await;
        let s = cap.capture().await?;
        if matches(&s, template)? {
            return Ok(true);
        }
    }
    Ok(false)
}
/// 等待元素消失
pub async fn wait_for_element_disappear<C, F, Fut>(
    cap: &C,
    template: &Template,
    mut action: F,
    times: u32,
    interval: Duration,
) -> Result<bool, CommonJobError>
where
    C: ScreenCapturer + ?Sized,
    F: FnMut(Mat) -> Fut,
    Fut: Future<Output = Result<(), CommonJobError>>,
{
    for _ in 0..times {
        let s0 = cap.capture().await?;
        action(s0).await?;
        tokio::time::sleep(interval).await;
        let s = cap.capture().await?;
        if !matches(&s, template)? {
            return Ok(true);
        }
    }
    Ok(false)
}

// 键盘/鼠标操作

fn click_at(sim: &Simulator, x: i32, y: i32) -> Result<(), CommonJobError> {
    sim.device()
        .teleport_mouse(x, y)
        .map_err(|e|CommonJobError::Device(e.to_string()))?;
    sim.left_button_click()
        .map_err(|e|CommonJobError::Device(e.to_string()))?;
    Ok(())
}
fn press_key(sim: &Simulator, key: EV_KEY) -> Result<(), CommonJobError> {
    sim.device()
        .press_keys(&[key], Duration::from_millis(50))
        .map_err(|e|CommonJobError::Device(e.to_string()))
}

/// 调整时间
/// 
/// 打开派蒙菜单 -> 时间界面 -> 点击/拖拽设置时间
pub async fn set_time<C: ScreenCapturer + ?Sized>(
    sim: &Simulator,
    cap: &C,
    hour: i32,
    minute: i32,
    skip_animation: bool,
) -> Result<(), CommonJobError> {
    const R1: f64 = 30.0;
    const R2: f64 = 150.0;
    const R3: f64 = 300.0;
    const STEP_MS: u64 = 50;
    let h_extra = (hour as f64 + minute as f64 / 60.0).floor() as i32;
    let m = hour * 60 + minute - h_extra * 60;
    let h = h_extra.rem_euclid(24);
    log::info!("设置时间到 {h} 点 {m} 分");
    press_key(sim, EV_KEY::KEY_ESC)?;
    tokio::time::sleep(Duration::from_millis(800)).await;
    click_at(sim, 50, 700)?; // 时钟图标，需要适配不同分辨率
    tokio::time::sleep(Duration::from_millis(900)).await;
    do_set_time_circle(sim, h, m, R1, R2, R3, STEP_MS).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    sim.device()
        .teleport_mouse(1500, 1000)
        .map_err(|e|CommonJobError::Device(e.to_string()))?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    sim.left_button_click()
        .map_err(|e|CommonJobError::Device(e.to_string()))?;
    tokio::time::sleep(Duration::from_millis(7)).await;
    if skip_animation {
        // 点一下跳过动画
        sim.device()
            .teleport_mouse(200, 200)
            .map_err(|e|CommonJobError::Device(e.to_string()))?;
        sim.left_button_down().map_err(|e|CommonJobError::Device(e.to_string()))?;
        tokio::time::sleep(Duration::from_millis(10)).await;
        sim.left_button_up().map_err(|e|CommonJobError::Device(e.to_string()))?;
        tokio::time::sleep(Duration::from_millis(1010)).await;
        let s = cap.capture().await?;
        if crate::navigate::bv::is_in_main_ui(&s)? {
            return Ok(());
        }
    }
    tokio::time::sleep(Duration::from_millis(3000)).await;
    let _ = wait_for_element_appear(
        cap,
        assets::page_close_white()?,
        || async { Ok(()) },
        25,
        Duration::from_millis(400),
    )
    .await?;
    return_main_ui(sim, cap).await?;
    Ok(())
}

fn position_at(r: f64, index: f64) -> (i32, i32) {
    let angle = index * std::f64::consts::PI / 720.0;
    let x = CENTER_X + r * angle.cos();
    let y = CENTER_Y + r * angle.sin();
    (x.round() as i32, y.round() as i32)
}

async fn do_set_time_circle(
    sim: &Simulator,
    hour: i32,
    minute: i32,
    r1: f64,
    r2: f64,
    r3: f64,
    step_ms: u64,
) -> Result<(), CommonJobError> {
    let end = (hour + 6) * 60 + minute - 20;
    let n = 3i32;
    for i in (-n + 1)..=0 {
        let (x, y) = position_at(r1, end as f64 + i as f64 * 1440.0 / n as f64);
        click_at(sim, x, y)?;
        tokio::time::sleep(Duration::from_millis(step_ms)).await;
    }
    let (x1, y1) = position_at(r2, end as f64 + 5.0);
    let (x2, y2) = position_at(r3, end as f64 + 20.5);
    sim.device()
        .teleport_mouse(x1, y1)
        .map_err(|e| CommonJobError::Device(e.to_string()))?;
    tokio::time::sleep(Duration::from_millis(50)).await;
    sim.left_button_down().map_err(|e| CommonJobError::Device(e.to_string()))?;
    tokio::time::sleep(Duration::from_millis(50)).await;
    sim.device()
        .teleport_mouse(x2, y2)
        .map_err(|e| CommonJobError::Device(e.to_string()))?;
    tokio::time::sleep(Duration::from_millis(50)).await;
    sim.left_button_up().map_err(|e| CommonJobError::Device(e.to_string()))?;
    tokio::time::sleep(Duration::from_millis(step_ms)).await;
    Ok(())
}

/// ESC回退主界面
async fn return_main_ui<C: ScreenCapturer + ?Sized>(
    sim: &Simulator,
    cap: &C,
) -> Result<(), CommonJobError> {
    let s0 = cap.capture().await?;
    if crate::navigate::bv::is_in_main_ui(&s0)? {
        return Ok(());
    }
    for _ in 0..8 {
        press_key(sim, EV_KEY::KEY_ESC)?;
        tokio::time::sleep(Duration::from_millis(900)).await;
        let s = cap.capture().await?;
        if crate::navigate::bv::is_in_main_ui(&s)? {
            return Ok(());
        }
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    press_key(sim, EV_KEY::KEY_ENTER)?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    press_key(sim, EV_KEY::KEY_ESC)?;
    Ok(())
}

/// 退出重进
/// 
/// 退出游戏 -> 进入游戏 -> 等待加载完成
pub async fn exit_and_relogin<C: ScreenCapturer + ?Sized>(
    sim: &Simulator,
    cap: &C,
) -> Result<(), CommonJobError> {
    log::info!("退出至登录页面");
    // ESC -> 等菜单
    if !wait_for_element_appear(
        cap,
        assets::menu_bag()?,
        || async {
            press_key(sim, EV_KEY::KEY_ESC)?;
            Ok(())
        },
        10,
        Duration::from_millis(1200),
    )
    .await?
    {
        return Err(CommonJobError::ElementAppearTimeout("MenuBag"));
    }
    // 点左下角"退出"按钮 -> 等待确认弹窗
    if !wait_for_element_appear(
        cap,
        assets::auto_wood_confirm()?,
        || async {
            click_at(sim, 50, 1080 - 50)?;
            Ok(())
        },
        5,
        Duration::from_millis(800),
    )
    .await?
    {
        return Err(CommonJobError::ElementAppearTimeout("AutoWoodConfirm"));
    }
    // 点击确认 -> 等弹窗消失
    if !wait_for_element_disappear(
        cap,
        assets::auto_wood_confirm()?,
        |screen| async move {
            if let Some(m) =
                crate::navigate::bv::matcher::find_template(&screen, assets::auto_wood_confirm()?)?
            {
                click_at(sim, m.center().x, m.center().y)?;
            }
            Ok(())
        },
        5,
        Duration::from_millis(1000),
    )
    .await?
    {
        return Err(CommonJobError::ElementDisappearTimeout("AutoWoodConfirm"));
    }
    tokio::time::sleep(Duration::from_millis(1000)).await;
    // 等"进入游戏"按钮出现
    log::info!("等待启动器：进入游戏");
    if !wait_for_element_appear(
        cap,
        assets::enter_game()?,
        || async { Ok(()) },
        120,
        Duration::from_millis(1000),
    )
    .await?
    {
        return Err(CommonJobError::ElementAppearTimeout("EnterGame"));
    }
    // 点击进入游戏直到消失
    if !wait_for_element_disappear(
        cap,
        assets::enter_game()?,
        |_| async {
            click_at(sim, 955, 666)?;
            Ok(())
        },
        120,
        Duration::from_millis(1000),
    )
    .await?
    {
        return Err(CommonJobError::ElementDisappearTimeout("EnterGame"));
    }
    // 等主界面(派蒙菜单)
    if !wait_for_element_appear(
        cap,
        crate::navigate::bv::assets::paimon_menu()?,
        || async { Ok(()) },
        120,
        Duration::from_millis(1000),
    )
    .await?
    {
        log::warn!("未检测到主界面，登录可能未完成");
    } else {
        log::info!("退出重新登录结束！");
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    Ok(())
}

/// 进千星奇域 -> 选第一个 -> 进大厅 -> ESC -> 返回世界
pub async fn wonderland_cycle<C: ScreenCapturer + ?Sized>(
    sim: &Simulator,
    cap: &C
) -> Result<(), CommonJobError> {
    log::info!("进入千星奇域");
    if !wait_for_element_appear(
        cap,
        assets::wonderland_close()?,
        || async {
            press_key(sim, EV_KEY::KEY_F6)?;
            Ok(())
        },
        10,
        Duration::from_millis(1000),
    )
    .await?
    {
        return Err(CommonJobError::ElementAppearTimeout("WonderlandClose"));
    }
    if !wait_for_element_appear(
        cap,
        assets::btn_black_confirm()?,
        || async {
            click_at(sim, 680, 310)?; // 分辨率
            Ok(())
        },
        5,
        Duration::from_millis(800),
    )
    .await?
    {
        return Err(CommonJobError::ElementAppearTimeout("BtnBlackConfirm<进入>"));
    }
    if !wait_for_element_disappear(
        cap,
        assets::btn_black_confirm()?,
        |screen| async move {
            if let Some(m) = crate::navigate::bv::matcher::find_template(
                &screen,
                assets::btn_black_confirm()?
            )?
            {
                click_at(sim, m.center().x, m.center().y)?;
            }
            Ok(())
        },
        5,
        Duration::from_millis(1000),
    )
    .await?
    {
        return Err(CommonJobError::ElementDisappearTimeout("BtnBlackConfirm"));
    }
    tokio::time::sleep(Duration::from_millis(1000)).await;
    if !wait_for_element_appear(
        cap,
        crate::navigate::bv::assets::paimon_menu()?,
        || async { Ok(()) },
        120,
        Duration::from_millis(1000),
    )
    .await?
    {
        log::warn!("未检测到主界面，可能未处于千星奇域");
    } else {
        log::info!("已进入千星奇域大厅，准备返回提瓦特");
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    if !wait_for_element_appear(
        cap,
        assets::btn_back_teyvat()?,
        || async {
            press_key(sim, EV_KEY::KEY_ESC)?;
            Ok(())
        },
        20,
        Duration::from_millis(800),
    )
    .await?
    {
        return Err(CommonJobError::ElementAppearTimeout("BtnBackTeyvat"));
    }
    if !wait_for_element_appear(
        cap,
        assets::btn_black_confirm()?,
        || async {
            let s = cap.capture().await?;
            if let Some(m) =
                crate::navigate::bv::matcher::find_template(&s, assets::btn_back_teyvat()?)?
            {
                click_at(sim, m.center().x, m.center().y)?;
            }
            Ok(())
        },
        5,
        Duration::from_millis(800),
    )
    .await?
    {
        return Err(CommonJobError::ElementAppearTimeout("BtnBlackConfirm(返回)"));
    }
    if !wait_for_element_disappear(
        cap,
        assets::btn_black_confirm()?,
        |screen| async move {
            if let Some(m) =
                crate::navigate::bv::matcher::find_template(&screen, assets::btn_black_confirm()?)?
            {
                click_at(sim, m.center().x, m.center().y)?;
            }
            Ok(())
        },
        5,
        Duration::from_millis(1000),
    )
    .await?
    {
        return Err(CommonJobError::ElementDisappearTimeout("BtnBlackConfirm"));
    }
    tokio::time::sleep(Duration::from_millis(1000)).await;
    if !wait_for_element_appear(
        cap,
        crate::navigate::bv::assets::paimon_menu()?,
        || async { Ok(()) },
        120,
        Duration::from_millis(1000),
    )
    .await?
    {
        log::warn!("未检测到主界面，可能未处于提瓦特");
    } else {
        log::info!("已返回提瓦特");
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    Ok(())
}

// 也许该合并一下
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
        let path: PathBuf = PathBuf::from("assets/GameTask").join(rel);
        let s = path.to_string_lossy().to_string();
        let m = imread(&s, IMREAD_COLOR).map_err(|e| NavigateError::Cv(e.to_string()))?;
        if m.empty() {
            return Err(NavigateError::Cv(format!("无法加载模板 PNG：{s}")));
        }
        Ok(m)
    }

    macro_rules! lazy_t {
        ($fn_name:ident, $rel:expr, $name:expr, $roi:expr, $th:expr) => {
            pub fn $fn_name() -> Result<&'static Template, NavigateError> {
                static CELL: OnceLock<Result<SyncT, String>> = OnceLock::new();
                let entry = CELL.get_or_init(|| {
                    load($rel)
                        .map(|mat| SyncT(Template { name: $name, mat, roi: $roi, threshold: $th }))
                        .map_err(|e| e.to_string())
                });
                entry.as_ref().map(|s| &s.0).map_err(|e| NavigateError::Cv(e.clone()))
            }
        };
    }
    // 图标资源，但需要适配不同分辨率
    // 右上角关闭
    lazy_t!(
        page_close_white,
        "Common/1920x1080/page_close_white.png",
        "PageCloseWhite",
        Rect { x: 1920 - 1920 / 8, y: 0, width: 1920 / 8, height: 1080 / 8 },
        0.8
    );
    // 全屏黑色确认
    lazy_t!(
        btn_black_confirm,
        "Common/1920x1080/btn_black_confirm.png",
        "BtnBlackConfirm",
        Rect { x: 0, y: 0, width: 1920, height: 1080 },
        0.8
    );
    // 全屏返回Teyvat按钮
    lazy_t!(
        btn_back_teyvat,
        "Common/1920x1080/btn_back_teyvat.png",
        "BtnBackTeyvat",
        Rect { x: 0, y: 0, width: 1920, height: 1080 },
        0.8
    );
    // 狗屁奇域关闭按钮
    lazy_t!(
        wonderland_close,
        "Common/1920x1080/wonderland_close.png",
        "WonderlandClose",
        Rect { x: 0, y: 0, width: 1920, height: 1080 },
        0.8
    );
    // 派蒙菜单左侧背包
    lazy_t!(
        menu_bag,
        "AutoWood/1920x1080/menu_bag.png",
        "MenuBag",
        Rect { x: 0, y: 0, width: 1920 / 2, height: 1080 },
        0.8
    );
    // 退出确认对话框确认
    // 用来刷新木材
    lazy_t!(
        auto_wood_confirm,
        "AutoWood/1920x1080/confirm.png",
        "AutoWoodConfirm",
        Rect { x: 0, y: 0, width: 1920, height: 1080 },
        0.8
    );
    // 启动器"进入游戏"
    lazy_t!(
        enter_game,
        "AutoWood/1920x1080/exit_welcome.png",
        "EnterGame",
        Rect { x: 0, y: 1080 / 2, width: 1920, height: 1080 / 2 },
        0.8
    );
}
