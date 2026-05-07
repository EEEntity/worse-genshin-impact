//! 路径任务与点位数据结构

use serde::{Deserialize, Serialize};

/// 路径文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathingTask {
    #[serde(default)]
    pub info: PathInfo,
    #[serde(default)]
    pub positions: Vec<Waypoint>,
}

/// 路径头部信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PathInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub task_type: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// 默认地图名`Teyvat`
    #[serde(default = "default_map_name")]
    pub map_name: String,
    /// 默认匹配方式
    #[serde(default)]
    pub map_match_method: String,
    #[serde(default)]
    pub bgi_version: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub last_modified_time: i64,
}

fn default_map_name() -> String { "Teyvat".to_string() }

/// 路径点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waypoint {
    #[serde(default)]
    pub id: i64,
    pub x: f64,
    pub y: f64,
    /// 路径点类型`WaypointType`
    #[serde(default = "default_type")]
    #[serde(rename = "type")]
    pub waypoint_type: String,
    /// 移动方式`MoveMode`
    #[serde(default = "default_move_mode")]
    pub move_mode: String,
    /// action code，可选
    #[serde(default)]
    pub action: Option<String>,
    /// action 参数，可选
    #[serde(default)]
    pub action_params: Option<String>,
    /// 扩展参数（异常处理、描述、怪物标签等）
    #[serde(default)]
    pub point_ext_params: ExtParams,
}

fn default_type() -> String { "path".to_string() }
fn default_move_mode() -> String { "walk".to_string() }

/// 扩展参数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtParams {
    #[serde(default)]
    pub misidentification: Option<Misidentification>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub monster_tags: Option<String>,
    #[serde(default)]
    pub enable_monser_loot_split: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Misidentification {
    /// 处理`unrecognized`/`pathTooFar`
    #[serde(default = "default_mis_type")]
    #[serde(rename = "type")]
    pub kinds: Vec<String>,
    #[serde(default = "default_handling_mode")]
    pub handling_mode: String,
    #[serde(default)]
    pub arrival_time: i32,
}

impl Default for Misidentification {
    fn default() -> Self {
        Self {
            kinds: default_mis_type(),
            handling_mode: default_handling_mode(),
            arrival_time: 0,
        }
    }
}

fn default_mis_type() -> Vec<String> { vec!["unrecognized".to_string()] }
fn default_handling_mode() -> String { "previousDetectedPoint".to_string() }
