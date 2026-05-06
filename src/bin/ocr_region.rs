/// 截取指定窗口并进行OCR
/// 
/// cargo run --bin ocr_region --release
/// - 弹出窗口选择器，选择目标
/// - 等待GStreamer管道推送第一帧
/// - 截取(X,Y,W,H)区域，保存为capture_region.ppm
/// - 执行OCR

use std::fs::File;
use std::io::Write;
use worse_genshin_impact::capture;
use worse_genshin_impact::inference::ocr;

const X: u32 = 100;
const Y: u32 = 100;
const W: u32 = 400;
const H: u32 = 400;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("请选择目标窗口...");
    let capturer = capture::init()?;
    // 等待GStreamer管道推送第一帧
    println!("管道已启动，等待第一帧...");
    std::thread::sleep(std::time::Duration::from_millis(500));
    // 截取区域
    let pixels = capturer
        .get_region(X, Y, W, H)
        .ok_or("读取失败(未收到帧或坐标越界)")?;
    let mut f = File::create("capture_region.ppm")?;
    write!(f, "P6\n{W} {H}\n255\n")?;
    f.write_all(&pixels)?;
    println!("已保存截取区域到 capture_region.ppm ({W}x{H})");
    let mut engine = ocr::OcrEngine::new()?;
    let results = engine.run(&pixels, W, H)?;
    if results.is_empty() {
        println!("未检测到文字");
    } else {
        for r in &results {
            let [x, y, w, h] = r.bbox;
            println!("[{x},{y},{w},{h}] \"{}\" score={:.2}", r.text, r.score);
        }
    }
    capturer.stop();
    Ok(())
}
