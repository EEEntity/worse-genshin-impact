use std::time::Duration;
use evdev_rs::enums::{EV_ABS, EV_KEY, EV_REL};

/// 虚拟输入设备相关配置
/// 最终由uinput上报事件，要求足够的权限
/// 
/// 设备信息
pub const DEVICE_NAME: &str = "gi-device";
pub const DEVICE_PHYS: &str = "gi-device";

/// 创建uinput设备后等待系统注册完成的时间
pub const DEVICE_INIT_DELAY: Duration = Duration::from_secs(2);

/// 相对轴
pub const GI_REL_AXES: &[EV_REL] = &[
    EV_REL::REL_X,
    EV_REL::REL_Y,
    EV_REL::REL_WHEEL,
];

/// 绝对轴范围(axis, min, max)
// 实际上是输入设备自身的绝对轴范围，和屏幕分辨率无关
#[deprecated(note = "需要根据屏幕分辨率和游戏窗口大小调整数值")]
pub const GI_ABS_AXES: &[(EV_ABS, i32, i32)] = &[
    (EV_ABS::ABS_X, 0, 2560),
    (EV_ABS::ABS_Y, 0, 1440),
];

/// 按键
pub const GI_KEYS: &[EV_KEY] = &[
    // 动作/UI
    EV_KEY::KEY_W,
    EV_KEY::KEY_A,
    EV_KEY::KEY_S,
    EV_KEY::KEY_D,
    EV_KEY::KEY_E,
    EV_KEY::KEY_Q,
    EV_KEY::KEY_R,
    EV_KEY::KEY_F,
    EV_KEY::KEY_G,
    EV_KEY::KEY_T,
    EV_KEY::KEY_V,
    EV_KEY::KEY_X,
    EV_KEY::KEY_Z,
    EV_KEY::KEY_B,
    EV_KEY::KEY_C,
    EV_KEY::KEY_J,
    EV_KEY::KEY_L,
    EV_KEY::KEY_M,
    EV_KEY::KEY_O,
    EV_KEY::KEY_P,
    EV_KEY::KEY_U,
    EV_KEY::KEY_Y,
    EV_KEY::KEY_SLASH,
    // 切人 1..=5
    EV_KEY::KEY_1,
    EV_KEY::KEY_2,
    EV_KEY::KEY_3,
    EV_KEY::KEY_4,
    EV_KEY::KEY_5,
    EV_KEY::KEY_SPACE,
    EV_KEY::KEY_TAB,
    EV_KEY::KEY_X,
    EV_KEY::KEY_ESC,
    EV_KEY::KEY_ENTER,
    EV_KEY::KEY_BACKSPACE,
    EV_KEY::KEY_LEFTSHIFT,
    EV_KEY::KEY_LEFTCTRL,
    EV_KEY::KEY_LEFTALT,
    // F1..=F12
    EV_KEY::KEY_F1,
    EV_KEY::KEY_F2,
    EV_KEY::KEY_F3,
    EV_KEY::KEY_F4,
    EV_KEY::KEY_F5,
    EV_KEY::KEY_F6,
    EV_KEY::KEY_F7,
    EV_KEY::KEY_F8,
    EV_KEY::KEY_F9,
    EV_KEY::KEY_F10,
    EV_KEY::KEY_F11,
    EV_KEY::KEY_F12,
    // 鼠标
    EV_KEY::BTN_LEFT,
    EV_KEY::BTN_RIGHT,
    EV_KEY::BTN_MIDDLE,
];

/// 时序常量
/// 
/// `KeyType::KeyPress`按下保持时间
pub const KEY_PRESS_DURATION_MS: u64 = 40;
pub const KEY_PRESS_DURATION: Duration = Duration::from_millis(KEY_PRESS_DURATION_MS);

/// `KeyType::Hold`默认按住时长
pub const HOLD_DURATION_MS: u64 = 1000;
pub const HOLD_DURATION: Duration = Duration::from_millis(HOLD_DURATION_MS);

/// 切换角色后等待UI更新
pub const SWITCH_AVATAR_WAIT_MS: u64 = 250;

/// 按Esc关闭界面后的等待
pub const ESC_CLOSE_WAIT_MS: u64 = 200;

/// 游泳脱困二次确认延迟
pub const SWIM_CONFIRM_WAIT_MS: u64 = 800;

/// 游泳脱困face_to超时
pub const SWIM_FACE_TIMEOUT_MS: u64 = 2000;

/// 游泳脱困move_to超时
pub const SWIM_MOVE_TIMEOUT_MS: u64 = 15000;

/// 战斗结束检测的延迟(等待结算UI出现)
pub const FIGHT_FINISH_DELAY_MS: u64 = 1500;

/// 战斗结束检测前的二次延迟
pub const FIGHT_FINISH_DETECT_DELAY_MS: u64 = 450;

/// 视角旋转闭环采样间隔
pub const ROTATE_POLL_INTERVAL_MS: u64 = 50;

/// 普通攻击连击间隔
pub const ATTACK_INTERVAL_MS: u64 = 100;

/// 单个路点最大执行时间
pub const WAYPOINT_TIMEOUT_S: u64 = 240;
