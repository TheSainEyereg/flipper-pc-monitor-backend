use btleplug::api::{
    Central, CentralEvent, Characteristic, Manager as _, Peripheral as _, ScanFilter,
};
use btleplug::platform::{Manager, Peripheral};
use futures::stream::StreamExt;
use std::collections::HashMap;
use std::error::Error;

mod flipper_manager;
mod helpers;
mod system_info;

async fn data_sender(flipper: Peripheral) {
    println!("Now you can launch PC Monitor app on your Flipper");

    loop {
        let chars = flipper.characteristics();
        let cmd_char = chars
            .iter()
            .find(|c| c.uuid == flipper_manager::FLIPPER_CHARACTERISTIC_UUID)
            .expect("Flipper Characteristic not found");

        let systeminfo = system_info::SystemInfo::get_system_info().await;
        let systeminfo_bytes = bincode::serialize(&systeminfo).unwrap();
        // println!("Writing {:?} to Flipper", systeminfo_bytes);

        flipper
            .write(
                cmd_char,
                &systeminfo_bytes,
                btleplug::api::WriteType::WithoutResponse,
            )
            .await
            .expect("Failed to write to Flipper");

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    // std::env::set_var("RUST_BACKTRACE", "full");

    let manager = Manager::new().await?;

    let central = flipper_manager::get_central(&manager).await;
    println!("Found {:?} adapter", central.adapter_info().await.unwrap());

    let mut events = central.events().await?;

    println!("Scanning...");
    central.start_scan(ScanFilter::default()).await?;

	let mut workers = HashMap::new();

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                // println!("Device Discovered: {}", &id.to_string());
                if let Some(flp) = flipper_manager::get_flipper(&central, &id).await {
                    println!("Connecting to Flipper {}", &id.to_string());
                    match flp.connect().await {
                        Err(_) => println!("Failed to connect to Flipper {}", id.to_string()),
                        _ => {}
                    }
                }
            }
            CentralEvent::DeviceConnected(id) => {
                if let Some(flp) = flipper_manager::get_flipper(&central, &id).await {
                    flp.discover_services().await?;
                    println!("Connected to Flipper {}", &id.to_string());

					workers.insert(id, tokio::spawn(data_sender(flp)));
                };
            }
            CentralEvent::DeviceDisconnected(id) => {
				match workers.get(&id) {
					Some(worker) => {
						worker.abort();
						println!("Disconnected from Flipper {}", &id.to_string());
						
						workers.remove(&id);
					},
					None => {}
				};
            }
            _ => {}
        }
    }
    Ok(())
}
