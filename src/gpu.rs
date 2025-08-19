use serde::Serialize;

mod amd;
mod nvidia;

#[derive(Serialize, Debug, Clone)]
pub struct GpuInfo {
    pub gpu_usage: u64,
    pub vram_max: u64,
    pub vram_used: u64,
}

impl GpuInfo {
    pub async fn get_gpu_info() -> Option<GpuInfo> {
        match GpuType::guess().await {
            GpuType::Nvidia => nvidia::get_gpu_info().await,
            GpuType::Amd => amd::get_gpu_info().await,
            GpuType::Unknown => None,
        }
    }
}

pub enum GpuType {
    Nvidia,
    Amd,
    Unknown,
}

impl GpuType {
    pub async fn guess() -> GpuType {
        if Self::is_executable_exists("nvidia-smi").await {
            GpuType::Nvidia
        } else if Self::is_executable_exists("rocm-smi").await {
            GpuType::Amd
        } else {
            GpuType::Unknown
        }
    }

    async fn is_executable_exists(name: &str) -> bool {
        let which = if cfg!(windows) { "where" } else { "which" };

        tokio::process::Command::new(which)
            .arg(name)
            .status()
            .await
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_is_executable_exists() {
        let exists = GpuType::is_executable_exists("nvidia-smi").await
            || GpuType::is_executable_exists("rocm-smi").await;
        assert!(exists);
    }

    #[tokio::test]
    async fn test_get_gpu_info() {
        let info = GpuInfo::get_gpu_info().await;
        assert!(info.is_some());
    }

    #[tokio::test]
    async fn test_get_gpu_type() {
        let info = GpuType::guess().await;
        assert!(matches!(info, GpuType::Nvidia | GpuType::Amd));
    }
}
