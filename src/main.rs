use btleplug::api::{Central, CentralEvent, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral, PeripheralId};
use futures::stream::StreamExt;
use std::collections::HashMap;
use std::error::Error;

mod flipper_manager;
mod helpers;
mod system_info;

async fn data_sender(flipper: Peripheral) {
    let id = flipper.id();
    let chars = flipper.characteristics();
    let cmd_char = match chars
        .iter()
        .find(|c| c.uuid == flipper_manager::FLIPPER_CHARACTERISTIC_UUID)
    {
        Some(c) => c,
        None => {
            return println!("[{}] Failed to find characteristic", id.to_string());
        }
    };
    println!("[{}] Sending data...", id.to_string());

    // Reuse system variable in loop (small performance and RAM boost)
    let mut system_info = sysinfo::System::new_all();
    loop {
        let systeminfo = system_info::SystemInfo::get_system_info(&mut system_info).await;
        let systeminfo_bytes = bincode::serialize(&systeminfo).unwrap();
        // println!("Writing {:?} to Flipper", systeminfo_bytes);

        if let Err(e) = flipper
            .write(
                cmd_char,
                &systeminfo_bytes,
                btleplug::api::WriteType::WithoutResponse,
            )
            .await
        {
            println!("[{}] Failed to write: {}", id.to_string(), e);
        };

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

async fn reconnect_thread(central: Adapter, id: PeripheralId) {
    loop {
        if let Some(flipper) = flipper_manager::get_flipper(&central, &id).await {
            let _ = flipper.connect().await;
        };

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
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

    println!("Scanning... Launch PC Monitor app on Flipper");
    central.start_scan(ScanFilter::default()).await?;

    let mut data_workers: HashMap<PeripheralId, tokio::task::JoinHandle<()>> = HashMap::new();
    let mut reconnect_workers: HashMap<PeripheralId, tokio::task::JoinHandle<()>> = HashMap::new();

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                if let Some(flp) = flipper_manager::get_flipper(&central, &id).await {
                    println!("[{}] Connecting to Flipper", &id.to_string());
                    if let Err(e) = flp.connect().await {
                        println!(
                            "[{}] Failed to connect to Flipper: {}",
                            id.to_string(),
                            e.to_string()
                        );
                    }
                }
            }
            CentralEvent::DeviceConnected(id) => {
                if let Some(flp) = flipper_manager::get_flipper(&central, &id).await {
                    flp.discover_services().await?;
                    println!("[{}] Connected to Flipper", &id.to_string());

                    data_workers.insert(id.clone(), tokio::spawn(data_sender(flp)));
                };

                match reconnect_workers.get(&id) {
                    Some(worker) => {
                        worker.abort();
                        reconnect_workers.remove(&id);
                    }
                    None => {}
                }
            }
            CentralEvent::DeviceDisconnected(id) => {
                match data_workers.get(&id) {
                    Some(worker) => {
                        worker.abort();
                        println!(
                            "[{}] Disconnected from Flipper. Waiting for reconnection",
                            &id.to_string()
                        );

                        data_workers.remove(&id);
                    }
                    None => {}
                };

                reconnect_workers.insert(
                    id.clone(),
                    tokio::spawn(reconnect_thread(central.clone(), id)),
                );
            }
            _ => {}
        }
    }
    Ok(())
}
