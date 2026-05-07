//! 转换SIFT缓存
//! - `BgiPrebuilt`: 直接读BGI生成的缓存
//! - `RawImage`: 在原图运行SIFT detect/compute
//! 用法：
//! cargo run --bin build_sift_cache
//! cargo run --bin build_sift_cache -- --bgi /path/to/BetterGI/Assets/Map --out custom/dir
//! cargo run --bin build_sift_cache -- --scene Teyvat
//! cargo run --bin build_sift_cache -- --force

use std::path::{Path, PathBuf};
use std::time::Instant;
use anyhow::{Context, Result, anyhow};
use opencv::core::{KeyPoint, KeyPointTraitConst, Mat, MatTraitConst, Vector, no_array};
use opencv::features2d::{Feature2DTrait, SIFT};
use opencv::imgcodecs::{IMREAD_GRAYSCALE, imread};
use opencv::prelude::*;
use worse_genshin_impact::navigate::map::cache::{KeyPointRaw, LayerCache, SIFT_DESC_LEN, read_bgi_keypoints};
use worse_genshin_impact::navigate::scene::{ALL_SCENES, Floor, Scene, SiftSource};

#[deprecated(note = "迁移常量")]
const BGI_MAP_ASSETS_DIR: &str = "../BetterGI/Assets/Map";
#[deprecated(note = "迁移常量")]
const MAP_ASSETS_DIR: &str = "assets/map";

fn main() -> Result<()> {
    let mut bgi_dir = PathBuf::from(BGI_MAP_ASSETS_DIR);
    let mut out_dir = PathBuf::from(MAP_ASSETS_DIR);
    let mut only: Option<String> = None;
    let mut force = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--bgi" => bgi_dir = PathBuf::from(args.next().ok_or_else(|| anyhow!("--bgi 缺少值"))?),
            "--out" => out_dir = PathBuf::from(args.next().ok_or_else(|| anyhow!("--out 缺少值"))?),
            "--scene" => only = Some(args.next().ok_or_else(|| anyhow!("--scene 缺少值"))?),
            "--force" => force = true,
            "-h" | "--help" => {
                println!("Usage: build_sift_cache [--bgi DIR] [--out DIR] [--scene NAME] [--force]");
                return Ok(());
            }
            other => return Err(anyhow!("未知参数: {other}")),
        }
    }
    println!("BGI assets : {}", bgi_dir.display());
    println!("output dir : {}", out_dir.display());
    if let Some(n) = &only { println!("only scene : {}", n); }
    if force { println!("force: true"); }
    if !bgi_dir.exists() {
        return Err(anyhow!("BetterGI 资源目录不存在: {}", bgi_dir.display()));
    }
    for scene in ALL_SCENES {
        if let Some(only) = &only && scene.name != only.as_str() {
            continue;
        }
        process_scene(scene, &bgi_dir, &out_dir, force)?;
    }
    println!("\n全部完成");
    Ok(())
}

fn process_scene(scene: &Scene, bgi_dir: &Path, out_dir: &Path, force: bool) -> Result<()> {
    let src_dir = bgi_dir.join(scene.name);
    let dst_dir = out_dir.join(scene.name);
    std::fs::create_dir_all(&dst_dir)?;
    println!("\n=== {} ({}) ===", scene.name, scene.desc);
    if !src_dir.exists() {
        println!("  跳过：BGI 资源目录不存在 {}", src_dir.display());
        return Ok(());
    }
    for floor in scene.floors {
        let out_path = dst_dir.join(format!("{}_{}.sift.bin", scene.name, floor.floor));
        if out_path.exists() && !force {
            println!("  [floor {:>2}] 已存在: {}（--force 强制重建）", floor.floor, out_path.display());
            continue;
        }
        process_floor(scene, floor, &src_dir, &out_path)?;
    }
    Ok(())
}

fn process_floor(scene: &Scene, floor: &Floor, src_dir: &Path, out_path: &Path) -> Result<()> {
    let t0 = Instant::now();
    let (kps, descriptors) = match floor.source {
        SiftSource::BgiPrebuilt { kp, mat } => {
            let kp_path = src_dir.join(kp);
            let mat_path = src_dir.join(mat);
            if !kp_path.exists() || !mat_path.exists() {
                return Err(anyhow!(
                    "缺少素材: {} 或 {}",
                    kp_path.display(),
                    mat_path.display()
                ));
            }
            println!("  [floor {:>2}] 读 BGI 预生成: {}", floor.floor, kp_path.display());
            let kps = read_bgi_keypoints(&kp_path).map_err(|e| anyhow!("read kp: {e}"))?;
            let desc = read_descriptor_png(&mat_path, kps.len())?;
            (kps, desc)
        }
        SiftSource::RawImage { path } => {
            let img_path = src_dir.join(path);
            if !img_path.exists() {
                return Err(anyhow!("缺少原图: {}", img_path.display()));
            }
            println!("  [floor {:>2}] 在原图上提取 SIFT: {}", floor.floor, img_path.display());
            extract_sift(&img_path)?
        }
    };
    println!(
        "    keypoints = {}, desc bytes = {}, took {:?}",
        kps.len(),
        descriptors.len(),
        t0.elapsed()
    );
    let cache = LayerCache {
        version: LayerCache::CURRENT_VERSION,
        scene: scene.name.to_string(),
        floor: floor.floor,
        geom: scene.geom.into(),
        desc_cols: SIFT_DESC_LEN as u32,
        keypoints: kps,
        descriptors,
    };
    let t1 = Instant::now();
    cache.save(out_path).map_err(|e| anyhow!("save {}: {e}", out_path.display()))?;
    let written = std::fs::metadata(out_path)?.len();
    println!(
        "    -> {} ({} bytes, save {:?})",
        out_path.display(),
        written,
        t1.elapsed()
    );
    Ok(())
}

