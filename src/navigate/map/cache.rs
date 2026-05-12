//! 导航模块SIFT特征缓存
//! 
//! - KeyPoint直接用`Vec<KeyPointRaw>`
//! - 描述符保留BGI格式(u8[128])，运行时转f32

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use bincode::config::{Configuration, standard};
use bincode::serde::{decode_from_std_read, encode_into_std_write};
use serde::{Deserialize, Serialize};

use crate::navigate::error::NavigateError;
use crate::navigate::scene::SceneGeom;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(C)]
pub struct KeyPointRaw {
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub angle: f32,
    pub response: f32,
    pub octave: i32,
    pub class_id: i32,
}

const _: () = assert!(std::mem::size_of::<KeyPointRaw>() == 28);

/// 描述符向量长度
pub const SIFT_DESC_LEN: usize = 128;

/// 几何描述，与[`SceneGeom`]对应
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CachedGeom {
    pub map_w: i32,
    pub map_h: i32,
    pub block_width: i32,
    pub origin_x: f32,
    pub origin_y: f32,
    pub split_row: i32,
    pub split_col: i32,
}

impl From<SceneGeom> for CachedGeom {
    fn from(g: SceneGeom) -> Self {
        Self {
            map_w: g.map_w,
            map_h: g.map_h,
            block_width: g.block_width,
            origin_x: g.origin_x,
            origin_y: g.origin_y,
            split_row: g.split_row,
            split_col: g.split_col,
        }
    }
}

impl From<CachedGeom> for SceneGeom {
    fn from(g: CachedGeom) -> Self {
        Self {
            map_w: g.map_w,
            map_h: g.map_h,
            block_width: g.block_width,
            origin_x: g.origin_x,
            origin_y: g.origin_y,
            split_row: g.split_row,
            split_col: g.split_col,
        }
    }
}

/// 单层(floor)的特征缓存
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerCache {
    /// 缓存格式版本
    pub version: u32,
    /// 场景名`Teyvat`
    pub scene: String,
    /// `floor`编号(0主层)
    pub floor: i32,
    /// 几何信息
    pub geom: CachedGeom,
    /// 描述符列数
    pub desc_cols: u32,
    /// `KeyPoint`数组
    pub keypoints: Vec<KeyPointRaw>,
    /// 描述符raw u8
    pub descriptors: Vec<u8>,
}

impl LayerCache {
    pub const CURRENT_VERSION: u32 = 2;

    pub fn rows(&self) -> usize { self.keypoints.len() }

    /// 序列化保存
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), NavigateError> {
        let path = path.as_ref();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut w = BufWriter::new(File::create(path)?);
        let cfg: Configuration = standard();
        encode_into_std_write(self, &mut w, cfg)
            .map_err(|e| NavigateError::Cache(format!("encode {}: {e}", path.display())))?;
        Ok(())
    }
    /// 反序列化加载
    pub fn load(path: impl AsRef<Path>) -> Result<Self, NavigateError> {
        let path = path.as_ref();
        let mut r = BufReader::new(File::open(path)?);
        let cfg: Configuration = standard();
        let cache: Self = decode_from_std_read(&mut r, cfg)
            .map_err(|e| NavigateError::Cache(format!("decode {}: {e}", path.display())))?;
        if cache.version != Self::CURRENT_VERSION {
            return Err(NavigateError::Cache(format!(
                "{}: version mismatch (got {}, expected {})",
                path.display(),
                cache.version,
                Self::CURRENT_VERSION
            )));
        }
        if cache.descriptors.len() != cache.keypoints.len() * cache.desc_cols as usize {
            return Err(NavigateError::Cache(format!(
                "{}: descriptors length {} != {} * {}",
                path.display(),
                cache.descriptors.len(),
                cache.keypoints.len(),
                cache.desc_cols,
            )));
        }
        Ok(cache)
    }
}

// BGI缓存转换
/// 读取
pub fn read_bgi_keypoints(path: impl AsRef<Path>) -> Result<Vec<KeyPointRaw>, NavigateError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)?;
    let kp_size = std::mem::size_of::<KeyPointRaw>();
    if !bytes.len().is_multiple_of(kp_size) {
        return Err(NavigateError::Cache(format!(
            "{}: file size {} not multiple of KeyPoint size {kp_size}",
            path.display(),
            bytes.len()
        )));
    }
    let n = bytes.len() / kp_size;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let start = i * kp_size;
        let end = start + kp_size;
        let mut kp = KeyPointRaw {
            x: 0.0, y: 0.0, size: 0.0, angle: 0.0, response: 0.0,
            octave: 0, class_id: 0,
        };
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr().add(start),
                &mut kp as *mut KeyPointRaw as *mut u8,
                kp_size,
            );
            let _ = end;
        }
        out.push(kp);
    }
    Ok(out)
}
