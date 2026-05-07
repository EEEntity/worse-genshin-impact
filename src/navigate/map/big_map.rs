//! 大地图SIFT匹配
//! 
//! 1. 加载.kp.bin/.mat.png
//! 2. 缩放大地图截图
//! 3. SIFT检测转换
//! 4. FLANN
//! 5. 转换256-scale矩形

use std::path::{Path, PathBuf};

use opencv::calib3d::{RANSAC, find_homography};
use opencv::core::{
    DMatch, KeyPointTraitConst, Mat, MatTraitConst, MatTraitConstManual,
    Point2f as CvP2f, Ptr, Rect, Vector, no_array, perspective_transform,
};
use opencv::features2d::{DescriptorMatcherTraitConst, FlannBasedMatcher, SIFT};
use opencv::imgcodecs::{IMREAD_GRAYSCALE, imread};
use opencv::imgproc::{INTER_AREA, resize};

use crate::navigate::error::NavigateError;
use crate::navigate::locate::{detect_sift, ensure_gray, new_sift};
use crate::navigate::map::cache::{SIFT_DESC_LEN, read_bgi_keypoints};

/// good matches最少数量
pub const SIFT_MIN_GOOD: usize = 7;
/// RANSAC重投影阈值
pub const SIFT_RANSAC_REPROJ: f64 = 3.0;
/// SIFT ratio阈值
pub const SIFT_RATIO: f32 = 0.75;
/// 来自BGI的地图素材
pub const ORIGIN_BGI_MAP_DIR: &str = "assets/Map";

/// 大地图SIFT匹配器(场景/floor)
pub struct BigMapMatcher {
    pub scene: String,
    /// keypoint像素坐标
    train_pts: Vec<CvP2f>,
    /// 描述符(行主序f32，N*128）
    desc_f32: Vec<f32>,
    sift: Ptr<SIFT>,
    matcher: Ptr<FlannBasedMatcher>,
}