/// 读BGI的.mat.png描述符
fn read_descriptor_png(path: &Path, expected_rows: usize) -> Result<Vec<u8>> {
    let p = path.to_str().ok_or_else(|| anyhow!("非 UTF-8 路径: {}", path.display()))?;
    let img: Mat = imread(p, IMREAD_GRAYSCALE).with_context(|| format!("imread {p}"))?;
    if img.empty() {
        return Err(anyhow!("PNG 解码后为空: {p}"));
    }
    let rows = img.rows() as usize;
    let cols = img.cols() as usize;
    let depth = img.depth();
    if rows != expected_rows {
        return Err(anyhow!("{p}: rows {rows} != keypoints {expected_rows}"));
    }
    if cols != SIFT_DESC_LEN {
        return Err(anyhow!("{p}: cols {cols} != {SIFT_DESC_LEN}"));
    }
    if depth != opencv::core::CV_8U {
        return Err(anyhow!("{p}: 期望CV_8U，实际 depth = {depth}"));
    }
    extract_u8_rows(&img, rows, cols)
}

/// 在原图上跑 SIFT，得到 (KeyPointRaw, descriptors as u8)。
/// 与 BetterGI `Cv2.SIFT.Create() + DetectAndCompute + ConvertTo CV_8U` 等价。
fn extract_sift(img_path: &Path) -> Result<(Vec<KeyPointRaw>, Vec<u8>)> {
    let p = img_path.to_str().ok_or_else(|| anyhow!("非UTF-8路径: {}", img_path.display()))?;
    let img: Mat = imread(p, IMREAD_GRAYSCALE).with_context(|| format!("imread {p}"))?;
    if img.empty() {
        return Err(anyhow!("图像解码失败: {p}"));
    }
    println!("原图大小 {}x{}", img.cols(), img.rows());
    let mut sift = SIFT::create_def().map_err(|e| anyhow!("SIFT::create: {e}"))?;
    let mut kps: Vector<KeyPoint> = Vector::new();
    let mut desc_f32 = Mat::default();
    sift.detect_and_compute(&img, &no_array(), &mut kps, &mut desc_f32, false)
        .map_err(|e| anyhow!("detect_and_compute: {e}"))?;
    let n = kps.len();
    if n == 0 {
        return Err(anyhow!("{p}: 未检测到任何SIFT特征"));
    }
    if desc_f32.cols() as usize != SIFT_DESC_LEN {
        return Err(anyhow!("{p}: 描述符维度 {} != {}", desc_f32.cols(), SIFT_DESC_LEN));
    }
    // KeyPoint -> KeyPointRaw
    let mut kps_raw = Vec::with_capacity(n);
    for kp in kps.iter() {
        let pt = kp.pt();
        kps_raw.push(KeyPointRaw {
            x: pt.x,
            y: pt.y,
            size: kp.size(),
            angle: kp.angle(),
            response: kp.response(),
            octave: kp.octave(),
            class_id: kp.class_id(),
        });
    }
    // f32描述符 -> u8
    let descriptors = quantize_descriptors_u8(&desc_f32)?;
    Ok((kps_raw, descriptors))
}

/// 将N*128 CV_32F描述符转换为u8
fn quantize_descriptors_u8(desc: &Mat) -> Result<Vec<u8>> {
    let rows = desc.rows() as usize;
    let cols = desc.cols() as usize;
    if desc.depth() != opencv::core::CV_32F {
        return Err(anyhow!("expected CV_32F, got {}", desc.depth()));
    }
    let mut out = vec![0u8; rows * cols];
    if desc.is_continuous() {
        let raw = desc.data_typed::<f32>().context("data_typed")?;
        for (i, &v) in raw.iter().enumerate() {
            out[i] = saturate_to_u8(v);
        }
    } else {
        for r in 0..rows {
            let row = desc.at_row::<f32>(r as i32).context("at_row")?;
            for c in 0..cols {
                out[r * cols + c] = saturate_to_u8(row[c]);
            }
        }
    }
    Ok(out)
}

#[inline]
fn saturate_to_u8(v: f32) -> u8 {
    let r = v.round();
    if r <= 0.0 { 0 } else if r >= 255.0 { 255 } else { r as u8 }
}

fn extract_u8_rows(img: &Mat, rows: usize, cols: usize) -> Result<Vec<u8>> {
    let total = rows * cols;
    let mut out = vec![0u8; total];
    if img.is_continuous() {
        let src = img.data_bytes().context("data_bytes")?;
        if src.len() < total {
            return Err(anyhow!("data_bytes len {} < {total}", src.len()));
        }
        out.copy_from_slice(&src[..total]);
    } else {
        for r in 0..rows {
            let row_slice = img.at_row::<u8>(r as i32).context("at_row")?;
            out[r * cols..(r + 1) * cols].copy_from_slice(&row_slice[..cols]);
        }
    }
    Ok(out)
}
