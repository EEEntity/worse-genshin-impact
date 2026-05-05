// 检测预处理/DBNet

use anyhow::{Context, Result};
use clipper2::{inflate, EndType, JoinType, Paths};
use ndarray::Array4;
use opencv::{
    core::{self, Mat, Point as CvPoint, Scalar, Vector},
    imgproc::{self, CHAIN_APPROX_SIMPLE, INTER_LINEAR, RETR_LIST},
    prelude::*,
};

const BOX_SORT_Y_THRESHOLD: f32 = 10.0;

// 预处理
/// BGR Mat -> [1,3,H,W] f32, normalize(x/127.5-1.0)
pub fn det_preprocess(img: &Mat, limit_side_len: i32, limit_type: &str) -> Result<Array4<f32>> {
    let h = img.rows();
    let w = img.cols();
    let ratio = if limit_type == "max" {
        if h.max(w) > limit_side_len { limit_side_len as f64 / h.max(w) as f64 } else { 1.0 }
    } else {
        let min_wh = h.min(w);
        if min_wh < limit_side_len { limit_side_len as f64 / min_wh as f64 } else { 1.0 }
    };
    let new_h = (((h as f64 * ratio) / 32.0).round() as i32) * 32;
    let new_w = (((w as f64 * ratio) / 32.0).round() as i32) * 32;
    let resized_tmp;
    let src = if new_h != h || new_w != w {
        let mut tmp = Mat::default();
        imgproc::resize(img, &mut tmp, opencv::core::Size::new(new_w, new_h), 0.0, 0.0, INTER_LINEAR)
            .context("det_preprocess resize failed")?;
        resized_tmp = tmp;
        &resized_tmp
    } else {
        img
    };
    let out_h = src.rows() as usize;
    let out_w = src.cols() as usize;
    let mut arr = Array4::<f32>::zeros((1, 3, out_h, out_w));
    for c in 0..3usize {
        for row in 0..out_h {
            let row_ptr = src.ptr(row as i32).context("ptr failed")?;
            for col in 0..out_w {
                let v = unsafe { *row_ptr.add(col * 3 + c) } as f32;
                arr[[0, c, row, col]] = v / 127.5 - 1.0;
            }
        }
    }
    Ok(arr)
}

pub struct  DbConfig {
    pub thresh: f32,
    pub box_thresh: f32,
    pub max_candidates: usize,
    pub unclip_ratio: f64,
    pub use_dilation: bool,
    pub min_size: f32,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self { thresh: 0.3, box_thresh: 0.5, max_candidates: 1000, unclip_ratio: 1.6, use_dilation: true, min_size: 3.0 }
    }
}

/// 4顶点box
pub type Box4 = [[i32; 2]; 4];

