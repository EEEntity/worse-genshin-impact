//! 鱼饵/鱼/鱼塘

use opencv::core::Rect;

use crate::inference::yolo::decode::Detection;

/// 鱼饵类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaitType {
    FruitPasteBait,
    RedrotBait,
    FalseWormBait,
    FakeFlyBait,
    SugardewBait,
    SourBait,
    FlashingMaintenanceMekBait,
    SpinelgrainBait,
    EmberglowBait,
    BerryBait,
    RefreshingLakkaBait,
}

impl BaitType {
    /// 中文名
    pub const fn chinese_name(self) -> &'static str {
        use BaitType::*;
        match self {
            FruitPasteBait => "果酿饵",
            RedrotBait => "赤糜饵",
            FalseWormBait => "蠕虫假饵",
            FakeFlyBait => "飞蝇假饵",
            SugardewBait => "甘露饵",
            SourBait => "酸桔饵",
            FlashingMaintenanceMekBait => "维护机关频闪诱饵",
            SpinelgrainBait => "澄晶果粒饵",
            EmberglowBait => "温火饵",
            BerryBait => "槲梭饵",
            RefreshingLakkaBait => "清白饵",
        }
    }
    /// 中文名 -> BaitType
    pub fn from_chinese_name(s: &str) -> Option<Self> {
        use BaitType::*;
        for b in [
            FruitPasteBait,
            RedrotBait,
            FalseWormBait,
            FakeFlyBait,
            SugardewBait,
            SourBait,
            FlashingMaintenanceMekBait,
            SpinelgrainBait,
            EmberglowBait,
            BerryBait,
            RefreshingLakkaBait,
        ] {
            if b.chinese_name() == s {
                return Some(b);
            }
        }
        None
    }
}

/// 鱼
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BigFishType {
    pub name: &'static str,
    pub bait: BaitType,
    pub chinese_name: &'static str,
    pub net_index: u8,
}

pub const BIG_FISH_TYPES: &[BigFishType] = &[
    BigFishType { name: "medaka", bait: BaitType::FruitPasteBait, chinese_name: "花鳉", net_index: 0 },
    BigFishType { name: "large medaka", bait: BaitType::FruitPasteBait, chinese_name: "大花鳉", net_index: 1 },
    BigFishType { name: "stickleback", bait: BaitType::RedrotBait, chinese_name: "棘鱼", net_index: 2 },
    BigFishType { name: "koi", bait: BaitType::FakeFlyBait, chinese_name: "假龙", net_index: 3 },
    BigFishType { name: "koi head", bait: BaitType::FakeFlyBait, chinese_name: "假龙头", net_index: 3 },
    BigFishType { name: "butterflyfish", bait: BaitType::FalseWormBait, chinese_name: "蝶鱼", net_index: 4 },
    BigFishType { name: "pufferfish", bait: BaitType::FakeFlyBait, chinese_name: "炮鲀", net_index: 5 },
    BigFishType { name: "ray", bait: BaitType::FakeFlyBait, chinese_name: "鳐", net_index: 6 },
    BigFishType { name: "angler", bait: BaitType::SugardewBait, chinese_name: "角鲀", net_index: 7 },
    BigFishType { name: "axe marlin", bait: BaitType::SugardewBait, chinese_name: "斧枪鱼", net_index: 8 },
    BigFishType { name: "heartfeather bass", bait: BaitType::SourBait, chinese_name: "心羽鲈", net_index: 9 },
    BigFishType { name: "maintenance mek", bait: BaitType::FlashingMaintenanceMekBait, chinese_name: "维护机关", net_index: 10 },
    BigFishType { name: "unihornfish", bait: BaitType::SpinelgrainBait, chinese_name: "独角鱼", net_index: 10 },
    BigFishType { name: "sunfish", bait: BaitType::SpinelgrainBait, chinese_name: "翻车鲀", net_index: 7 },
    BigFishType { name: "rapidfish", bait: BaitType::SpinelgrainBait, chinese_name: "斗士急流鱼", net_index: 9 },
    BigFishType { name: "phony unihornfish", bait: BaitType::EmberglowBait, chinese_name: "燃素独角鱼", net_index: 10 },
    BigFishType { name: "magma rapidfish", bait: BaitType::EmberglowBait, chinese_name: "炽岩斗士急流鱼", net_index: 9 },
    BigFishType { name: "secret source", bait: BaitType::EmberglowBait, chinese_name: "秘源机关・巡戒使", net_index: 9 },
    BigFishType { name: "mauler shark", bait: BaitType::RefreshingLakkaBait, chinese_name: "凶凶鲨", net_index: 9 },
    BigFishType { name: "crystal eye", bait: BaitType::RefreshingLakkaBait, chinese_name: "明眼鱼", net_index: 9 },
    BigFishType { name: "axehead", bait: BaitType::BerryBait, chinese_name: "巨斧鱼", net_index: 9 },
];

