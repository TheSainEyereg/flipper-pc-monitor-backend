use crate::helpers::{avg_vecu32, nvd_r2u64, pop_4u8};
use serde::Serialize;
use sysinfo::MemoryRefreshKind;
use tokio::io::AsyncReadExt;

/*
typedef struct {
    uint8_t cpu_usage;
    uint16_t ram_max;
    uint8_t ram_usage;
    char ram_unit[4];
    uint8_t gpu_usage;
    uint16_t vram_max;
    uint8_t vram_usage;
    char vram_unit[4];
} DataStruct;
*/

const MIB_TO_BYTES: u64 = 1024 * 1024;

#[derive(Serialize, Debug, Clone)]
pub struct SystemInfo {
    pub cpu_usage: u8,
    pub ram_max: u16,
    pub ram_usage: u8,
    pub ram_unit: [u8; 4],
    pub gpu_usage: u8,
    pub vram_max: u16,
    pub vram_usage: u8,
    pub vram_unit: [u8; 4],
}

impl SystemInfo {
    fn get_unit(exp: u32) -> String {
        match exp {
            0 => "B",
            1 => "KB",
            2 => "MB",
            3 => "GB",
            4 => "TB",
            _ => "UB",
        }
        .to_owned()
    }

    fn get_exp(num: u64, base: u64) -> u32 {
        match num {
            x if x > u64::pow(base, 4) => 4,
            x if x > u64::pow(base, 3) => 3,
            x if x > u64::pow(base, 2) => 2,
            x if x > base => 1,
            _ => 0,
        }
    }

