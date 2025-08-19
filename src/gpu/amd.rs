use tokio::io::AsyncReadExt;

pub async fn get_gpu_info() -> Option<super::GpuInfo> {
    let Ok(mut cmd) = tokio::process::Command::new("rocm-smi")
        .arg("--showmeminfo")
        .arg("vram")
        .arg("-u")
        .arg("--json")
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

    match serde_json::from_str::<serde_json::Value>(&mut_stdout) {
        Ok(json) => {
            let g = json["card0"].to_owned();

            let Some(gpu_usage) = g["GPU use (%)"].as_str().map(|x| x.parse().ok()).flatten()
            else {
                return None;
            };
            let Some(vram_max) = g["VRAM Total Memory (B)"]
                .as_str()
                .map(|x| x.parse().ok())
                .flatten()
            else {
                return None;
            };
            let Some(vram_used) = g["VRAM Total Used Memory (B)"]
                .as_str()
                .map(|x| x.parse().ok())
                .flatten()
            else {
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
