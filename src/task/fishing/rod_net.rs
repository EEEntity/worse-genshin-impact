//! 抛竿距离预测
//! 
//! 参考[HutaoFisher](https://github.com/myHuTao-qwq/HutaoFisher)几何模型
//! 
//! 没做tensor版本，直接算术实现

/// 抛竿网格输入
#[derive(Debug, Clone, Copy)]
pub struct RodInput {
    pub rod_x1: f64,
    pub rod_x2: f64,
    pub rod_y1: f64,
    pub rod_y2: f64,
    pub fish_x1: f64,
    pub fish_x2: f64,
    pub fish_y1: f64,
    pub fish_y2: f64,
    /// `BigFishType.net_index`，0..=10
    pub fish_label: usize,
}

/// 抛竿状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RodState {
    /// 0距离合适，抛竿
    JustRight,
    /// 1太近，鼠标要远离鱼方向
    TooClose,
    /// 2太远，鼠标要靠近鱼方向
    TooFar,
}

// 模型常数
const ALPHA: f64 = 1734.34 / 2.5;
/// z偏移
const DZ: [f64; 11] = [
    1.0307939, 1.5887239,  1.4377865, 0.8548809,
    1.8640924, -0.1687729, 1.8621461, 0.7167622,
    1.7071064, 1.8727832,  0.5531539,
];
/// h系数
const H_COEFF: [f64; 11] = [
    0.5840698,  0.8029298,  0.6090596,
    -0.1390072, 0.7214464,  -0.6076725,
    0.3286690,  -0.2991239, 0.6072225,
    0.7662407,  -0.3689651,
];
/// 11x3线性层权重
const WEIGHT: [[f64; 3]; 11] = [
    [ 0.7779633, -1.7124480,  2.7366412],
    [-0.0381155, -1.6536976,  3.5904298],
    [ 0.1947731, -0.0445049,  0.8416666],
    [-0.0331017, -1.3641578,  1.2834741],
    [ 1.0268835, -1.6553984,  2.9930501],
    [ 0.0108103, -0.8515291,  1.0032536],
    [-0.0746362, -0.9677668,  0.7450780],
    [ 0.7382144, -9.5275803,  2.6134675],
    [-0.3597502, -1.7422760,  1.4354013],
    [-0.0578425, -2.0274212,  1.7173727],
    [-0.1225260, -1.0630554,  1.2958838],
];
/// 11x3线性层偏置
const BIAS: [[f64; 3]; 11] = [
    [ 3.1733532,   9.3601589, -11.0612173],
    [ 6.4961057,  11.2683334, -13.7752209],
    [ 2.3662698,   2.4709859,  -2.5402584],
    [ 2.4701204,   8.5112562,  -7.6070199],
    [ 0.9597272,   8.9189463, -11.9037018],
    [ 2.1239815,   5.8446727,  -5.7748013],
    [ 2.1403685,   5.5432696,  -4.0048418],
    [-9.0128260,  28.4402637, -24.2205143],
    [ 5.2072763,   8.6428480,  -9.2946615],
    [ 4.9253063,  11.4634714,  -9.4336052],
    [ 5.2460732,   7.7711511,  -7.5998945],
];
/// 校准偏移
const OFFSET: [f64; 11] = [
    0.8, 0.4, 0.35, 0.35, 0.6, 0.3, 0.3, 0.8, 0.8, 0.8, 0.8,
];

/// 入口
pub fn get_rod_state(input: &RodInput) -> RodState {
    let s = compute_scores(input);
    // argmax
    let mut idx = 0;
    let mut best = s[0];
    for i in 1..3 {
        if s[i] > best {
            best = s[i];
            idx = i;
        }
    }
    match idx {
        0 => RodState::JustRight,
        1 => RodState::TooClose,
        _ => RodState::TooFar,
    }
}

fn compute_scores(input: &RodInput) -> [f64; 3] {
    let label = input.fish_label.min(10);
    let (y0, z0, t, u, mut v, h) = preprocess(input);

    v -= h * H_COEFF[label];

    let dz = DZ[label];
    let z0_dz = z0 + dz;
    let denom = t - v;
    let x = u * z0_dz * (1.0 + t * t).sqrt() / denom;
    let y = z0_dz * (1.0 + t * v) / denom;
    let dist = (x * x + (y - y0) * (y - y0)).sqrt();

    let mut logits = [0.0; 3];
    for i in 0..3 {
        logits[i] = WEIGHT[label][i] * dist + BIAS[label][i];
    }

    let mut pred = softmax(logits);
    pred[0] -= OFFSET[label];
    pred
}

fn preprocess(input: &RodInput) -> (f64, f64, f64, f64, f64, f64) {
    let mut a = (input.rod_x2 - input.rod_x1) / 2.0 / ALPHA;
    let mut b = (input.rod_y2 - input.rod_y1) / 2.0 / ALPHA;
    let h = (input.fish_y2 - input.fish_y1) / 2.0 / ALPHA;

    if a < b {
        b = (a * b).sqrt();
        a = b + 1e-6;
    }
    let v0 = (288.0 - (input.rod_y1 + input.rod_y2) / 2.0) / ALPHA;
    let u = (input.fish_x1 + input.fish_x2 - input.rod_x1 - input.rod_x2) / 2.0 / ALPHA;
    let v = (288.0 - (input.fish_y1 + input.fish_y2) / 2.0) / ALPHA;

    let a2 = a * a;
    let b2 = b * b;
    let y0 = (a2 * a2 - b2 + a2 * (1.0 - b2 + v0 * v0)).sqrt() / a2;
    let z0 = b / a2;
    let t = a2 * (y0 * b + v0) / (a2 - b2);

    (y0, z0, t, u, v, h)
}

fn softmax(x: [f64; 3]) -> [f64; 3] {
    let m = x[0].max(x[1]).max(x[2]);
    let e0 = (x[0] - m).exp();
    let e1 = (x[1] - m).exp();
    let e2 = (x[2] - m).exp();
    let s = e0 + e1 + e2;
    [e0 / s, e1 / s, e2 / s]
}