    pub async fn get_system_info(system_info: &mut sysinfo::System) -> Self {
        system_info.refresh_memory_specifics(MemoryRefreshKind::new().with_ram());
        let base = 1024;

        let ram_max = system_info.total_memory();
        let ram_exp = Self::get_exp(ram_max, base);

        let gpu_info = GpuInfo::get_gpu_info().await;
        let vram_mult = u64::pow(base, 2);

        let vram_max = match &gpu_info {
            Some(gi) => gi.vram_max * vram_mult,
            None => 0,
        };
        let vram_exp = Self::get_exp(vram_max, base);

        let vram_usage = match &gpu_info {
            Some(gi) if vram_max > 0 => {
                (gi.vram_used as f64 * vram_mult as f64 / vram_max as f64 * 100.0) as u8
            }
            _ => u8::MAX,
        };

        system_info.refresh_cpu_usage();

        SystemInfo {
            cpu_usage: avg_vecu32(
                system_info
                    .cpus()
                    .iter()
                    .map(|c| c.cpu_usage() as u32)
                    .collect(),
            ) as u8,
            ram_max: (ram_max as f64 / u64::pow(base, ram_exp) as f64 * 10.0) as u16,
            ram_usage: (system_info.used_memory() as f64 / ram_max as f64 * 100.0) as u8,
            ram_unit: pop_4u8(Self::get_unit(ram_exp).as_bytes()),
            gpu_usage: match &gpu_info {
                Some(gi) => gi.gpu_usage as u8,
                None => u8::MAX,
            },
            vram_max: (vram_max as f64 / u64::pow(base, vram_exp) as f64 * 10.0) as u16,
            vram_usage,
            vram_unit: pop_4u8(Self::get_unit(vram_exp).as_bytes()),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct GpuInfo {
    pub gpu_usage: u64,
    pub vram_max: u64,
    pub vram_used: u64,
}

impl GpuInfo {
    pub async fn get_gpu_info() -> Option<Self> {
        #[cfg(target_os = "macos")]
        {
            Self::get_macos_gpu_info().await
        }

        #[cfg(not(target_os = "macos"))]
        {
            Self::get_generic_gpu_info().await
        }
    }

    async fn get_nvidia_gpu_info() -> Option<Self> {
        let Ok(mut cmd) = tokio::process::Command::new("nvidia-smi")
            .arg("-q")
            .arg("-x")
            .stdout(std::process::Stdio::piped())
            .spawn()
        else {
            return None;
        };

        let stdout = cmd.stdout.take()?;
        let mut stdout_reader = tokio::io::BufReader::new(stdout);
        let mut output = String::new();
        if stdout_reader.read_to_string(&mut output).await.is_err() {
            return None;
        }

        let json = xmltojson::to_json(&output).ok()?;
        let g = json["nvidia_smi_log"]["gpu"].to_owned();

        let gpu_usage = nvd_r2u64(g["utilization"]["gpu_util"].to_string())?;
        let vram_max = nvd_r2u64(g["fb_memory_usage"]["total"].to_string())?;
        let vram_used = nvd_r2u64(g["fb_memory_usage"]["used"].to_string())?;

        Some(GpuInfo {
            gpu_usage,
            vram_max,
            vram_used,
        })
    }
}

#[cfg(target_os = "macos")]
impl GpuInfo {
    async fn get_macos_gpu_info() -> Option<Self> {
        if let Some(apple_info) = Self::get_apple_silicon_gpu_info().await {
            return Some(apple_info);
        }

        if let Some(intel_info) = Self::get_macos_intel_gpu_info().await {
            return Some(intel_info);
        }

        Self::get_nvidia_gpu_info().await
    }

    async fn get_apple_silicon_gpu_info() -> Option<Self> {
        let Ok(output) = tokio::process::Command::new("uname")
            .arg("-m")
            .output()
            .await
        else {
            return None;
        };

        let arch = String::from_utf8_lossy(&output.stdout);
        if arch.trim() != "arm64" {
            return None;
        }

        let Ok(output) = tokio::process::Command::new("ioreg")
            .arg("-r")
            .arg("-c")
            .arg("IOAccelerator")
            .output()
            .await
        else {
            return None;
        };

        if !output.status.success() {
            return None;
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut gpu_usage = 0u64;
        let mut vram_used = 0u64;
        let mut vram_max = 0u64;
        let mut is_apple_gpu = false;

        for line in output_str.lines() {
            if line.contains("AGXAccelerator") || line.contains("\"model\" = \"Apple") {
                is_apple_gpu = true;
            }

            if line.contains("\"PerformanceStatistics\"") {
                if let Some(usage) = Self::parse_ioreg_number(line, "\"Device Utilization %\"=", 23)
                {
                    gpu_usage = usage;
                }

                if let Some(mem) = Self::parse_ioreg_number(line, "\"In use system memory\"=", 23) {
                    vram_used = mem / MIB_TO_BYTES;
                }

                if let Some(mem) = Self::parse_ioreg_number(line, "\"Alloc system memory\"=", 22) {
                    vram_max = mem / MIB_TO_BYTES;
                }
            }
        }

        if !is_apple_gpu {
            return None;
        }

        Some(GpuInfo {
            gpu_usage,
            vram_max,
            vram_used,
        })
    }

    async fn get_macos_intel_gpu_info() -> Option<Self> {
        let Ok(output) = tokio::process::Command::new("ioreg")
            .arg("-r")
            .arg("-c")
            .arg("IOAccelerator")
            .output()
            .await
        else {
            return None;
        };

        if !output.status.success() {
            return None;
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut vram_max = 0u64;
        let mut is_intel = false;

        for line in output_str.lines() {
            if line.contains("\"IOClass\" = \"IntelAccelerator\"")
                || line.contains("\"model\" = <\"Intel")
            {
                is_intel = true;
            }

            if line.contains("\"VRAM,totalMB\"") {
                if let Some(equals_pos) = line.find('=') {
                    let after_equals = line[equals_pos + 1..].trim();
                    let number_str = after_equals
                        .trim_start_matches('<')
                        .trim_end_matches('>')
                        .trim();
                    if let Ok(mb_val) = number_str.parse::<u64>() {
                        vram_max = mb_val;
                    }
                }
            }
        }

        if !is_intel || vram_max == 0 {
            return None;
        }

        Some(GpuInfo {
            gpu_usage: 0,
            vram_max,
            vram_used: 0,
        })
    }

    fn parse_ioreg_number(line: &str, key: &str, offset: usize) -> Option<u64> {
        line.find(key).and_then(|start| {
            let after_equals = &line[start + offset..];
            let end = after_equals.find(|c: char| !c.is_ascii_digit())?;
            after_equals[..end].parse::<u64>().ok()
        })
    }
}

#[cfg(not(target_os = "macos"))]
impl GpuInfo {
    async fn get_generic_gpu_info() -> Option<Self> {
        if let Some(nvidia_info) = Self::get_nvidia_gpu_info().await {
            return Some(nvidia_info);
        }

        if let Some(intel_info) = Self::get_intel_gpu_info().await {
            return Some(intel_info);
        }

        None
    }

    async fn get_intel_gpu_info() -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            Self::get_windows_intel_gpu_info().await
        }

        #[cfg(target_os = "linux")]
        {
            Self::get_linux_intel_gpu_info().await
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            None
        }
    }

    #[cfg(target_os = "windows")]
    async fn get_windows_intel_gpu_info() -> Option<Self> {
        let Ok(output) = tokio::process::Command::new("wmic")
            .arg("path")
            .arg("win32_VideoController")
            .arg("get")
            .arg("Name,AdapterRAM")
            .arg("/format:csv")
            .output()
            .await
        else {
            return None;
        };

        if !output.status.success() {
            return None;
        }

        let output_str = String::from_utf8_lossy(&output.stdout);

        for line in output_str.lines().skip(1) {
            if line.to_lowercase().contains("intel") {
                let parts: Vec<&str> = line.split(',').collect();

                if parts.len() >= 2 {
                    if let Ok(ram_bytes) = parts[1].trim().parse::<u64>() {
                        if ram_bytes > 0 {
                            let vram_max = ram_bytes / MIB_TO_BYTES;
                            return Some(GpuInfo {
                                gpu_usage: 0,
                                vram_max,
                                vram_used: 0,
                            });
                        }
                    }
                }
            }
        }

        None
    }

    #[cfg(target_os = "linux")]
    async fn get_linux_intel_gpu_info() -> Option<Self> {
        let drm_path = std::path::Path::new("/sys/class/drm");
        if !drm_path.exists() {
            return None;
        }

        let Ok(entries) = std::fs::read_dir(drm_path) else {
            return None;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let device_path = path.join("device");
            let vendor_path = device_path.join("vendor");

            if let Ok(vendor) = std::fs::read_to_string(&vendor_path) {
                if vendor.trim() == "0x8086" {
                    let mem_info_path = device_path.join("mem_info_vram_total");
                    if let Ok(mem_str) = std::fs::read_to_string(&mem_info_path) {
                        if let Ok(mem_bytes) = mem_str.trim().parse::<u64>() {
                            return Some(GpuInfo {
                                gpu_usage: 0,
                                vram_max: mem_bytes / MIB_TO_BYTES,
                                vram_used: 0,
                            });
                        }
                    }

                    return None;
                }
            }
        }

        None
    }
}
