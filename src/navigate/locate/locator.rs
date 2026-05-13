//! 单层小地图SIFT定位

use std::path::Path;

use opencv::boxed_ref::BoxedRef;
use opencv::calib3d::{RANSAC, find_homography};
use opencv::core::{
    DMatch, KeyPoint, KeyPointTraitConst, Mat, MatTraitConst, Point2f as CvP2f, Ptr, Vector,
    no_array, perspective_transform,
};
use opencv::features2d::{
    DescriptorMatcherTraitConst, Feature2DTrait, FlannBasedMatcher, SIFT,
};
use opencv::imgproc::{COLOR_BGR2GRAY, COLOR_BGRA2GRAY, cvt_color_def};

use crate::navigate::coord::{Point2f, image_to_game};
use crate::navigate::error::NavigateError;
use crate::navigate::map::cache::{LayerCache, SIFT_DESC_LEN};
use crate::navigate::map::splits::SplitGrid;
use crate::navigate::scene::SceneGeom;

/// 局部匹配时邻接块扩展数(9)
pub const LOCAL_MATCH_NEIGHBORS: i32 = 1;
/// good matches 最少数量
#[deprecated(note = "迁移常量")]
pub const SIFT_MIN_GOOD: usize = 7;
/// RANSAC重投影阈值
pub const SIFT_RANSAC_REPROJ: f64 = 3.0;
pub const SIFT_RATIO: f32 = 0.75;

/// 定位信息
#[derive(Debug, Clone)]
pub struct LocateInfo {
    pub scene: String,
    pub floor: i32,
    /// 大图坐标
    pub image_pos: Point2f,
    /// 游戏坐标
    pub game_pos: Point2f,
    /// 小地图检测到的keypoint数
    pub query_kps: usize,
    /// good match
    pub good_matches: usize,
    /// 训练集大小
    pub train_size: usize,
    /// 是否局部匹配
    pub local: bool,
}

/// 单floor定位器
/// 线程安全可能有问题
pub struct Locator {
    pub cache: LayerCache,
    pub geom: SceneGeom,
    /// N*128描述符
    desc_f32: Vec<f32>,
    /// keypoint像素坐标
    train_pts: Vec<CvP2f>,
    sift: Ptr<SIFT>,
    matcher: Ptr<FlannBasedMatcher>,
    /// 仅`split_row>0`且`split_col>0`时存在
    grid: Option<SplitGrid>,
    /// 上次成功定位的图像坐标
    last_image_pos: Option<CvP2f>,
}

