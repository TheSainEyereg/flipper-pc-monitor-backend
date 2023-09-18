use crate::helpers::{avg_vecu32, nvd_r2u32, pop_4u8};
use serde::Serialize;
use sysinfo::{CpuExt, SystemExt};
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
    fn get_unit(exp: u8) -> String {
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

    pub async fn get_system_info() -> Self {
        let mut system_info = sysinfo::System::new_all();
        system_info.refresh_all();
        let base: u64 = 1024;

        let ram_max = system_info.total_memory();
        let ram_exp: u8 = match ram_max {
            x if x > u64::pow(base, 4) => 4,
            x if x > u64::pow(base, 3) => 3,
            x if x > u64::pow(base, 2) => 2,
            x if x > base => 1,
            _ => 0,
        };

        let gpu_info = GpuInfo::get_gpu_info().await;
        let vram_mult: u64 = u64::pow(base, 2); // MiB

        let vram_max = match &gpu_info {
            Some(gi) => nvd_r2u32(&gi.vram_max) as u64 * vram_mult,
            None => 0,
        };
        let vram_exp: u8 = match vram_max {
            x if x > u64::pow(base, 4) => 4,
            x if x > u64::pow(base, 3) => 3,
            x if x > u64::pow(base, 2) => 2,
            x if x > base => 1,
            _ => 0,
        };

        SystemInfo {
            cpu_usage: avg_vecu32(
                system_info
                    .cpus()
                    .iter()
                    .map(|c| c.cpu_usage() as u32)
                    .collect(),
            ) as u8,
            // cpu_usage: system_info.cpus().first().unwrap().cpu_usage() as u8,
            ram_max: (ram_max as f64 / u64::pow(base, ram_exp as u32) as f64 * 10.0) as u16,
            ram_usage: (system_info.used_memory() as f64 / ram_max as f64 * 100.0) as u8,
            ram_unit: pop_4u8(Self::get_unit(ram_exp).as_bytes()),
            gpu_usage: match &gpu_info {
                Some(gi) => nvd_r2u32(&gi.gpu_usage) as u8,
                None => u8::MAX,
            },
            vram_max: (vram_max as f64 / u64::pow(base, vram_exp as u32) as f64 * 10.0) as u16,
            vram_usage: match &gpu_info {
                Some(gi) => {
                    (nvd_r2u32(&gi.vram_used) as f64 * vram_mult as f64 / vram_max as f64 * 100.0)
                        as u8
                }
                None => u8::MAX,
            },
            vram_unit: pop_4u8(Self::get_unit(vram_exp).as_bytes()),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct GpuInfo {
    pub gpu_usage: String,
    pub vram_max: String,
    pub vram_used: String,
}

impl GpuInfo {
    pub async fn get_gpu_info() -> Option<Self> {
        // TODO: AMD support
        let Ok(mut cmd) = tokio::process::Command::new("nvidia-smi")
            .arg("-q")
            .arg("-x")
            .stdout(std::process::Stdio::piped())
            .spawn() else {
				return None;
			};

        let stdout = cmd.stdout.take().unwrap();
        let mut stdout_reader = tokio::io::BufReader::new(stdout);
        let mut mut_stdout = String::new();
        if stdout_reader.read_to_string(&mut mut_stdout).await.is_err() {
            return None;
        };

        match xmltojson::to_json(&mut_stdout) {
            Ok(json) => {
                let g = json["nvidia_smi_log"]["gpu"].to_owned();
                Some(GpuInfo {
                    vram_max: (g["fb_memory_usage"]["total"].to_string()),
                    gpu_usage: (g["utilization"]["gpu_util"].to_string()),
                    vram_used: (g["fb_memory_usage"]["used"].to_string()),
                })
            }
            Err(_) => None,
        }
    }
}
