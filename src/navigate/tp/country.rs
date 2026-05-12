//! 提瓦特国家中心点坐标/距离计算

/// 国家中心点坐标
pub const COUNTRY_POSITIONS: &[(&str, [f64; 2])] = &[
    ("蒙德", [-876.0, 2278.0]),
    ("璃月", [270.0, -666.0]),
    ("稻妻", [-4400.0, -3050.0]),
    ("须弥", [2877.0, -374.0]),
    ("枫丹", [4515.0, 3631.0]),
    ("纳塔", [8973.5, -1879.1]),
    ("挪德卡莱", [9542.25, 1661.84]),
];

/// 找到距`(x,y)`最近的国家，如果当前已知`current`距离更近就返回None
pub fn nearest_country(x: f64, y: f64, current_distance: f64) -> Option<&'static str> {
    let mut best: Option<&'static str> = None;
    let mut best_d = current_distance;
    for (name, pos) in COUNTRY_POSITIONS {
        let dx = pos[0] - x;
        let dy = pos[1] - y;
        let d = (dx * dx + dy * dy).sqrt();
        if d < best_d {
            best_d = d;
            best = Some(name);
        }
    }
    best
}
