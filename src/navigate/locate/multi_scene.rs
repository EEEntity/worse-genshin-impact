//! 多场景小地图定位
//! 
//! 依次尝试所有已加载的场景/floor，返回good matches最多的
//! 
//! - 共享一个SIFT检测器
//! - 每个`Locator`对应一层
//! - 优先复用上次匹配的场景/floor

use std::path::PathBuf;
use opencv::core::{Mat, MatTraitConst, Ptr};
use opencv::features2d::SIFT;

use crate::navigate::coord::{Point2f, game_to_image};
use crate::navigate::error::NavigateError;
use super::locator::{LocateInfo, Locator, detect_sift, ensure_gray, new_sift};
use crate::navigate::map::cache::LayerCache;
use crate::navigate::scene::ALL_SCENES;

#[deprecated(note = "迁移常量")]
pub const SIFT_MIN_GOOD: usize = 7;
/// 新坐标相对上次跳跃距离阈值
pub const STABLE_JUMP_THRESHOLD: f32 = 150.0;
/// 稳定定位阈值
pub const STABLE_LOCAL_MIN_GOOD: usize = 15;
/// navigate 自有 SIFT 缓存目录（项目相对路径）
#[deprecated(note = "迁移常量")]
pub const MAP_ASSETS_DIR: &str = "navigate/assets/map";

pub struct MultiSceneLocator {
    pub locators: Vec<Locator>,
    sift: Ptr<SIFT>,
    /// 最近成功匹配的locators索引
    last_hit: Option<usize>,
}

impl MultiSceneLocator {
    pub fn load_default() -> Result<Self, NavigateError> {
        Self::load_from(MAP_ASSETS_DIR.as_ref())
    }
    pub fn load_from(root: &std::path::Path) -> Result<Self, NavigateError> {
        let mut locators = Vec::new();
        for scene in ALL_SCENES {
            let dir = root.join(scene.name);
            for floor in scene.floors {
                let path: PathBuf = dir.join(format!("{}_{}.sift.bin", scene.name, floor.floor));
                if !path.exists() {
                    log::warn!("缓存缺失，跳过: {}", path.display());
                    continue;
                }
                let cache = LayerCache::load(&path)?;
                // sanity check
                if cache.scene != scene.name || cache.floor != floor.floor {
                    return Err(NavigateError::Cache(format!(
                        "{}: scene/floor 不一致 (got {}/{}, expected {}/{})",
                        path.display(),
                        cache.scene,
                        cache.floor,
                        scene.name,
                        floor.floor
                    )));
                }
                let _ = floor.source; // 标注一下匹配来源已记录在 cache 中
                locators.push(Locator::from_cache(cache)?);
            }
        }
        if locators.is_empty() {
            return Err(NavigateError::Cache(format!(
                "未在 {} 找到任何 .sift.bin 缓存。请先运行 `cargo run --bin build_sift_cache`",
                root.display()
            )));
        }
        let sift = new_sift()?;
        Ok(Self { locators, sift, last_hit: None })
    }
    /// 多场景定位
    pub fn locate(&mut self, minimap: &Mat) -> Result<Option<LocateInfo>, NavigateError> {
        let gray = ensure_gray(minimap)?;
        let (q_kps, q_desc) = detect_sift(&mut self.sift, &gray)?;
        if q_kps.len() < SIFT_MIN_GOOD || q_desc.rows() == 0 {
            return Ok(None);
        }
        let w = gray.cols();
        let h = gray.rows();
        // 优先尝试上次的
        let order: Vec<usize> = match self.last_hit {
            Some(i) => std::iter::once(i)
                .chain((0..self.locators.len()).filter(|&j| j != i))
                .collect(),
            None => (0..self.locators.len()).collect(),
        };
        let mut best: Option<(usize, LocateInfo)> = None;
        for i in order {
            // hot-hit
            let info_opt = self.locators[i].match_query(&q_kps, &q_desc, w, h)?;
            if let Some(info) = info_opt {
                let is_hot = self.last_hit == Some(i);
                if is_hot && info.local {
                    self.last_hit = Some(i);
                    return Ok(Some(info));
                }
                let better = match &best {
                    None => true,
                    Some((_, b)) => info.good_matches > b.good_matches,
                };
                if better {
                    best = Some((i, info));
                }
            }
        }
        if let Some((i, info)) = &best {
            self.last_hit = Some(*i);
            Ok(Some(info.clone()))
        } else {
            self.last_hit = None;
            Ok(None)
        }
    }
    /// 保留上次已知位置
    pub fn set_prev_position_game(&mut self, scene_name: &str, game: Point2f) {
        for loc in &mut self.locators {
            if loc.cache.scene.eq_ignore_ascii_case(scene_name) {
                let img = game_to_image(&loc.geom, game);
                loc.set_hint_image_pos(img);
            }
        }
        if let Some(idx) = self
            .locators
            .iter()
            .position(|l| l.cache.scene.eq_ignore_ascii_case(scene_name))
        {
            self.last_hit = Some(idx);
        }
    }
    /// 清空hint
    pub fn reset_hints(&mut self) {
        for loc in &mut self.locators {
            loc.reset();
        }
    }
    /// 备用定位
    pub fn locate_fallback(&mut self, minimap: &Mat) -> Result<Option<LocateInfo>, NavigateError> {
        let gray = ensure_gray(minimap)?;
        let (q_kps, q_desc) = detect_sift(&mut self.sift, &gray)?;
        if q_kps.len() < SIFT_MIN_GOOD || q_desc.rows() == 0 {
            return Ok(None);
        }
        let w = gray.cols();
        let h = gray.rows();
        // last_hit优先
        let order: Vec<usize> = match self.last_hit {
            Some(i) => std::iter::once(i)
                .chain((0..self.locators.len()).filter(|&j| j != i))
                .collect(),
            None => (0..self.locators.len()).collect(),
        };
        let mut local_hit: Option<(usize, LocateInfo, Point2f)> = None;
        for i in order {
            // 无hint就跳过局部匹配
            let prev = match self.locators[i].last_image_pos() {
                Some(p) => p,
                None => continue,
            };
            // 严格阈值的局部匹配
            let info_opt = self.locators[i].match_query_with_min_good(
                &q_kps,
                &q_desc,
                w,
                h,
                STABLE_LOCAL_MIN_GOOD,
            )?;
            if let Some(info) = info_opt {
                if info.local {
                    local_hit = Some((i, info, prev));
                    break;
                }
                // 局部失败
            }
        }
        if let Some((i, info, prev)) = local_hit {
            // 换成全图
            let dx = info.image_pos.x - prev.x;
            let dy = info.image_pos.y - prev.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= STABLE_JUMP_THRESHOLD {
                self.last_hit = Some(i);
                return Ok(Some(info));
            }
            log::warn!(
                "locate_stable: 局部匹配位置跳跃 {dist:.1} > {STABLE_JUMP_THRESHOLD}, 改用全图匹配"
            );
        }
        // 清掉hints，在全图匹配
        self.reset_hints();
        let mut best: Option<(usize, LocateInfo)> = None;
        for i in 0..self.locators.len() {
            let info_opt = self.locators[i].match_query(&q_kps, &q_desc, w, h)?;
            if let Some(info) = info_opt {
                let better = match &best {
                    None => true,
                    Some((_, b)) => info.good_matches > b.good_matches,
                };
                if better {
                    best = Some((i, info));
                }
            }
        }
        if let Some((i, info)) = best {
            self.last_hit = Some(i);
            Ok(Some(info))
        } else {
            self.last_hit = None;
            Ok(None)
        }
    }
}