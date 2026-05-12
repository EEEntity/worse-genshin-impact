//! 复苏弹窗识别

use opencv::core::Mat;

use crate::navigate::bv::assets;
use crate::navigate::bv::matcher::matches;
use crate::navigate::error::NavigateError;

/// 是否处于复苏弹窗
/// 
/// FP情形: 秘境等带"确定"按钮的弹窗
/// 最好在收到`true`后再做一次OCR确认是"复苏"
pub fn is_in_revive_prompt(screen: &Mat) -> Result<bool, NavigateError> {
    matches(screen, assets::confirm_button()?)
}
