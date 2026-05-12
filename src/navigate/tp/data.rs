//! tp.json数据模型/最近点查询

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::navigate::error::NavigateError;

#[deprecated(note = "迁移常量")]
const TP_JSON_PATH: &str = "assets/configs/tp.json"; 

/// 单个传送点
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TpPosition {
    pub id: i64,
    #[serde(default)]
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub areas: Vec<String>,
    /// 世界坐标`[a, b, c]`: a=纵向(Y), b=高度(无关), c=横向(X)
    #[serde(default)]
    pub position: [f64; 3],
    /// 实际传送落点(`position`是图标显示位置，传送会落到 `tranPosition`)
    #[serde(default)]
    pub tran_position: [f64; 3],
}

impl TpPosition {
    /// 游戏坐标X
    pub fn x(&self) -> f64 { self.position[2] }
    /// 游戏坐标Y
    pub fn y(&self) -> f64 { self.position[0] }
    /// 实际落点X
    pub fn tran_x(&self) -> f64 { self.tran_position[2] }
    /// 实际落点Y
    pub fn tran_y(&self) -> f64 { self.tran_position[0] }
    /// 与`(x,y)`的欧氏距离
    pub fn distance_to(&self, x: f64, y: f64) -> f64 {
        let dx = self.x() - x;
        let dy = self.y() - y;
        (dx * dx + dy * dy).sqrt()
    }
    pub fn is_goddess(&self) -> bool { self.kind == "Goddess" }
    pub fn is_domain(&self) -> bool {
        matches!(self.kind.as_str(), "BlessDomain" | "ForgeryDomain" | "MasteryDomain")
    }
}

/// 单场景全部传送点
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldScene {
    #[serde(default)]
    pub scene_id: i64,
    pub map_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub points: Vec<TpPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpJsonRoot {
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub data: Vec<WorldScene>,
}

/// 已解析的TP数据
pub struct TpDatabase {
    scenes: std::collections::HashMap<String, WorldScene>,
}

impl TpDatabase {
    /// 从tp.json文件加载数据
    pub fn load(path: impl AsRef<Path>) -> Result<Self, NavigateError> {
        let bytes = std::fs::read(path.as_ref())?;
        Self::from_slice(&bytes)
    }
    pub fn from_slice(bytes: &[u8]) -> Result<Self, NavigateError> {
        let root: TpJsonRoot = serde_json::from_slice(bytes)?;
        let scenes = root
            .data
            .into_iter()
            .map(|s|(s.map_name.clone(), s))
            .collect();
        Ok(Self { scenes })
    }
    pub fn load_default() -> Result<Self, NavigateError> {
        Self::load(TP_JSON_PATH)
    }
    /// 地图名 -> 场景数据
    pub fn scene(&self, map_name: &str) -> Option<&WorldScene> {
        self.scenes.get(map_name)
    }
    /// 场景下所有点
    pub fn points(&self, map_name: &str) -> &[TpPosition] {
        self.scenes
            .get(map_name)
            .map(|s|s.points.as_slice())
            .unwrap_or(&[])
    }
    /// 找到`(x,y)`在指定场景下最近的N个传送点
    /// 返回按距离升序
    pub fn nearest_n(&self, x: f64, y:f64, map_name: &str, n: usize) -> Vec<&TpPosition> {
        let points = self.points(map_name);
        let mut indexed: Vec<(f64, &TpPosition)> = points
            .iter()
            .map(|p|(p.distance_to(x, y), p))
            .collect();
        indexed.sort_by(|a, b|a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        indexed.into_iter().take(n).map(|(_, p)|p).collect()
    }
    /// 距离`(x,y)`最近的七天神像(提瓦特)
    pub fn nearest_goddess(&self, x: f64, y: f64) -> Option<&TpPosition> {
        self.points("Teyvat")
            .iter()
            .filter(|p|p.is_goddess())
            .min_by(|a,b|{a.distance_to(x, y)
                            .partial_cmp(&b.distance_to(x, y))
                            .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}
