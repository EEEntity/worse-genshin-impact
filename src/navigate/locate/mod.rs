pub mod locator;
pub mod multi_scene;

pub use locator::{LocateInfo, Locator, detect_sift, ensure_gray, new_sift};
pub use multi_scene::MultiSceneLocator;
