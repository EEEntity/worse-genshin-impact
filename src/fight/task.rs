//! 战斗主循环
//! 
//! 还有一堆功能没实现

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use opencv::core::{Mat, MatTraitConst, Vec3b};

use crate::fight::command::{CombatCommand, CommandError};
use crate::fight::method::Method;
use crate::fight::parser::{CombatScriptBag, read_and_parse};
use crate::fight::seek::{ROTATION_LIMIT, SeekResult, SeekState, seek_and_fight};
use crate::avatar::avatar::CancelFlag;
use crate::avatar::predictor::{AvatarPredictor, OrtAvatarPredictor};
use crate::avatar::scenes::CombatScenes;
use crate::device::action::GIAction;
use crate::device::keytype::KeyType;
use crate::device::simulator::Simulator;
use crate::device::constants::{FIGHT_FINISH_DELAY_MS, FIGHT_FINISH_DETECT_DELAY_MS};

/// 截图源签名
pub type ScreenFn = Box<dyn FnMut() -> Option<Mat> + Send>;

/// 自动战斗参数
#[derive(Debug, Clone)]
pub struct AutoFightParam {
    /// 战斗脚本路径(文件或目录)
    pub script_path: std::path::PathBuf,
    /// 队伍名(初始化时使用，留空走截图识别YOLO分类器)
    pub team_names_csv: String,
    /// 战斗超时(sec)
    pub timeout_seconds: u64,
    /// 是否启用战斗结束检测
    pub fight_finish_detect_enabled: bool,
    /// 结束检测配置
    pub finish: FinishDetectConfig,
    /// `ActionSchedulerByCd`可被跳过的角色名列表
    /// 在主循环里若该角色当前`cd>0`且上次执行不是该角色，则跳过本次命令
    pub skip_avatars: Vec<String>,
    /// 进入战斗第一条命令前先做一次旋转寻敌
    pub is_first_check: bool,
    /// 每次释放Q前先寻敌确认是否有敌
    pub check_before_burst: bool,
    /// 旋转因子1-13，传给[`crate::fight::seek`]
    pub rotary_factor: i32,
}

#[derive(Debug, Clone)]
pub struct FinishDetectConfig {
    pub fast_check_enabled: bool,
    /// 多少秒过后允许触发fast check
    pub check_time_seconds: f64,
    /// 哪些角色名换出后立刻触发fast check
    pub check_names: Vec<String>,
    /// 检测前等待
    pub end_delay_ms: u64,
    /// 打开队伍菜单后等待
    pub detect_delay_ms: u64,
    /// 按角色覆盖`end_delay_ms`
    pub end_delay_by_name: std::collections::HashMap<String, u64>,
    /// 开启后 [`crate::fight::seek`]
    /// 在`check_fight_finish`/首次检查/Q前检查处生效
    pub rotate_find_enemy_enabled: bool,
}

impl Default for FinishDetectConfig {
    fn default() -> Self {
        Self {
            fast_check_enabled: false,
            check_time_seconds: 5.0,
            check_names: vec![],
            end_delay_ms: FIGHT_FINISH_DELAY_MS,
            detect_delay_ms: FIGHT_FINISH_DETECT_DELAY_MS,
            end_delay_by_name: std::collections::HashMap::new(),
            rotate_find_enemy_enabled: false,
        }
    }
}

/// 战斗主任务执行器
pub struct AutoFightTask {
    param: AutoFightParam,
    bag: CombatScriptBag,
    sim: Arc<Simulator>,
    cancel: CancelFlag,
    /// 旋转寻敌跨`check_fight_finish`调用的共享状态
    seek_state: std::sync::Mutex<SeekState>,
}

