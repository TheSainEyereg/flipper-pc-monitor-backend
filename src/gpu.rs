use gfxinfo::active_gpu;
use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct GpuInfo {
    pub gpu_usage: u64,
    pub vram_max: u64,
    pub vram_used: u64,
}

impl GpuInfo {
    pub async fn get_gpu_info() -> Option<GpuInfo> {
        let gpu = active_gpu().ok()?;
        let info = gpu.info();

        Some(GpuInfo {
            gpu_usage: info.load_pct() as u64,
            vram_max: info.total_vram(),
            vram_used: info.used_vram(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_gpu_info() {
        let info = GpuInfo::get_gpu_info().await;
        assert!(info.is_some());
    }
}