impl BigFishType {
    pub fn from_name(name: &str) -> Option<&'static BigFishType> {
        let norm = name.replace('_', " ");
        BIG_FISH_TYPES.iter().find(|f| f.name == name || f.name == norm)
    }
}

/// 一条鱼
#[derive(Debug, Clone)]
pub struct OneFish {
    pub fish_type: &'static BigFishType,
    pub rect: Rect,
    pub confidence: f32,
}

/// 鱼塘
#[derive(Debug, Clone, Default)]
pub struct Fishpond {
    /// 所有鱼按置信度降序排列
    pub fishes: Vec<OneFish>,
    /// 抛竿落点矩形
    pub target_rect: Option<Rect>,
    /// 最小包围所有鱼的矩形，空鱼塘为`None`
    pub fishpond_rect: Option<Rect>,
}

impl Fishpond {
    /// 从YOLO检测结果构造
    pub fn from_detections(
        detections: &[Detection],
        image_w: i32,
        image_h: i32,
        include_target: bool,
        ignore_obtained: bool,
    ) -> Self {
        let mut pond = Fishpond::default();
        let img_w_f = image_w as f32;
        let img_h_f = image_h as f32;
        for det in detections {
            if det.score < 0.4 {
                continue;
            }
            // 模型可能用下划线代替空格
            let label_norm = det.label.replace('_', " ");
            let rect = Rect {
                x: det.bbox.x as i32,
                y: det.bbox.y as i32,
                width: det.bbox.w as i32,
                height: det.bbox.h as i32,
            };
            // rod/err rod单独走target
            if label_norm == "rod" || label_norm == "err rod" {
                pond.target_rect = Some(rect);
                continue;
            }
            // 抛竿阶段只看koi head
            if include_target && label_norm == "koi" {
                continue;
            }
            // 忽略"获得物品"图标干扰
            if ignore_obtained {
                let bw = det.bbox.w;
                let bh = det.bbox.h;
                // 界面左侧获得提示
                if bw < img_w_f * 0.036 && bh < img_w_f * 0.036 {
                    let huode = Rect {
                        x: (0.04375 * img_w_f) as i32,
                        y: (0.4666 * img_h_f) as i32,
                        width: (0.1 * img_w_f) as i32,
                        height: (0.1 * img_w_f) as i32,
                    };
                    if rect_contains(huode, rect) {
                        continue;
                    }
                }
                // 界面中央获得提示(观赏鱼)
                if bw > img_w_f * 0.03 && bw < img_w_f * 0.06
                    && bh > img_w_f * 0.03 && bh < img_w_f * 0.06
                {
                    let huode = Rect {
                        x: (0.4 * img_w_f) as i32,
                        y: (0.445 * img_h_f) as i32,
                        width: (0.2 * img_w_f) as i32,
                        height: (0.06125 * img_w_f) as i32,
                    };
                    if rect_contains(huode, rect) {
                        continue;
                    }
                }
            }
            let Some(big) = BigFishType::from_name(&label_norm) else {
                // 未知类跳过
                // 也许可以加上warn
                continue;
            };
            pond.fishes.push(OneFish {
                fish_type: big,
                rect,
                confidence: det.score,
            });
        }
        // 按置信度降序
        pond.fishes
            .sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        pond.fishpond_rect = compute_fishpond_rect(&pond.fishes);
        pond
    }
    pub fn is_empty(&self) -> bool {
        self.fishes.is_empty()
    }
}

fn rect_contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

fn compute_fishpond_rect(fishes: &[OneFish]) -> Option<Rect> {
    if fishes.is_empty() {
        return None;
    }
    let (mut l, mut t, mut r, mut b) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for f in fishes {
        l = l.min(f.rect.x);
        t = t.min(f.rect.y);
        r = r.max(f.rect.x + f.rect.width);
        b = b.max(f.rect.y + f.rect.height);
    }
    Some(Rect { x: l, y: t, width: r - l, height: b - t })
}
