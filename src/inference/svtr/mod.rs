//! SVTR
//! 
//! 实现思路来自 [Yap](https://github.com/Alex-Beng/Yap)
//! 
//! 输入固定32x384灰度，输出CTC

pub mod yap;
pub use yap::YapRecognizer;