impl AutoFightTask {
    /// 加载脚本并创建任务
    pub fn new(
        param: AutoFightParam,
        sim: Arc<Simulator>,
        cancel: CancelFlag,
    ) -> Result<Self, CommandError> {
        let bag = read_and_parse(&param.script_path)?;
        Ok(Self {
            param,
            bag,
            sim,
            cancel,
            seek_state: std::sync::Mutex::new(SeekState::default()),
        })
    }
    /// 异步运行整个战斗任务
    pub async fn run(&mut self, mut screen: Option<ScreenFn>) -> Result<(), CommandError> {
        // 队伍初始化，配置模式优先，为空时回退YOLO
        let scenes = if self.param.team_names_csv.trim().is_empty() {
            self.initialize_team_from_yolo(screen.as_mut())?
        } else {
            CombatScenes::initialize_from_config(
                &self.param.team_names_csv,
                self.sim.clone(),
                self.cancel.clone(),
            )
        };
        if scenes.avatar_count() == 0 {
            return Err(CommandError::Parse("队伍初始化失败：无可用角色".into()));
        }
        let team_names: Vec<String> = scenes
            .avatars()
            .iter()
            .map(|a| a.combat.name.clone())
            .collect();
        // 匹配最佳脚本，并按队伍角色筛选可执行命令
        let commands_ref = self.bag.find(&team_names)?;
        let commands: Vec<CombatCommand> = commands_ref
            .iter()
            .filter(|c| {
                scenes.select_by_name(&c.name).is_some()
            })
            .cloned()
            .collect();
        if commands.is_empty() {
            return Err(CommandError::Parse("没有可用战斗脚本".into()));
        }
        // "可跳过角色名"集合：与命令角色名取交集
        let cmd_names: std::collections::HashSet<String> =
            commands.iter().map(|c| c.name.clone()).collect();
        let skip_set: std::collections::HashSet<String> = self
            .param
            .skip_avatars
            .iter()
            .filter(|n| cmd_names.contains(*n))
            .cloned()
            .collect();
        let all_can_be_skipped = cmd_names.iter().all(|n| skip_set.contains(n));
        // 主循环
        let timeout = Duration::from_secs(self.param.timeout_seconds.max(1));
        let timeout_start = Instant::now();
        let check_time = Duration::from_secs_f64(self.param.finish.check_time_seconds.max(0.0));
        let mut check_stopwatch = Instant::now();
        let mut last_fight_name = String::new();
        let mut fight_end = false;
        let mut count_fight = 0u32;
        let result = self
            .main_loop(
                &commands,
                &skip_set,
                all_can_be_skipped,
                &scenes,
                &mut screen,
                timeout,
                timeout_start,
                check_time,
                &mut check_stopwatch,
                &mut last_fight_name,
                &mut fight_end,
                &mut count_fight,
            )
            .await;
        // finally: 释放所有按键
        self.sim.release_all_keys();
        log::info!("战斗结束。共战斗 {count_fight} 次");
        result
    }
    /// 识别角色
    fn initialize_team_from_yolo(
        &self,
        screen: Option<&mut ScreenFn>,
    ) -> Result<CombatScenes, CommandError> {
        let cap = screen.ok_or_else(|| {
            CommandError::Parse(
                "team_names_csv 为空时需要 ScreenFn 截图源以识别队伍".into(),
            )
        })?;
        let frame = cap().ok_or_else(|| {
            CommandError::Parse("截图源未返回有效画面，无法识别队伍".into())
        })?;
        let predictor = OrtAvatarPredictor::load_default().map_err(|e| {
            CommandError::Parse(format!("加载 BgiAvatarSide YOLO 失败：{e:#}"))
        })?;
        let scenes = CombatScenes::initialize_from_screen(
            &frame,
            &predictor as &dyn AvatarPredictor,
            self.sim.clone(),
            self.cancel.clone(),
        )
        .map_err(|e|CommandError::Parse(format!("YOLO 识别队伍失败：{e}")))?;
        log::info!(
            "YOLO 识别队伍成功：{}",
            scenes
                .avatars()
                .iter()
                .map(|a| a.combat.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        Ok(scenes)
    }
    #[allow(clippy::too_many_arguments)]
    async fn main_loop(
        &self,
        commands: &[CombatCommand],
        skip_set: &std::collections::HashSet<String>,
        all_can_be_skipped: bool,
        scenes: &CombatScenes,
        screen: &mut Option<ScreenFn>,
        timeout: Duration,
        timeout_start: Instant,
        check_time: Duration,
        check_stopwatch: &mut Instant,
        last_fight_name: &mut String,
        fight_end: &mut bool,
        count_fight: &mut u32,
    ) -> Result<(), CommandError> {
        loop {
            if self.cancel.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            // "全员可跳过且全员cd>0 -> 强制等待最小cd"
            if all_can_be_skipped {
                let min_cd = commands
                    .iter()
                    .filter_map(|c| scenes.select_by_name(&c.name))
                    .map(|a| a.get_skill_cd_seconds())
                    .fold(f64::INFINITY, f64::min);
                if min_cd > 0.0 && min_cd.is_finite() {
                    log::info!("队伍中所有角色的技能都在冷却中, 等待{:.2}秒后继续", min_cd);
                    tokio::time::sleep(Duration::from_millis((min_cd * 1000.0).ceil() as u64)).await;
                }
            }
            let mut skip_fight_name = String::new();
            for i in 0..commands.len() {
                if self.cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    *fight_end = true;
                    break;
                }
                let command = &commands[i];
                let avatar = match scenes.select_by_name(&command.name) {
                    Some(a) => a,
                    None => continue,
                };
                // 初始寻敌
                if i == 0
                    && self.param.is_first_check
                    && self.param.finish.rotate_find_enemy_enabled
                {
                    let _ = self.run_seek(screen, true).await;
                }
                let last_was_same = *last_fight_name == command.name && skip_fight_name.is_empty();
                let allow_skip_check = (all_can_be_skipped || skip_set.contains(&command.name))
                    && !last_was_same;
                if allow_skip_check {
                    let cd = avatar.get_skill_cd_seconds();
                    if cd > 0.0 {
                        if skip_fight_name != command.name {
                            log::info!(
                                "{}cd冷却剩余{:.2}秒, 跳过此次行动",
                                command.name, cd
                            );
                        }
                        skip_fight_name = command.name.clone();
                        continue;
                    }
                    skip_fight_name.clear();
                }
                if timeout_start.elapsed() > timeout {
                    log::info!("战斗超时结束");
                    *fight_end = true;
                    break;
                }
                // `RotationCount`超限 -> 中止战斗
                if self.seek_state.lock().unwrap().rotation_count >= ROTATION_LIMIT {
                    log::info!("旋转次数达到上限，战斗结束");
                    *fight_end = true;
                    break;
                }
                // Q前寻敌处理
                if self.param.finish.rotate_find_enemy_enabled
                    && self.param.check_before_burst
                    && (command.method == Method::Burst
                        || command.args.iter().any(|a| a.eq_ignore_ascii_case("q")))
                {
                    *fight_end = self.check_fight_finish(screen).await;
                    if *fight_end {
                        break;
                    }
                }
                // 执行命令
                // 单条失败 -> log warn -> 继续
                if let Err(e) = command.execute_on(avatar).await {
                    log::warn!("战斗指令执行失败 {command} : {e}");
                }
                // 战斗人次统计：当前角色与下一条不同(or tail)
                let next_diff = i + 1 >= commands.len() || commands[i + 1].name != command.name;
                if next_diff {
                    *count_fight += 1;
                }
                // 触发结束检测
                if command.method == Method::Check {
                    *fight_end = self.check_fight_finish(screen).await;
                }
                *last_fight_name = command.name.clone();
                // 末尾或FastCheck触发
                if !*fight_end && self.param.fight_finish_detect_enabled {
                    let trigger = i == commands.len() - 1
                        || (self.param.finish.fast_check_enabled
                            && commands[i + 1].name != command.name
                            && ((self.param.finish.check_time_seconds > 0.0
                                && check_stopwatch.elapsed() > check_time)
                                || self.param.finish.check_names.contains(&command.name)));
                    if trigger {
                        *check_stopwatch = Instant::now();
                        *fight_end = self.check_fight_finish(screen).await;
                    }
                }
                if *fight_end {
                    break;
                }
            }
            if *fight_end {
                break;
            }
        }
        Ok(())
    }
    /// 检测战斗是否结束
    async fn check_fight_finish(&self, screen: &mut Option<ScreenFn>) -> bool {
        // seek/原始像素检测
        if self.param.finish.rotate_find_enemy_enabled {
            if let Some(end) = self.run_seek(screen, false).await {
                return end;
            }
        } else {
            tokio::time::sleep(Duration::from_millis(self.param.finish.end_delay_ms)).await;
        }
        log::info!("打开编队界面检查战斗是否结束");
        let _ = self
            .sim
            .simulate(GIAction::OpenPartySetupScreen, KeyType::KeyPress)
            .await;
        tokio::time::sleep(Duration::from_millis(self.param.finish.detect_delay_ms)).await;
        let Some(provider) = screen.as_mut() else {
            log::warn!("无截图源可用，跳过fight-end像素采样，按继续战斗处理");
            let _ = self.sim.key_press(GIAction::Drop);
            return false;
        };
        let Some(img) = provider() else {
            let _ = self.sim.key_press(GIAction::Drop);
            return false;
        };
        let (_w, h) = (img.cols(), img.rows());
        // 按当前截图分辨率缩放
        let scale = h as f64 / 1080.0;
        let py_bar = (50.0 * scale) as i32;
        let px_bar = (790.0 * scale) as i32;
        let py_white = py_bar;
        let px_white = (768.0 * scale) as i32;
        let bar = sample(&img, py_bar, px_bar);
        let white = sample(&img, py_white, px_white);
        let _ = self.sim.key_press(GIAction::Drop);
        let result = is_white(white) && is_yellow(bar);
        if result {
            log::info!("识别到战斗结束");
            // 取消正在进行的换队(多按一次)
            let _ = self.sim.key_press(GIAction::OpenPartySetupScreen);
        } else {
            log::info!(
                "未识别到战斗结束: yellow{},{},{};white{},{},{}",
                bar.0, bar.1, bar.2, white.0, white.1, white.2
            );
        }
        result
    }
    /// 旋转寻敌
    async fn run_seek(&self, screen: &mut Option<ScreenFn>, is_first_check: bool) -> Option<bool> {
        let result = match seek_and_fight(
            self.sim.clone(),
            screen,
            &mut *self.seek_state.lock().unwrap(),
            self.param.finish.detect_delay_ms,
            self.param.finish.end_delay_ms,
            is_first_check,
            self.param.rotary_factor,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                log::error!("seek_and_fight 异常: {e}");
                // 异常 -> 未结束
                return Some(false);
            }
        };
        match result {
            SeekResult::BattleEnded => Some(true),
            SeekResult::EnemyFound => Some(false),
            SeekResult::NoEnemy => None,
        }
    }
}

/// 采样BGR
fn sample(img: &Mat, y: i32, x: i32) -> (u8, u8, u8) {
    if y < 0 || x < 0 || y >= img.rows() || x >= img.cols() {
        return (0, 0, 0);
    }
    match img.at_2d::<Vec3b>(y, x) {
        Ok(p) => (p.0[0], p.0[1], p.0[2]),
        Err(_) => (0, 0, 0),
    }
}
fn is_yellow(bgr: (u8, u8, u8)) -> bool {
    let (b, g, r) = bgr;
    (200..=255).contains(&r) && (200..=255).contains(&g) && b <= 100
}
fn is_white(bgr: (u8, u8, u8)) -> bool {
    let (b, g, r) = bgr;
    (240..=255).contains(&r) && (240..=255).contains(&g) && (240..=255).contains(&b)
}

/// 留个测试点
pub fn default_script_dir() -> Option<&'static Path> {
    None
}
