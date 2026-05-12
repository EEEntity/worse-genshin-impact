//! 根据当前/目标角度产生鼠标横向位移下发至[`GIDevice`]
//! 
//! 控制策略:
//! - 单步`dx=round(diff_deg * deg_to_dx)`，diff[-180,180]应为最短弧
//! - 死区`dead_zone_deg`返回false
//! - 限制最大鼠标位移`max_dx_per_step`，~~可能甩出去~~

use crate::device::{DeviceError, GIDevice};

const DEFAULT_DEG_TO_DX: f32 = 1920.0 / 360.0;

pub struct RotateController {
    /// 旋转1度对应的鼠标REL_X增量
    pub deg_to_dx: f32,
    /// 死区
    pub dead_zone_deg: f32,
    /// 单帧最大|dx|
    pub max_dx_per_step: i32,
}

impl Default for RotateController {
    fn default() -> Self {
        Self {
            deg_to_dx: DEFAULT_DEG_TO_DX,
            dead_zone_deg: 1.5,
            max_dx_per_step: 800, // 好像有点大...
        }
    }
}

impl RotateController {
    pub fn new(deg_to_dx: f32) -> Self {
        Self { deg_to_dx, ..Self::default() }
    }
    pub fn set_deg_to_dx(&mut self, k: f32) {
        self.deg_to_dx = k;
    }
    /// 折叠到(-180,180]
    pub fn shortest_diff(target: f32, current: f32) -> f32 {
        let mut d = (target - current) % 360.0;
        if d <= -180.0 {
            d += 360.0;
        } else if d > 180.0 {
            d -= 360.0;
        }
        d
    }
    /// 计算单帧dx
    pub fn compute_step(&self, current_deg: f32, target_deg: f32) ->(i32, bool) {
        let diff = Self::shortest_diff(target_deg, current_deg);
        if diff.abs() < self.dead_zone_deg {
            return (0, true);
        }
        let raw = (diff * self.deg_to_dx).round() as i32;
        let dx = raw.clamp(-self.max_dx_per_step, self.max_dx_per_step);
        (dx, false)
    }
    /// 单帧下发
    pub fn step(
        &self,
        device: &GIDevice,
        current_deg: f32,
        target_deg: f32,
    ) -> Result<bool, DeviceError> {
        let (dx, aligned) = self.compute_step(current_deg, target_deg);
        if dx != 0 {
            device.mouse_move_rel(dx, 0)?;
        }
        Ok(aligned)
    }
}
