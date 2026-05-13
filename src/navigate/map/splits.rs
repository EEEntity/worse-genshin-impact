//! 特征点按网格切块
//! 
//! 给定地图大小`(w,h)`，行列数`(rows,cols)`，全部特征点
//! 返回`rows*cols`个块，每块包含特征点在原数组中的索引

use crate::navigate::map::cache::KeyPointRaw;

/// 单个特征块
#[derive(Debug, Clone, Default)]
pub struct FeatureBlock {
    pub indices: Vec<u32>,
}

/// 行主序切分结果
pub struct SplitGrid {
    pub rows: i32,
    pub cols: i32,
    pub map_w: i32,
    pub map_h: i32,
    /// 行主序的块数组
    pub blocks: Vec<FeatureBlock>,
}

impl SplitGrid {
    /// 切分
    pub fn split(map_w: i32, map_h: i32, rows: i32, cols: i32, kps: &[KeyPointRaw]) -> Self {
        debug_assert!(rows > 0 && cols > 0);
        let cell_w = map_w / cols;
        let cell_h = map_h / rows;
        let mut blocks = vec![FeatureBlock::default(); (rows * cols) as usize];
        for (i, kp) in kps.iter().enumerate() {
            let mut row = (kp.y as i32) / cell_h;
            let mut col = (kp.x as i32) / cell_w;
            row = row.clamp(0, rows - 1);
            col = col.clamp(0, cols - 1);
            blocks[(row * cols + col) as usize].indices.push(i as u32);
        }
        Self { rows, cols, map_w, map_h, blocks }
    }
    /// 获取块索引
    pub fn at(&self, row: i32, col: i32) -> Option<&FeatureBlock> {
        if row < 0 || col < 0 || row >= self.rows || col >= self.cols {
            return None;
        }
        Some(&self.blocks[(row * self.cols + col) as usize])
    }
    pub fn cell_of(&self, x: f32, y: f32) -> (i32, i32) {
        let cell_w = self.map_w as f32 / self.cols as f32;
        let cell_h = self.map_h as f32 / self.rows as f32;
        let row = (y / cell_h).round() as i32;
        let col = (x / cell_w).round() as i32;
        (row.clamp(0, self.rows - 1), col.clamp(0, self.cols - 1))
    }
    /// 合并
    pub fn merge_neighbors(&self, cell_row: i32, cell_col: i32, expand: i32) -> Vec<u32> {
        let r0 = (cell_row - expand).max(0);
        let r1 = (cell_row + expand).min(self.rows - 1);
        let c0 = (cell_col - expand).max(0);
        let c1 = (cell_col + expand).min(self.cols - 1);
        let mut out = Vec::new();
        for r in r0..=r1 {
            for c in c0..=c1 {
                out.extend_from_slice(&self.blocks[(r * self.cols + c) as usize].indices);
            }
        }
        out
    }
}
