//! 文件名排序
//! - ASCII/自然数值序
//! - 中文按拼音排序

use std::io;
use std::path::{Path, PathBuf};
use pinyin::ToPinyin;

/// 读取目录并返回子项路径
pub fn sorted_read_dir_paths(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    sort_paths_by_name(&mut out);
    Ok(out)
}

/// 按路径最后一段文件名排序
pub fn sort_paths_by_name(paths: &mut [PathBuf]) {
    paths.sort_by(|a, b| {
        let an = a.file_name().and_then(|s| s.to_str()).unwrap_or_default();
        let bn = b.file_name().and_then(|s| s.to_str()).unwrap_or_default();
        let ak = normalize_for_sort(an);
        let bk = normalize_for_sort(bn);

        let ord = natord::compare_ignore_case(&ak, &bk);
        if ord == std::cmp::Ordering::Equal {
            natord::compare_ignore_case(an, bn)
        } else {
            ord
        }
    });
}

fn normalize_for_sort(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            continue;
        }
        if ch.is_ascii() {
            out.push(ch);
            continue;
        }
        if let Some(py) = ch.to_pinyin() {
            out.push_str(py.plain());
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}