impl BigMapMatcher {
    /// 加载Teyvat地图资源
    pub fn load_teyvat() -> Result<Self, NavigateError> {
        Self::load_teyvat_from(ORIGIN_BGI_MAP_DIR)
    }
    pub fn load_teyvat_from(bgi_map_dir: impl AsRef<Path>) -> Result<Self, NavigateError> {
        let dir: PathBuf = bgi_map_dir.as_ref().join("Teyvat");
        let kp = dir.join("Teyvat_0_256_SIFT.kp.bin");
        let mat = dir.join("Teyvat_0_256_SIFT.mat.png");
        Self::load("Teyvat", &kp, &mat)
    }
    /// 通用加载
    pub fn load(
        scene: &str,
        kp_path: &Path,
        mat_path: &Path,
    ) -> Result<Self, NavigateError> {
        if !kp_path.exists() {
            return Err(NavigateError::Other(format!(
                "BigMapMatcher: keypoint 文件不存在: {}",
                kp_path.display()
            )));
        }
        if !mat_path.exists() {
            return Err(NavigateError::Other(format!(
                "BigMapMatcher: descriptor 文件不存在: {}",
                mat_path.display()
            )));
        }
        let kps = read_bgi_keypoints(kp_path)?;
        let train_pts: Vec<CvP2f> = kps.iter().map(|k| CvP2f::new(k.x, k.y)).collect();
        // .mat.png
        let mp = mat_path.to_str().ok_or_else(|| {
            NavigateError::Other(format!("BigMapMatcher: 非 UTF-8 路径 {}", mat_path.display()))
        })?;
        let img = imread(mp, IMREAD_GRAYSCALE)
            .map_err(|e| NavigateError::Sift(format!("imread {mp}: {e}")))?;
        if img.empty() {
            return Err(NavigateError::Sift(format!("BigMapMatcher: PNG 解码空: {mp}")));
        }
        let rows = img.rows() as usize;
        let cols = img.cols() as usize;
        if rows != train_pts.len() {
            return Err(NavigateError::Sift(format!(
                "BigMapMatcher: descriptor rows {rows} ≠ keypoints {}",
                train_pts.len()
            )));
        }
        if cols != SIFT_DESC_LEN {
            return Err(NavigateError::Sift(format!(
                "BigMapMatcher: descriptor cols {cols} ≠ {SIFT_DESC_LEN}"
            )));
        }
        let mut desc_f32 = Vec::with_capacity(rows * cols);
        // 行主序cpy
        for r in 0..rows as i32 {
            let row = img
                .row(r)
                .map_err(|e| NavigateError::Sift(format!("row {r}: {e}")))?;
            let bytes = row
                .data_bytes()
                .map_err(|e| NavigateError::Sift(format!("row {r} data_bytes: {e}")))?;
            if bytes.len() != cols {
                return Err(NavigateError::Sift(format!(
                    "BigMapMatcher: row {r} bytes {} ≠ cols {cols}",
                    bytes.len()
                )));
            }
            for &b in bytes {
                desc_f32.push(b as f32);
            }
        }
        let sift = new_sift()?;
        let matcher = FlannBasedMatcher::create()
            .map_err(|e| NavigateError::Sift(format!("FlannBasedMatcher::create: {e}")))?;
        log::info!(
            "BigMapMatcher 加载完成 scene={scene} keypoints={} descriptors={}",
            train_pts.len(),
            desc_f32.len() / SIFT_DESC_LEN,
        );
        Ok(Self {
            scene: scene.to_string(),
            train_pts,
            desc_f32,
            sift,
            matcher,
        })
    }
    /// 在256-scale数据上匹配查询截图
    pub fn match_rect_256(
        &mut self,
        query: &Mat,
    ) -> Result<Option<Rect>, NavigateError> {
        // 灰度
        let gray = ensure_gray(query)?;
        // resize 1/4
        let mut small = Mat::default();
        resize(
            &gray,
            &mut small,
            opencv::core::Size::new(0, 0),
            0.25,
            0.25,
            INTER_AREA,
        )
        .map_err(|e| NavigateError::Sift(format!("resize 1/4: {e}")))?;
        if small.empty() || small.cols() < 16 || small.rows() < 16 {
            return Ok(None);
        }
        // SIFT detect + compute
        let (q_kps, q_desc) = detect_sift(&mut self.sift, &small)?;
        if q_kps.is_empty() || q_desc.rows() == 0 {
            return Ok(None);
        }
        // train矩阵
        let train_mat = Mat::new_rows_cols_with_data::<f32>(
            self.train_pts.len() as i32,
            SIFT_DESC_LEN as i32,
            &self.desc_f32,
        )
        .map_err(|e| NavigateError::Sift(format!("train Mat: {e}")))?;

        // KNN k=2
        let mut matches: Vector<Vector<DMatch>> = Vector::new();
        self.matcher
            .knn_train_match(&q_desc, &train_mat, &mut matches, 2, &no_array(), false)
            .map_err(|e| NavigateError::Sift(format!("knn_train_match: {e}")))?;
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
                dst_pts.push(self.train_pts[m.train_idx as usize]);
            }
        }
        if src_pts.len() < SIFT_MIN_GOOD {
            log::debug!(
                "BigMapMatcher: good matches {} < {}",
                src_pts.len(),
                SIFT_MIN_GOOD
            );
            return Ok(None);
        }
        // RANSAC
        let mut mask = Mat::default();
        let h = find_homography(
            &src_pts,
            &dst_pts,
            &mut mask,
            RANSAC,
            SIFT_RANSAC_REPROJ,
        )
        .map_err(|e| NavigateError::Sift(format!("find_homography: {e}")))?;
        if h.empty() {
            return Ok(None);
        }
        let qw = small.cols() as f32;
        let qh = small.rows() as f32;
        let corners: Vector<CvP2f> = Vector::from_iter([
            CvP2f::new(0.0, 0.0),
            CvP2f::new(0.0, qh),
            CvP2f::new(qw, qh),
            CvP2f::new(qw, 0.0),
        ]);
        let mut mapped: Vector<CvP2f> = Vector::new();
        perspective_transform(&corners, &mut mapped, &h)
            .map_err(|e| NavigateError::Sift(format!("perspective_transform: {e}")))?;
        // boudingRect
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for i in 0..mapped.len() {
            let p = mapped.get(i).unwrap();
            if p.x < min_x { min_x = p.x; }
            if p.x > max_x { max_x = p.x; }
            if p.y < min_y { min_y = p.y; }
            if p.y > max_y { max_y = p.y; }
        }
        let rect = Rect::new(
            min_x.floor() as i32,
            min_y.floor() as i32,
            (max_x - min_x).ceil() as i32,
            (max_y - min_y).ceil() as i32,
        );
        if rect.width <= 0 || rect.height <= 0 {
            return Ok(None);
        }
        Ok(Some(rect))
    }
}