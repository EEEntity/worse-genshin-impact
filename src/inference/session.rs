//! ONNX Runtime会话封装
//! GPU接口先留下

use anyhow::{Context, Result};
use ndarray::{Array, Array4, IxDyn};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::TensorRef;
use std::path::Path;
use std::sync::OnceLock;

use crate::inference::model::ORT_LIB_PATH;

static ORT_INIT: OnceLock<std::result::Result<(), String>> = OnceLock::new();

fn ensure_ort_initialized() -> Result<()> {
    let init = ORT_INIT.get_or_init(|| {
        ort::init_from(ORT_LIB_PATH)
            .map(|_| ())
            .map_err(|e| format!("failed to initialize ORT from `{ORT_LIB_PATH}`: {e}"))
    });
    init.as_ref()
        .map_err(|e| anyhow::anyhow!(e.clone()))?;
    Ok(())
}

/// 推理后端
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Provider {
    #[default]
    CPU,
    CUDA,
    OpenVino,
    ROCm,
    MIGraphX,
}

/// 会话创建选项
#[derive(Debug, Clone, Default)]
pub struct SessionOptions {
    pub provider: Provider,
    pub optimize: bool,
}

impl SessionOptions {
    pub fn cpu() -> Self {
        Self { provider: Provider::CPU, optimize: true }
    }
}

/// 单模型ONNXRuntime推理会话
pub struct OrtSession {
    session: Session,
}

impl OrtSession {
    /// 默认CPU/图优化加载模型
    pub fn new(model_path: impl AsRef<Path>) -> Result<Self> {
        Self::with_options(model_path, &SessionOptions::cpu())
    }
    /// 非CPU Provider返回`Err`
    pub fn with_options(model_path: impl AsRef<Path>, opts: &SessionOptions) -> Result<Self> {
        match opts.provider {
            Provider::CPU => {}
            other => anyhow::bail!("Provider {:?} not implemented yet", other),
        }
        ensure_ort_initialized()?;
        let level = if opts.optimize {
            GraphOptimizationLevel::All
        } else {
            GraphOptimizationLevel::Disable
        };
        let session = Session::builder()
            .context("failed to create ORT session builder")?
            .with_optimization_level(level)
            .map_err(|e|anyhow::anyhow!("failed to set optimization level: {e}"))?
            .commit_from_file(model_path)
            .context("failed to load ONNX model")?;
        Ok(Self { session })
    }
    /// 执行
    pub fn run(&mut self, input: &Array4<f32>) -> Result<Array<f32, IxDyn>> {
        let input_name = self.session.inputs()[0].name().to_owned();
        let tensor = TensorRef::from_array_view(input.view())
            .context("failed to create input tensor")?;
        let outputs = self
            .session
            .run(ort::inputs![input_name.as_str() => tensor])
            .context("ORT run failed")?;
        let view = outputs[0]
            .try_extract_array::<f32>()
            .context("failed to extract output tensor")?;
        let shape_out = view.shape().to_vec();
        let data: Vec<f32> = view.iter().cloned().collect();
        Array::from_shape_vec(IxDyn(&shape_out), data).context("failed to reshape output")
    }
    /// 公开底层[`ort::session::Session`]
    pub fn raw(&mut self) -> &mut Session {
        &mut self.session
    }
}
