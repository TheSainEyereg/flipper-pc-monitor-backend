use serde::Serialize;
use sysinfo::{SystemExt, CpuExt};
// use tokio::io::AsyncReadExt;
use crate::helpers::{avg_vecu32, pop_4u8};

#[derive(Serialize, Debug, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub usage: String,
    pub memory: String
}

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
			0 => { "B" }
			1 => { "KB" }
			2 => { "MB" }
			3 => { "GB" }
			4 => { "TB" }
			_ => { "UB" }
		}.to_owned()
	}

    pub async fn get_system_info() -> Self {
        let system_info = sysinfo::System::new_all();
		let base: u32 = 1024;

		let ram_max = system_info.available_memory() as u32;
		let ram_exp: u8 = if ram_max > u32::pow(base, 3) { 3 } else if ram_max > u32::pow(base, 2) { 2 } else if ram_max > base { 1 } else { 0 };
		let ram_divider = f32::powf(base as f32, ram_exp as f32);

		// let vram_max =  0 as u32;
		// let vram_exp: u8 = if vram_max > u32::pow(base, 4) { 4 } else if vram_max > u32::pow(base, 3) { 3 } else if vram_max > u32::pow(base, 2) { 2 } else if vram_max > base { 1 } else { 0 };
		// let vram_divider = f32::powf(base as f32, vram_max as f32);

        SystemInfo {
			cpu_usage: avg_vecu32(system_info.cpus().iter().map(|c| (c.cpu_usage() * 100.0) as u32).collect()) as u8,
			ram_max: (ram_max as f32 / ram_divider) as u16,	
			ram_usage: (system_info.used_memory() as f32 / ram_divider * 100.0) as u8,
			ram_unit: pop_4u8(Self::get_unit(ram_exp).as_bytes()),
			gpu_usage: 0,
			vram_max: 0,
			vram_usage: 0,
			vram_unit: [0; 4],
        }
    }
}

impl GpuInfo {
    // pub async fn get_gpu_info() -> Self {
	// 	// TODO: AMD support
    //     let mut cmd = tokio::process::Command::new("nvidia-smi")
    //         .arg("-q")
    //         .arg("-x")
    //         .stdout(std::process::Stdio::piped())
    //         .spawn().unwrap();

    //     let stdout = cmd.stdout.take().unwrap();
    //     let mut stdout_reader = tokio::io::BufReader::new(stdout);
    //     let mut mut_stdout = String::new();
    //     stdout_reader.read_to_string(&mut mut_stdout).await.unwrap();

    //     let json = xmltojson::to_json(&mut_stdout).unwrap()["nvidia_smi_log"]["gpu"].to_owned();

    //     GpuInfo {
    //         name: (json["product_name"].to_string()),
    //         usage: (json["fb_memory_usage"]["used"].to_string()),
    //         memory: (json["fb_memory_usage"]["total"].to_string()),
    //     }
    // }
}