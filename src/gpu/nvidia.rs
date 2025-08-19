use tokio::io::AsyncReadExt;

use crate::helpers::nvd_r2u64;

pub async fn get_gpu_info() -> Option<super::GpuInfo> {
    let Ok(mut cmd) = tokio::process::Command::new("nvidia-smi")
        .arg("-q")
        .arg("-x")
        .stdout(std::process::Stdio::piped())
        .spawn()
    else {
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

            let Some(gpu_usage) = nvd_r2u64(g["utilization"]["gpu_util"].to_string()) else {
                return None;
            };
            let Some(vram_max) = nvd_r2u64(g["fb_memory_usage"]["total"].to_string()) else {
                return None;
            };
            let Some(vram_used) = nvd_r2u64(g["fb_memory_usage"]["used"].to_string()) else {
                return None;
            };

            Some(super::GpuInfo {
                gpu_usage,
                vram_max,
                vram_used,
            })
        }
        Err(_) => None,
    }
}
