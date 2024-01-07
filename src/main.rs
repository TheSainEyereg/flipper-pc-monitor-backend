use btleplug::api::{
    Central, CentralEvent, Characteristic, Manager as _, Peripheral as _, ScanFilter,
};
use btleplug::platform::{Manager, Peripheral};
use futures::stream::StreamExt;
use std::collections::HashMap;
use std::error::Error;
use std::io::{self, Write};
use sysinfo::{System};

mod flipper_manager;
mod helpers;
mod system_info;

async fn data_sender(flipper: Peripheral) {
    let chars = flipper.characteristics();
    let cmd_char = match chars
        .iter()
        .find(|c| c.uuid == flipper_manager::FLIPPER_CHARACTERISTIC_UUID)
    {
        Some(c) => c,
        None => {
            return println!("Failed to find characteristic");
        }
    };
    println!("Now you can launch PC Monitor app on your Flipper");
    
    // Reuse system variable in loop (small performance and RAM boost)
    let mut system_info = sysinfo::System::new_all();
    loop {
        let systeminfo = system_info::SystemInfo::get_system_info(&mut system_info).await;
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
    println!();

    let mut events = central.events().await?;
    let mut flipper_name = String::new();
    println!("- Scan will be searching for Flippers with a name that contains the string you enter here");
    println!("- If you run official firmware you should be fine by entering 'Flipper' (case sensitive)");
    println!("- Empty string will search for all possible Flippers (experimental)");
    println!();
    print!("Enter the name (for empty just press Enter): ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut flipper_name).expect("Error: unable to read user input");
    let flipper_name = flipper_name.trim();
    println!();

    println!("Scanning...");
    central.start_scan(ScanFilter::default()).await?;

    let mut workers = HashMap::new();

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                // println!("Device Discovered: {}", &id.to_string());
                if let Some(flp) = flipper_manager::get_flipper(&central, &id, (&flipper_name).to_string()).await {
                    println!("Connecting to Flipper {}", &id.to_string());
                    match flp.connect().await {
                        Err(_) => println!("Failed to connect to Flipper {}", id.to_string()),
                        _ => {}
                    }
                }
            }
            CentralEvent::DeviceConnected(id) => {
                if let Some(flp) = flipper_manager::get_flipper(&central, &id, (&flipper_name).to_string()).await {
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
                    }
                    None => {}
                };
            }
            _ => {}
        }
    }
    Ok(())
}