impl Locator {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, NavigateError> {
        let cache = LayerCache::load(path)?;
        Self::from_cache(cache)
    }
    pub fn from_cache(cache: LayerCache) -> Result<Self, NavigateError> {
        let geom: SceneGeom = cache.geom.into();
        let desc_f32: Vec<f32> = cache.descriptors.iter().map(|&b| b as f32).collect();
        let train_pts: Vec<CvP2f> = cache
            .keypoints
            .iter()
            .map(|kp| CvP2f::new(kp.x, kp.y))
            .collect();
        let grid = if geom.split_row > 0 && geom.split_col > 0 {
            Some(SplitGrid::split(
                geom.map_w,
                geom.map_h,
                geom.split_row,
                geom.split_col,
                &cache.keypoints,
            ))
        } else {
            None
        };
        let sift = new_sift()?;
        let matcher = FlannBasedMatcher::create()
            .map_err(|e| NavigateError::Sift(format!("FlannBasedMatcher::create: {e}")))?;
        Ok(Self { cache, geom, desc_f32, train_pts, sift, matcher, grid, last_image_pos: None })
    }
    pub fn reset(&mut self) {
        self.last_image_pos = None;
    }
    pub fn last_image_pos(&self) -> Option<Point2f> {
        self.last_image_pos.map(|p| Point2f { x: p.x, y: p.y })
    }
    pub fn last_game_pos(&self) -> Option<Point2f> {
        self.last_image_pos
            .map(|p| image_to_game(&self.geom, Point2f { x: p.x, y: p.y }))
    }
    /// 注入位置hint
    pub fn set_hint_image_pos(&mut self, p: Point2f) {
        self.last_image_pos = Some(CvP2f::new(p.x, p.y));
    }
    /// 完整定位流程
    pub fn locate(&mut self, minimap: &Mat) -> Result<Option<Point2f>, NavigateError> {
        Ok(self.locate_with_info(minimap)?.map(|info| info.game_pos))
    }
    pub fn locate_with_info(
        &mut self,
        minimap: &Mat,
    ) -> Result<Option<LocateInfo>, NavigateError> {
        let gray = ensure_gray(minimap)?;
        let (q_kps, q_desc) = detect_sift(&mut self.sift, &gray)?;
        if q_kps.is_empty() || q_desc.rows() == 0 {
            return Ok(None);
        }
        self.match_query(&q_kps, &q_desc, gray.cols(), gray.rows())
    }
    /// 预检测query特征匹配
    pub fn match_query(
        &mut self,
        q_kps: &Vector<KeyPoint>,
        q_desc: &Mat,
        query_w: i32,
        query_h: i32,
    ) -> Result<Option<LocateInfo>, NavigateError> {
        self.match_query_with_min_good(q_kps, q_desc, query_w, query_h, SIFT_MIN_GOOD)
    }
    /// [`Self::match_query`]可调阈值版本
    /// query自身仍按[`SIFT_MIN_GOOD`]提前丢弃
    /// `min_good`仅用于ratio test后匹配数判定
    pub fn match_query_with_min_good(
        &mut self,
        q_kps: &Vector<KeyPoint>,
        q_desc: &Mat,
        query_w: i32,
        query_h: i32,
        min_good: usize,
    ) -> Result<Option<LocateInfo>, NavigateError> {
        let q_kps_len = q_kps.len();
        if q_kps_len < SIFT_MIN_GOOD || q_desc.rows() == 0 {
            return Ok(None);
        }
        // 选择局部/全局
        let local_indices: Option<Vec<u32>> = match (self.last_image_pos, &self.grid) {
            (Some(prev), Some(grid)) => {
                let (cr, cc) = grid.cell_of(prev.x, prev.y);
                Some(grid.merge_neighbors(cr, cc, LOCAL_MATCH_NEIGHBORS))
            }
            _ => None,
        };
        let local_buf: Vec<f32>;
        let local_pts: Vec<CvP2f>;
        let train_pts_ref: &[CvP2f];
        let train_mat: BoxedRef<'_, Mat> = match &local_indices {
            Some(idx) => {
                local_buf = collect_descriptors(&self.desc_f32, idx);
                local_pts = idx.iter().map(|&i| self.train_pts[i as usize]).collect();
                train_pts_ref = &local_pts;
                Mat::new_rows_cols_with_data::<f32>(
                    idx.len() as i32,
                    SIFT_DESC_LEN as i32,
                    &local_buf,
                )
                .map_err(|e| NavigateError::Sift(format!("Mat from local: {e}")))?
            }
            None => {
                train_pts_ref = &self.train_pts;
                Mat::new_rows_cols_with_data::<f32>(
                    self.train_pts.len() as i32,
                    SIFT_DESC_LEN as i32,
                    &self.desc_f32,
                )
                .map_err(|e| NavigateError::Sift(format!("Mat from full: {e}")))?
            }
        };
        let train_size = train_pts_ref.len();
        let is_local = local_indices.is_some();
        // FLANN-based KNN k=2
        let mut matches: Vector<Vector<DMatch>> = Vector::new();
        self.matcher
            .knn_train_match(q_desc, &train_mat, &mut matches, 2, &no_array(), false)
            .map_err(|e| NavigateError::Sift(format!("knn_train_match: {e}")))?;
        // ratio test
        let mut src_pts: Vector<CvP2f> = Vector::new();
        let mut dst_pts: Vector<CvP2f> = Vector::new();
        for pair in matches.iter() {
            if pair.len() < 2 {
                continue;
            }
            let m = pair.get(0).unwrap();
            let n = pair.get(1).unwrap();
            if m.distance < SIFT_RATIO * n.distance {
                let q = q_kps.get(m.query_idx as usize).unwrap();
                src_pts.push(q.pt());
                dst_pts.push(train_pts_ref[m.train_idx as usize]);
            }
        }
        let good = src_pts.len();
        if good < min_good {
            if is_local {
                self.last_image_pos = None;
            }
            return Ok(None);
        }
        // RANSAC
        let mut mask = Mat::default();
        let h = find_homography(&src_pts, &dst_pts, &mut mask, RANSAC, SIFT_RANSAC_REPROJ)
            .map_err(|e| NavigateError::Sift(format!("find_homography: {e}")))?;
        if h.empty() {
            return Ok(None);
        }
        // 小地图中心 -> 大图
        let cx = (query_w as f32) / 2.0;
        let cy = (query_h as f32) / 2.0;
        let src: Vector<CvP2f> = Vector::from_iter([CvP2f::new(cx, cy)]);
        let mut dst: Vector<CvP2f> = Vector::new();
        perspective_transform(&src, &mut dst, &h)
            .map_err(|e| NavigateError::Sift(format!("perspective_transform: {e}")))?;
        let p = dst.get(0).unwrap();
        let img_pos = Point2f { x: p.x, y: p.y };
        let game_pos = image_to_game(&self.geom, img_pos);
        self.last_image_pos = Some(p);
        Ok(Some(LocateInfo {
            scene: self.cache.scene.clone(),
            floor: self.cache.floor,
            image_pos: img_pos,
            game_pos,
            query_kps: q_kps_len,
            good_matches: good,
            train_size,
            local: is_local,
        }))
    }
}

fn collect_descriptors(full: &[f32], indices: &[u32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(indices.len() * SIFT_DESC_LEN);
    for &i in indices {
        let off = (i as usize) * SIFT_DESC_LEN;
        out.extend_from_slice(&full[off..off + SIFT_DESC_LEN]);
    }
    out
}

/// 图像转灰度
pub fn ensure_gray(src: &Mat) -> Result<Mat, NavigateError> {
    let ch = src.channels();
    if ch == 1 {
        return Ok(src.clone());
    }
    let code = match ch {
        3 => COLOR_BGR2GRAY,
        4 => COLOR_BGRA2GRAY,
        n => return Err(NavigateError::Sift(format!("unsupported channels: {n}"))),
    };
    let mut gray = Mat::default();
    cvt_color_def(src, &mut gray, code)
        .map_err(|e| NavigateError::Sift(format!("cvt_color: {e}")))?;
    Ok(gray)
}
/// 用特定SIFT实例检测
/// 用来共享检测器
pub fn detect_sift(
    sift: &mut Ptr<SIFT>,
    gray: &Mat,
) -> Result<(Vector<KeyPoint>, Mat), NavigateError> {
    let mut kps: Vector<KeyPoint> = Vector::new();
    let mut desc = Mat::default();
    sift.detect_and_compute(gray, &no_array(), &mut kps, &mut desc, false)
        .map_err(|e| NavigateError::Sift(format!("detect_and_compute: {e}")))?;
    Ok((kps, desc))
}

pub fn new_sift() -> Result<Ptr<SIFT>, NavigateError> {
    SIFT::create_def().map_err(|e| NavigateError::Sift(format!("SIFT::create: {e}")))
}