/// DBNet后处理
pub fn db_postprocess(prob_map: &ndarray::Array4<f32>, ori_shape: (i32, i32), cfg: &DbConfig) -> Result<Vec<Box4>> {
    let (ori_h, ori_w) = ori_shape;
    let h = prob_map.dim().2;
    let w = prob_map.dim().3;
    let prob_slice = prob_map.as_slice().ok_or_else(|| anyhow::anyhow!("det output not contiguous"))?;
    let prob_mat = unsafe {
        Mat::new_rows_cols_with_data_unsafe(
            h as i32, w as i32, core::CV_32FC1,
            prob_slice.as_ptr() as *mut std::ffi::c_void, core::Mat_AUTO_STEP,
        )
    }
    .context("create prob_mat failed")?;
    // 二值化
    let mut binary = Mat::default();
    imgproc::threshold(&prob_mat, &mut binary, cfg.thresh as f64, 1.0, imgproc::THRESH_BINARY)
        .context("threshold failed")?;
    let mut binary_u8 = Mat::default();
    binary.convert_to(&mut binary_u8, core::CV_8UC1, 255.0, 0.0).context("convert_to u8 failed")?;
    // dilate(可选)
    let mask = if cfg.use_dilation {
        let kernel = Mat::ones(2, 2, core::CV_8UC1).context("ones failed")?.to_mat().context("to_mat failed")?;
        let mut dilated = Mat::default();
        imgproc::dilate(&binary_u8, &mut dilated, &kernel, CvPoint::new(-1, -1), 1, core::BORDER_CONSTANT, Scalar::all(0.0))
            .context("dilate failed")?;
        dilated
    } else {
        binary_u8
    };
    let mut contours: Vector<Vector<CvPoint>> = Vector::new();
    imgproc::find_contours(&mask, &mut contours, RETR_LIST, CHAIN_APPROX_SIMPLE, CvPoint::new(0, 0))
        .context("find_contours failed")?;
    let n = contours.len().min(cfg.max_candidates);
    let mut boxes: Vec<Box4> = Vec::with_capacity(n);
    for i in 0..n {
        let contour = contours.get(i).context("get contour")?;
        let (pts, sside) = get_mini_boxes(&contour)?;
        if sside < cfg.min_size { continue; }
        let score = box_score_fast(&prob_mat, &pts)?;
        if score < cfg.box_thresh { continue; }
        let expanded = unclip(&pts, cfg.unclip_ratio)?;
        if expanded.is_empty() { continue; }
        let exp_contour: Vector<CvPoint> = Vector::from_iter(
            expanded.iter().map(|p| CvPoint::new(p[0] as i32, p[1] as i32))
        );
        let (pts2, sside2) = get_mini_boxes(&exp_contour)?;
        if sside2 < cfg.min_size + 2.0 { continue; }
        let mut bx: Box4 = [[0; 2]; 4];
        for (j, pt) in pts2.iter().enumerate() {
            bx[j] = [
                ((pt[0] / w as f32) * ori_w as f32).round().clamp(0.0, ori_w as f32) as i32,
                ((pt[1] / h as f32) * ori_h as f32).round().clamp(0.0, ori_h as f32) as i32,
            ];
        }
        boxes.push(bx);
    }
    let filtered: Vec<Box4> = boxes
        .into_iter()
        .filter_map(|bx| {
            let bx = order_points_clockwise(bx);
            let w_rect = pt_dist(bx[0], bx[1]);
            let h_rect = pt_dist(bx[0], bx[3]);
            if w_rect <= 3.0 || h_rect <= 3.0 { None } else { Some(bx) }
        })
        .collect();
    Ok(sorted_boxes(filtered))
}

fn get_mini_boxes(contour: &Vector<CvPoint>) -> Result<(Vec<[f32; 2]>, f32)> {
    let rect = imgproc::min_area_rect(contour).context("min_area_rect failed")?;
    let mut box_pts_mat = Mat::default();
    imgproc::box_points(rect, &mut box_pts_mat).context("box_points failed")?;
    let mut pts: Vec<[f32; 2]> = (0..4)
        .map(|i| [*box_pts_mat.at_2d::<f32>(i, 0).unwrap(), *box_pts_mat.at_2d::<f32>(i, 1).unwrap()])
        .collect();
    pts.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());
    let (i1, i4) = if pts[1][1] > pts[0][1] { (0, 1) } else { (1, 0) };
    let (i2, i3) = if pts[3][1] > pts[2][1] { (2, 3) } else { (3, 2) };
    let sside = rect.size.width.min(rect.size.height);
    Ok((vec![pts[i1], pts[i2], pts[i3], pts[i4]], sside))
}

