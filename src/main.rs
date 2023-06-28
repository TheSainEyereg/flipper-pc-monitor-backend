#![feature(if_let_guard)]

use std::error::Error;
use btleplug::api::{
    Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter, Characteristic,
};
use btleplug::platform::{Manager, Peripheral};
use futures::stream::StreamExt;

mod flipper_manager;
mod system_info;

async fn data_sender(flipper: Peripheral) {
    // TODO: AMD (suck) support
    let mut systeminfo = serde_json::to_string(&system_info::SystemInfo::get_system_info().await).unwrap();
    let chars = flipper.characteristics();
    let cmd_char = chars
        .iter()
        .find(|c| c.uuid == flipper_manager::FLIPPER_CHARACTERISTIC_UUID)
        .expect("Flipper Characteristic not found");
    println!("Writing {:?} to Flipper", systeminfo.as_bytes());

    flipper.write(cmd_char, systeminfo.as_bytes(), 
        btleplug::api::WriteType::WithoutResponse).await.expect("Failed to write to Flipper");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let manager = Manager::new().await?;

    let central = flipper_manager::get_central(&manager).await;
    println!("Found {:?} adapter", central.adapter_info().await.unwrap());

    let mut events = central.events().await?;

    println!("Scanning...");
    central.start_scan(ScanFilter::default()).await?;

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                println!("Device Discovered: {}", &id.to_string());
                if let Some(flp) = flipper_manager::get_flipper(&central, &id).await {
                    println!("Connecting to Flipper {}", &id.to_string());
                    flp.connect().await.expect(format!("Failed to connect to Flipper {}", id.to_string()).as_str());
                }
            }
            CentralEvent::DeviceConnected(id) => {
                if let Some(flp) = flipper_manager::get_flipper(&central, &id).await {
                    println!("Connected to Flipper {}\nDiscover Services", &id.to_string());
                    flp.discover_services().await?;
                    println!("Services Discovered");

                    tokio::spawn(data_sender(flp));
                };
            }
            _ => {}
        }
    }

    Ok(())
}
