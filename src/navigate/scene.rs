//! 地图场景配置

/// 地图参数
#[derive(Debug, Clone, Copy)]
pub struct SceneGeom {
    pub map_w: i32,
    pub map_h: i32,
    pub block_width: i32,
    pub origin_x: f32,
    pub origin_y: f32,
    pub split_row: i32,
    pub split_col: i32,
}

impl SceneGeom {
    pub const fn new(
        rows: i32,
        cols: i32,
        up_rows: i32,
        left_cols: i32,
        block_width: i32,
        split_row: i32,
        split_col: i32,
    ) -> Self {
        Self {
            map_w: cols * block_width,
            map_h: rows * block_width,
            block_width,
            origin_x: ((left_cols + 1) * block_width) as f32,
            origin_y: ((up_rows + 1) * block_width) as f32,
            split_row,
            split_col,
        }
    }

    pub fn block_scale_to_1024(&self) -> f32 {
        self.block_width as f32 / 1024.0
    }
}

/// 特征源
#[derive(Debug, Clone, Copy)]
pub enum SiftSource {
    BgiPrebuilt {
        kp: &'static str,
        mat: &'static str,
    },
    RawImage {
        path: &'static str,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct Floor {
    pub floor: i32,
    pub source: SiftSource,
}

#[derive(Debug, Clone, Copy)]
pub struct Scene {
    pub name: &'static str,
    pub desc: &'static str,
    pub geom: SceneGeom,
    pub floors: &'static [Floor],
}

/// 提瓦特
const TEYVAT_FLOORS: &[Floor] = &[Floor {
    floor: 0,
    source: SiftSource::BgiPrebuilt {
        kp: "Teyvat_0_2048_SIFT.kp.bin",
        mat: "Teyvat_0_2048_SIFT.mat.png",
    },
}];

/// 层岩巨渊
const THECHASM_FLOORS: &[Floor] = &[Floor {
    floor: 0,
    source: SiftSource::RawImage { path: "TheChasm_0_1024.png" },
}];

/// 渊下宫
const ENKANOMIYA_FLOORS: &[Floor] = &[Floor {
    floor: 0,
    source: SiftSource::RawImage { path: "Enkanomiya_0_1024.png" },
}];

/// 旧日之海
const SEAOFBYGONEERAS_FLOORS: &[Floor] = &[
    Floor { floor: 0, source: SiftSource::RawImage { path: "SeaOfBygoneEras_0_1024.png" } },
    Floor { floor: -1, source: SiftSource::RawImage { path: "SeaOfBygoneEras_-1_1024.webp" } },
    Floor { floor: -2, source: SiftSource::RawImage { path: "SeaOfBygoneEras_-2_1024.webp" } },
];

/// 远古圣山
const ANCIENTSACREDMOUNTAIN_FLOORS: &[Floor] = &[
    Floor { floor: 0, source: SiftSource::RawImage { path: "AncientSacredMountain_0_1024.png" } },
    Floor { floor: -1, source: SiftSource::RawImage { path: "AncientSacredMountain_-1_1024.webp" } },
];

pub const ALL_SCENES: &[Scene] = &[
    Scene {
        name: "Teyvat",
        desc: "提瓦特大陆",
        geom: SceneGeom::new(15, 22, 7, 15, 2048, 30, 44),
        floors: TEYVAT_FLOORS,
    },
    Scene {
        name: "TheChasm",
        desc: "层岩巨渊",
        geom: SceneGeom::new(2, 2, 1, 1, 1024, 0, 0),
        floors: THECHASM_FLOORS,
    },
    Scene {
        name: "Enkanomiya",
        desc: "渊下宫",
        geom: SceneGeom::new(3, 3, 1, 1, 1024, 0, 0),
        floors: ENKANOMIYA_FLOORS,
    },
    Scene {
        name: "SeaOfBygoneEras",
        desc: "旧日之海",
        geom: SceneGeom::new(3, 4, 2, 5, 1024, 0, 0),
        floors: SEAOFBYGONEERAS_FLOORS,
    },
    Scene {
        name: "AncientSacredMountain",
        desc: "远古圣山",
        geom: SceneGeom::new(4, 4, 1, 1, 1024, 0, 0),
        floors: ANCIENTSACREDMOUNTAIN_FLOORS,
    },
];

pub fn scene_by_name(name: &str) -> Option<&'static Scene> {
    ALL_SCENES.iter().find(|s| s.name == name)
}