fn box_score_fast(bitmap: &Mat, pts: &[[f32; 2]]) -> Result<f32> {
    let bh = bitmap.rows();
    let bw = bitmap.cols();
    let xs: Vec<f32> = pts.iter().map(|p| p[0]).collect();
    let ys: Vec<f32> = pts.iter().map(|p| p[1]).collect();
    let xmin = xs.iter().cloned().fold(f32::MAX, f32::min).floor().max(0.0) as i32;
    let xmax = xs.iter().cloned().fold(f32::MIN, f32::max).ceil().min((bw - 1) as f32) as i32;
    let ymin = ys.iter().cloned().fold(f32::MAX, f32::min).floor().max(0.0) as i32;
    let ymax = ys.iter().cloned().fold(f32::MIN, f32::max).ceil().min((bh - 1) as f32) as i32;
    let rw = (xmax - xmin + 1).max(1);
    let rh = (ymax - ymin + 1).max(1);
    let mut mask = Mat::zeros(rh, rw, core::CV_8UC1).context("zeros")?.to_mat().context("to_mat")?;
    let shifted: Vector<CvPoint> = Vector::from_iter(
        pts.iter().map(|p| CvPoint::new((p[0] - xmin as f32) as i32, (p[1] - ymin as f32) as i32))
    );
    let contours_arr: Vector<Vector<CvPoint>> = Vector::from_iter([shifted]);
    imgproc::fill_poly(&mut mask, &contours_arr, Scalar::all(1.0), imgproc::LINE_8, 0, CvPoint::new(0, 0))
        .context("fill_poly failed")?;
    let roi = Mat::roi(bitmap, core::Rect::new(xmin, ymin, rw, rh)).context("roi failed")?;
    let mean = core::mean(&roi, &mask).context("mean failed")?;
    Ok(mean[0] as f32)
}
fn unclip(pts: &[[f32; 2]], unclip_ratio: f64) -> Result<Vec<[f64; 2]>> {
    let n = pts.len();
    let mut area = 0.0f64;
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i][0] as f64 * pts[j][1] as f64;
        area -= pts[j][0] as f64 * pts[i][1] as f64;
    }
    area = area.abs() / 2.0;
    let perimeter: f64 = (0..n)
        .map(|i| { let j = (i + 1) % n; let dx = pts[j][0] as f64 - pts[i][0] as f64; let dy = pts[j][1] as f64 - pts[i][1] as f64; (dx * dx + dy * dy).sqrt() })
        .sum();
    if perimeter == 0.0 { return Ok(vec![]); }
    let distance = area * unclip_ratio / perimeter;
    let path_pts: Paths = pts.iter().map(|p| (p[0] as f64, p[1] as f64)).collect::<Vec<_>>().into();
    let result = inflate(path_pts, distance, JoinType::Round, EndType::Polygon, 0.0);
    if result.is_empty() { return Ok(vec![]); }
    Ok(result.into_iter().next().unwrap_or_default().into_iter().map(|pt| [pt.x(), pt.y()]).collect())
}

fn order_points_clockwise(pts: Box4) -> Box4 {
    let mut sorted = pts;
    sorted.sort_by(|a, b| a[0].cmp(&b[0]));
    let left = [sorted[0], sorted[1]];
    let right = [sorted[2], sorted[3]];
    let (tl, bl) = if left[0][1] <= left[1][1] { (left[0], left[1]) } else { (left[1], left[0]) };
    let (tr, br) = if right[0][1] <= right[1][1] { (right[0], right[1]) } else { (right[1], right[0]) };
    [tl, tr, br, bl]
}

fn sorted_boxes(mut boxes: Vec<Box4>) -> Vec<Box4> {
    if boxes.is_empty() { return boxes; }
    boxes.sort_by(|a, b| a[0][1].cmp(&b[0][1]));
    let n = boxes.len();
    let mut line_ids = vec![0usize; n];
    let mut line_id = 0;
    for i in 1..n {
        if (boxes[i][0][1] - boxes[i - 1][0][1]) as f32 >= BOX_SORT_Y_THRESHOLD { line_id += 1; }
        line_ids[i] = line_id;
    }
    let mut indexed: Vec<(usize, usize, i32)> = boxes.iter().enumerate().map(|(i, bx)| (i, line_ids[i], bx[0][0])).collect();
    indexed.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)));
    indexed.into_iter().map(|(i, _, _)| boxes[i]).collect()
}

fn pt_dist(a: [i32; 2], b: [i32; 2]) -> f32 {
    let dx = (a[0] - b[0]) as f32;
    let dy = (a[1] - b[1]) as f32;
    (dx * dx + dy * dy).sqrt()
}
