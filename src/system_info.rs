use serde::{Serialize};
use sysinfo::{SystemExt, CpuExt};
use tokio::io::AsyncReadExt;

#[derive(Serialize, Debug, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub usage: String,
    pub memory: String
}

#[derive(Serialize, Debug, Clone)]
pub struct SystemInfo {
    pub os: String,
    pub cpus: Vec<String>,
    pub ram_max: String,
    pub ram_usage: String,
    pub gpu: GpuInfo,
}

impl SystemInfo {
    pub async fn get_system_info() -> Self {
        let system_info = sysinfo::System::new_all();
    
        SystemInfo {
            os: system_info.name().unwrap(),
            cpus: system_info.cpus().iter().map(|c| c.name().to_owned()).collect(),
            ram_max: system_info.total_memory().to_string(),
            ram_usage: system_info.used_memory().to_string(),
            gpu: GpuInfo::get_gpu_info().await
        }
    }
}

impl GpuInfo {
    pub async fn get_gpu_info() -> Self {
        let mut cmd = tokio::process::Command::new("nvidia-smi")
            .arg("-q")
            .arg("-x")
            .stdout(std::process::Stdio::piped())
            .spawn().unwrap();

        let stdout = cmd.stdout.take().unwrap();
        let mut stdout_reader = tokio::io::BufReader::new(stdout);
        let mut mut_stdout = String::new();
        stdout_reader.read_to_string(&mut mut_stdout).await.unwrap();

        let json = xmltojson::to_json(&mut_stdout).unwrap()["nvidia_smi_log"]["gpu"].to_owned();

        GpuInfo {
            name: (json["product_name"].to_string()),
            usage: (json["fb_memory_usage"]["used"].to_string()),
            memory: (json["fb_memory_usage"]["total"].to_string()),
        }
    }
}